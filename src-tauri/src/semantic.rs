use crate::{
    extractors::ExtractionOutput,
    models::{SemanticMatch, SemanticSourceFile},
};
use anyhow::{anyhow, Context, Result};
use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, RecordBatchIterator,
    StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use fastembed::{
    EmbeddingModel, ImageEmbedding, ImageEmbeddingModel, ImageInitOptions, InitOptions,
    TextEmbedding,
};
use futures::TryStreamExt;
use lancedb::{
    connect,
    query::{ExecutableQuery, QueryBase},
    Connection, Table,
};
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const TABLE_NAME: &str = "file_embeddings";
const CLIP_MODEL_NAME: &str = "clip-vit-b32";
const VECTOR_DIMENSIONS: i32 = 512;
const MAX_TEXT_CHARS: usize = 1_600;
pub const SEMANTIC_TEXT_BATCH_SIZE: usize = 96;
pub const SEMANTIC_IMAGE_BATCH_SIZE: usize = 12;

#[derive(Debug, Clone)]
pub enum SemanticPayload {
    Text(String),
    Image(PathBuf),
}

#[derive(Debug, Clone)]
pub struct SemanticPlan {
    pub status: String,
    pub modality: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SemanticIndexItem {
    pub file_id: i64,
    pub root_id: i64,
    pub kind: String,
    pub modality: String,
    pub summary: Option<String>,
    pub payload: SemanticPayload,
}

#[derive(Debug, Clone)]
pub struct IndexedSemanticRecord {
    pub file_id: i64,
    pub status: String,
    pub modality: Option<String>,
    pub model: Option<String>,
    pub summary: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct SemanticIndexHandle {
    connection: Connection,
    table: Option<Table>,
}

#[derive(Default)]
struct SemanticModels {
    cache_dir: Option<PathBuf>,
    text: Option<TextEmbedding>,
    image: Option<ImageEmbedding>,
}

static MODELS: Lazy<Mutex<SemanticModels>> = Lazy::new(|| Mutex::new(SemanticModels::default()));

pub fn semantic_model_name() -> &'static str {
    CLIP_MODEL_NAME
}

pub fn prepare_semantic_plan(kind: &str) -> SemanticPlan {
    if kind == "image" {
        return SemanticPlan {
            status: "pending".to_string(),
            modality: Some("image".to_string()),
            summary: Some("Visual features".to_string()),
        };
    }

    if matches!(kind, "document" | "text" | "code") {
        return SemanticPlan {
            status: "pending".to_string(),
            modality: Some("text".to_string()),
            summary: Some("Semantic text preview".to_string()),
        };
    }

    SemanticPlan {
        status: "unsupported".to_string(),
        modality: None,
        summary: None,
    }
}

pub fn open_index_handle(vector_db_path: &Path) -> Result<SemanticIndexHandle> {
    tauri::async_runtime::block_on(async move {
        let connection = connect_to_db(vector_db_path).await?;
        let table = connection.open_table(TABLE_NAME).execute().await.ok();
        Ok::<SemanticIndexHandle, anyhow::Error>(SemanticIndexHandle { connection, table })
    })
}

pub fn index_batch_with_handle(
    handle: &mut SemanticIndexHandle,
    model_cache_dir: &Path,
    items: &[SemanticIndexItem],
) -> Result<Vec<IndexedSemanticRecord>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let mut vectors_by_file = HashMap::<i64, Vec<f32>>::new();

    let text_items = items
        .iter()
        .filter_map(|item| match &item.payload {
            SemanticPayload::Text(text) => Some((item.file_id, text.clone())),
            SemanticPayload::Image(_) => None,
        })
        .collect::<Vec<_>>();
    if !text_items.is_empty() {
        let embeddings = with_models(model_cache_dir, |models| {
            let model = ensure_text_model(models, model_cache_dir)?;
            let inputs = text_items
                .iter()
                .map(|(_, text)| text.clone())
                .collect::<Vec<_>>();
            model.embed(inputs, None).map_err(Into::into)
        })?;

        for ((file_id, _), embedding) in text_items.into_iter().zip(embeddings) {
            validate_dimensions(&embedding)?;
            vectors_by_file.insert(file_id, embedding);
        }
    }

