use crate::{
    models::{ContentMatch, FileCandidate, SearchResult, SemanticMatch},
    semantic, storage,
};
use anyhow::Result;
use rusqlite::Connection;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub fn search_files(
    conn: &Connection,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    query: &str,
    root_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let metadata_candidates = storage::fetch_candidates(conn, query, root_ids, limit)?;

    if query.is_empty() {
        return Ok(metadata_candidates
            .into_iter()
            .take(limit)
            .map(|candidate| SearchResult {
                file_id: candidate.file_id,
                root_id: candidate.root_id,
                name: candidate.name,
                path: candidate.path,
                extension: candidate.extension,
                kind: candidate.kind,
                size: candidate.size,
                modified_at: candidate.modified_at,
                indexed_at: candidate.indexed_at,
                score: 0,
                semantic_score: None,
                match_reasons: vec!["recent file".to_string()],
                snippet: None,
                snippet_source: None,
            })
            .collect());
    }

    let tokens = tokenize(query);
    let mut combined = HashMap::<i64, SearchResult>::new();

    for candidate in metadata_candidates {
        let scored = score_candidate(candidate, query, &tokens);
        combined.insert(scored.file_id, scored);
    }

    if let Some(fts_query) = build_fts_query(&tokens) {
        let content_matches =
            storage::search_content_matches(conn, &fts_query, root_ids, limit * 8)?;

        for (rank, content_match) in content_matches.into_iter().enumerate() {
            merge_content_match(&mut combined, content_match, &tokens, rank);
        }
    }

    if let Ok(semantic_matches) =
        semantic::search_semantic(vector_db_path, model_cache_dir, query, root_ids, limit * 3)
    {
        merge_semantic_matches(conn, &mut combined, semantic_matches);
    }

    let mut results = combined.into_values().collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| compare_semantic_score(right.semantic_score, left.semantic_score))
            .then(right.modified_at.cmp(&left.modified_at))
            .then_with(|| left.name.cmp(&right.name))
    });
    results.truncate(limit);
    Ok(results)
}

fn merge_semantic_matches(
    conn: &Connection,
    combined: &mut HashMap<i64, SearchResult>,
    semantic_matches: Vec<SemanticMatch>,
) {
    if semantic_matches.is_empty() {
        return;
    }

    let existing_ids = combined.keys().copied().collect::<HashSet<_>>();
    let missing_ids = semantic_matches
        .iter()
        .map(|hit| hit.file_id)
        .filter(|file_id| !existing_ids.contains(file_id))
        .collect::<Vec<_>>();

    let mut candidates_by_id = storage::fetch_candidates_by_ids(conn, &missing_ids)
        .unwrap_or_default()
        .into_iter()
        .map(|candidate| (candidate.file_id, candidate))
        .collect::<HashMap<_, _>>();

    for (rank, semantic_match) in semantic_matches.into_iter().enumerate() {
        let fallback_candidate = candidates_by_id.remove(&semantic_match.file_id);
        merge_semantic_match(combined, semantic_match, fallback_candidate, rank);
    }
}

fn score_candidate(candidate: FileCandidate, query: &str, tokens: &[String]) -> SearchResult {
    let lower_name = candidate.name.to_lowercase();
    let lower_path = candidate.path.to_lowercase();
    let lower_ext = candidate.extension.to_lowercase();

    let mut score = 0;
    let mut reasons = Vec::new();

    if lower_name == query {
        score += 220;
        reasons.push("exact filename match".to_string());
    }

    if lower_path == query {
        score += 180;
        reasons.push("exact path match".to_string());
    }

    if lower_ext == query {
        score += 130;
        reasons.push("file type match".to_string());
    }

    if lower_name.contains(query) {
        score += 110;
        reasons.push("filename match".to_string());
    }

    if lower_path.contains(query) {
        score += 70;
        reasons.push("path match".to_string());
    }

    let mut token_hits = 0;
    for token in tokens {
        if lower_name.contains(token) {
            score += 32;
            token_hits += 1;
        } else if lower_path.contains(token) {
            score += 16;
            token_hits += 1;
        } else if lower_ext == *token {
            score += 24;
            token_hits += 1;
        }
    }

    if token_hits > 0 {
        reasons.push("keyword match".to_string());
    }

    if reasons.is_empty() {
        reasons.push("metadata match".to_string());
    } else {
        reasons.sort();
        reasons.dedup();
    }

    SearchResult {
        file_id: candidate.file_id,
        root_id: candidate.root_id,
        name: candidate.name,
        path: candidate.path,
        extension: candidate.extension,
        kind: candidate.kind,
        size: candidate.size,
        modified_at: candidate.modified_at,
        indexed_at: candidate.indexed_at,
        score,
        semantic_score: None,
        match_reasons: reasons,
        snippet: None,
        snippet_source: None,
    }
}

