use crate::{
    models::{FileCandidate, SearchResult},
    storage,
};
use anyhow::Result;
use rusqlite::Connection;

pub fn search_files(
    conn: &Connection,
    query: &str,
    root_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let candidates = storage::fetch_candidates(conn, query, root_ids, limit)?;

    let mut results = if query.is_empty() {
        candidates
            .into_iter()
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
                match_reasons: vec!["recent file".to_string()],
            })
            .collect::<Vec<_>>()
    } else {
        let tokens = tokenize(query);
        let mut scored = candidates
            .into_iter()
            .map(|candidate| score_candidate(candidate, query, &tokens))
            .collect::<Vec<_>>();

        scored.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(right.modified_at.cmp(&left.modified_at))
                .then_with(|| left.name.cmp(&right.name))
        });
        scored.truncate(limit);
        scored
    };

    results.truncate(limit);
    Ok(results)
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
        match_reasons: reasons,
    }
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|token| token.trim_matches(|char: char| !char.is_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}