    let image_items = items
        .iter()
        .filter_map(|item| match &item.payload {
            SemanticPayload::Image(path) => Some((item.file_id, path.clone())),
            SemanticPayload::Text(_) => None,
        })
        .collect::<Vec<_>>();
    if !image_items.is_empty() {
        let embeddings = with_models(model_cache_dir, |models| {
            let model = ensure_image_model(models, model_cache_dir)?;
            let inputs = image_items
                .iter()
                .map(|(_, path)| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            model.embed(inputs, None).map_err(Into::into)
        })?;

        for ((file_id, _), embedding) in image_items.into_iter().zip(embeddings) {
            validate_dimensions(&embedding)?;
            vectors_by_file.insert(file_id, embedding);
        }
    }

    let enriched_items = items
        .iter()
        .filter_map(|item| {
            vectors_by_file.get(&item.file_id).map(|vector| IndexedRow {
                file_id: item.file_id,
                root_id: item.root_id,
                kind: item.kind.clone(),
                modality: item.modality.clone(),
                summary: item
                    .summary
                    .clone()
                    .unwrap_or_else(|| item.modality.clone()),
                vector: vector.clone(),
            })
        })
        .collect::<Vec<_>>();

    tauri::async_runtime::block_on(async {
        handle.upsert_rows(&enriched_items).await?;
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(items
        .iter()
        .map(|item| IndexedSemanticRecord {
            file_id: item.file_id,
            status: "indexed".to_string(),
            modality: Some(item.modality.clone()),
            model: Some(CLIP_MODEL_NAME.to_string()),
            summary: item.summary.clone(),
            error_message: None,
        })
        .collect())
}

pub fn build_index_item(source: &SemanticSourceFile) -> Option<SemanticIndexItem> {
    if source.kind == "image" {
        return Some(SemanticIndexItem {
            file_id: source.file_id,
            root_id: source.root_id,
            kind: source.kind.clone(),
            modality: "image".to_string(),
            summary: source
                .summary
                .clone()
                .or_else(|| Some("Visual features".to_string())),
            payload: SemanticPayload::Image(PathBuf::from(&source.path)),
        });
    }

    if matches!(source.kind.as_str(), "document" | "text" | "code") {
        let text = source.content_text.as_ref()?.trim();
        if text.is_empty() {
            return None;
        }

        return Some(SemanticIndexItem {
            file_id: source.file_id,
            root_id: source.root_id,
            kind: source.kind.clone(),
            modality: "text".to_string(),
            summary: source
                .summary
                .clone()
                .or_else(|| Some("Semantic text preview".to_string())),
            payload: SemanticPayload::Text(truncate_chars(text, MAX_TEXT_CHARS)),
        });
    }

    None
}

pub fn build_index_item_for_file(
    file_id: i64,
    root_id: i64,
    path: &Path,
    kind: &str,
    content: Option<&ExtractionOutput>,
) -> Option<SemanticIndexItem> {
    let summary = content
        .and_then(|output| output.chunks.first())
        .and_then(|chunk| chunk.source_label.clone())
        .or_else(|| Some(default_summary_for_kind(kind).to_string()));

    let content_text = content.and_then(|output| {
        let text = output
            .chunks
            .iter()
            .take(6)
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    });

    let source = SemanticSourceFile {
        file_id,
        root_id,
        path: path.to_string_lossy().into_owned(),
        kind: kind.to_string(),
        summary,
        content_text,
    };

    build_index_item(&source)
}

pub fn remove_root_embeddings(vector_db_path: &Path, root_id: i64) -> Result<()> {
    tauri::async_runtime::block_on(async move {
        let connection = connect_to_db(vector_db_path).await?;
        if let Ok(table) = connection.open_table(TABLE_NAME).execute().await {
            let filter = format!("root_id = {root_id}");
            table.delete(&filter).await.map_err(anyhow::Error::from)?;
        }
        Ok::<(), anyhow::Error>(())
    })
}

pub fn remove_embeddings_for_files(vector_db_path: &Path, file_ids: &[i64]) -> Result<()> {
    if file_ids.is_empty() {
        return Ok(());
    }

    tauri::async_runtime::block_on(async move {
        let connection = connect_to_db(vector_db_path).await?;
        if let Ok(table) = connection.open_table(TABLE_NAME).execute().await {
            for chunk in file_ids.chunks(200) {
                let filter = format!(
                    "file_id IN ({})",
                    chunk
                        .iter()
                        .map(|file_id| file_id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                table.delete(&filter).await.map_err(anyhow::Error::from)?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })
}

pub fn search_semantic(
    vector_db_path: &Path,
    model_cache_dir: &Path,
    query: &str,
    root_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<SemanticMatch>> {
    let query_embedding = with_models(model_cache_dir, |models| {
        let model = ensure_text_model(models, model_cache_dir)?;
        let mut embeddings = model.embed(vec![query.to_string()], None)?;
        embeddings
            .pop()
            .ok_or_else(|| anyhow!("semantic model returned no query embedding"))
    })?;
    validate_dimensions(&query_embedding)?;

    let allowed_roots = root_ids
        .map(|ids| ids.iter().copied().collect::<HashSet<_>>())
        .unwrap_or_default();

    tauri::async_runtime::block_on(async move {
        let connection = connect_to_db(vector_db_path).await?;
        let table = match connection.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(_) => return Ok(Vec::new()),
        };

        let batches: Vec<RecordBatch> = table
            .query()
            .nearest_to(query_embedding.as_slice())?
            .limit(limit.max(8) * 6)
            .execute()
            .await?
            .try_collect::<Vec<_>>()
            .await?;

        let mut matches = Vec::new();
        let mut seen = HashSet::new();

        for batch in batches {
            let file_ids = batch
                .column_by_name("file_id")
                .and_then(|column| as_int64_array(column.as_ref()))
                .ok_or_else(|| anyhow!("semantic results missing file_id column"))?;
            let root_ids_array = batch
                .column_by_name("root_id")
                .and_then(|column| as_int64_array(column.as_ref()))
                .ok_or_else(|| anyhow!("semantic results missing root_id column"))?;
            let modalities = batch
                .column_by_name("modality")
                .and_then(|column| as_string_array(column.as_ref()))
                .ok_or_else(|| anyhow!("semantic results missing modality column"))?;
            let distances = batch
                .column_by_name("_distance")
                .and_then(|column| as_float32_array(column.as_ref()))
                .ok_or_else(|| anyhow!("semantic results missing distance column"))?;

            for row in 0..batch.num_rows() {
                let file_id = file_ids.value(row);
                let root_id = root_ids_array.value(row);
                if !allowed_roots.is_empty() && !allowed_roots.contains(&root_id) {
                    continue;
                }
                if !seen.insert(file_id) {
                    continue;
                }

                let distance = distances.value(row);
                matches.push(SemanticMatch {
                    file_id,
                    modality: modalities.value(row).to_string(),
                    similarity: 1.0 / (1.0 + distance),
                });

                if matches.len() >= limit {
                    return Ok(matches);
                }
            }
        }

        Ok(matches)
    })
}

#[derive(Debug, Clone)]
struct IndexedRow {
    file_id: i64,
    root_id: i64,
    kind: String,
    modality: String,
    summary: String,
    vector: Vec<f32>,
}

fn with_models<T>(
    model_cache_dir: &Path,
    run: impl FnOnce(&mut SemanticModels) -> Result<T>,
) -> Result<T> {
    let mut guard = MODELS
        .lock()
        .map_err(|_| anyhow!("semantic model lock was poisoned"))?;

    if guard.cache_dir.as_deref() != Some(model_cache_dir) {
        *guard = SemanticModels {
            cache_dir: Some(model_cache_dir.to_path_buf()),
            text: None,
            image: None,
        };
    }

    run(&mut guard)
}

fn ensure_text_model<'a>(
    models: &'a mut SemanticModels,
    model_cache_dir: &Path,
) -> Result<&'a mut TextEmbedding> {
    if models.text.is_none() {
        models.text = Some(
            TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::ClipVitB32)
                    .with_cache_dir(model_cache_dir.to_path_buf())
                    .with_show_download_progress(false),
            )
            .context("failed to initialize CLIP text embedding model")?,
        );
    }

    models
        .text
        .as_mut()
        .ok_or_else(|| anyhow!("semantic text model is unavailable"))
}

fn ensure_image_model<'a>(
    models: &'a mut SemanticModels,
    model_cache_dir: &Path,
) -> Result<&'a mut ImageEmbedding> {
    if models.image.is_none() {
        models.image = Some(
            ImageEmbedding::try_new(
                ImageInitOptions::new(ImageEmbeddingModel::ClipVitB32)
                    .with_cache_dir(model_cache_dir.to_path_buf())
                    .with_show_download_progress(false),
            )
            .context("failed to initialize CLIP image embedding model")?,
        );
    }

    models
        .image
        .as_mut()
        .ok_or_else(|| anyhow!("semantic image model is unavailable"))
}

async fn connect_to_db(vector_db_path: &Path) -> Result<Connection> {
    connect(vector_db_path.to_string_lossy().as_ref())
        .execute()
        .await
        .map_err(Into::into)
}

impl SemanticIndexHandle {
    async fn upsert_rows(&mut self, items: &[IndexedRow]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let table = self.ensure_table(items).await?;
        let batch = build_record_batch(items)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);
        table.add(reader).execute().await?;
        Ok(())
    }