fn merge_content_match(
    combined: &mut HashMap<i64, SearchResult>,
    content_match: ContentMatch,
    tokens: &[String],
    rank: usize,
) {
    let snippet = build_snippet(&content_match.text, tokens);
    let content_score = (280_i64 - (rank as i64 * 7)).max(90);

    let entry = combined
        .entry(content_match.file_id)
        .or_insert_with(|| SearchResult {
            file_id: content_match.file_id,
            root_id: content_match.root_id,
            name: content_match.name,
            path: content_match.path,
            extension: content_match.extension,
            kind: content_match.kind,
            size: content_match.size,
            modified_at: content_match.modified_at,
            indexed_at: content_match.indexed_at,
            score: 0,
            semantic_score: None,
            match_reasons: Vec::new(),
            snippet: None,
            snippet_source: None,
        });

    entry.score += content_score;
    entry.match_reasons.push("text match".to_string());

    if entry.snippet.is_none() {
        entry.snippet = snippet;
        entry.snippet_source = content_match.source_label;
    }

    entry.match_reasons.sort();
    entry.match_reasons.dedup();
}

fn merge_semantic_match(
    combined: &mut HashMap<i64, SearchResult>,
    semantic_match: SemanticMatch,
    fallback_candidate: Option<FileCandidate>,
    rank: usize,
) {
    let modality = semantic_match.modality.clone();
    let reason = if modality == "image" {
        "visual match"
    } else {
        "semantic match"
    };
    let semantic_score =
        (semantic_match.similarity * 240.0).round() as i64 - (rank as i64 * 4).min(40);

    if !combined.contains_key(&semantic_match.file_id) && fallback_candidate.is_none() {
        return;
    }

    let entry =
        combined
            .entry(semantic_match.file_id)
            .or_insert_with(|| match fallback_candidate {
                Some(candidate) => SearchResult {
                    file_id: candidate.file_id,
                    root_id: candidate.root_id,
                    name: candidate.name,
                    path: candidate.path,
                    extension: candidate.extension,
                    kind: candidate.kind,
                    size: candidate.size,
                    modified_at: candidate.modified_at,
                    indexed_at: candidate.indexed_at,
                    score: 0,
                    semantic_score: None,
                    match_reasons: Vec::new(),
                    snippet: None,
                    snippet_source: None,
                },
                None => unreachable!("semantic result without fallback candidate"),
            });

    entry.score += semantic_score.max(60);
    entry.semantic_score = Some(
        entry
            .semantic_score
            .map(|score| score.max(semantic_match.similarity))
            .unwrap_or(semantic_match.similarity),
    );
    entry.match_reasons.push(reason.to_string());

    if entry.snippet.is_none() && modality == "image" {
        entry.snippet = Some("Matched using local image embeddings.".to_string());
        entry.snippet_source = Some("Visual features".to_string());
    }

    entry.match_reasons.sort();
    entry.match_reasons.dedup();
}

fn compare_semantic_score(left: Option<f32>, right: Option<f32>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left
            .partial_cmp(&right)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|token| token.trim_matches(|char: char| !char.is_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn build_fts_query(tokens: &[String]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    Some(
        tokens
            .iter()
            .map(|token| format!("{token}*"))
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

fn build_snippet(text: &str, tokens: &[String]) -> Option<String> {
    const SNIPPET_CHARS: usize = 220;

    if text.trim().is_empty() {
        return None;
    }

    let lowercase = text.to_lowercase();
    let first_match = tokens
        .iter()
        .filter_map(|token| lowercase.find(token))
        .min()
        .unwrap_or(0);

    let snippet_start = first_match.saturating_sub(70);
    let snippet_end = (snippet_start + SNIPPET_CHARS).min(text.len());
    let mut snippet = text
        .get(snippet_start..snippet_end)
        .unwrap_or(text)
        .trim()
        .to_string();

    if snippet_start > 0 {
        snippet.insert(0, '…');
    }
    if snippet_end < text.len() {
        snippet.push('…');
    }

    Some(snippet)
}