    async fn ensure_table(&mut self, seed_items: &[IndexedRow]) -> Result<Table> {
        if let Some(table) = &self.table {
            return Ok(table.clone());
        }

        let batch = build_record_batch(seed_items)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);
        let table = self
            .connection
            .create_table(TABLE_NAME, reader)
            .execute()
            .await?;
        self.table = Some(table.clone());
        Ok(table)
    }
}

fn build_record_batch(items: &[IndexedRow]) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("file_id", DataType::Int64, false),
        Field::new("root_id", DataType::Int64, false),
        Field::new("kind", DataType::Utf8, false),
        Field::new("modality", DataType::Utf8, false),
        Field::new("summary", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                VECTOR_DIMENSIONS,
            ),
            false,
        ),
    ]));

    let file_ids = Int64Array::from(items.iter().map(|item| item.file_id).collect::<Vec<_>>());
    let root_ids = Int64Array::from(items.iter().map(|item| item.root_id).collect::<Vec<_>>());
    let kinds = StringArray::from(
        items
            .iter()
            .map(|item| item.kind.as_str())
            .collect::<Vec<_>>(),
    );
    let modalities = StringArray::from(
        items
            .iter()
            .map(|item| item.modality.as_str())
            .collect::<Vec<_>>(),
    );
    let summaries = StringArray::from(
        items
            .iter()
            .map(|item| item.summary.as_str())
            .collect::<Vec<_>>(),
    );
    let vectors = FixedSizeListArray::from_iter_primitive::<arrow_array::types::Float32Type, _, _>(
        items
            .iter()
            .map(|item| Some(item.vector.iter().copied().map(Some))),
        VECTOR_DIMENSIONS,
    );

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(file_ids),
            Arc::new(root_ids),
            Arc::new(kinds),
            Arc::new(modalities),
            Arc::new(summaries),
            Arc::new(vectors),
        ],
    )
    .map_err(Into::into)
}

fn validate_dimensions(embedding: &[f32]) -> Result<()> {
    if embedding.len() != VECTOR_DIMENSIONS as usize {
        return Err(anyhow!(
            "semantic embedding dimension mismatch: expected {}, got {}",
            VECTOR_DIMENSIONS,
            embedding.len()
        ));
    }
    Ok(())
}

fn default_summary_for_kind(kind: &str) -> &'static str {
    match kind {
        "image" => "Visual features",
        "document" | "text" | "code" => "Semantic text preview",
        _ => "Semantic preview",
    }
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }

    let mut truncated = text.chars().take(limit).collect::<String>();
    truncated.push('…');
    truncated
}

fn as_int64_array(column: &dyn Array) -> Option<&Int64Array> {
    column.as_any().downcast_ref::<Int64Array>()
}

fn as_string_array(column: &dyn Array) -> Option<&StringArray> {
    column.as_any().downcast_ref::<StringArray>()
}

fn as_float32_array(column: &dyn Array) -> Option<&Float32Array> {
    column.as_any().downcast_ref::<Float32Array>()
}
