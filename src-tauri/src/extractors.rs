use anyhow::{Context, Result};
use pdf_oxide::{parallel::extract_all_text_parallel, PdfDocument};
use std::{
    fs,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

const CHUNK_WORD_TARGET: usize = 180;
const CHUNK_WORD_OVERLAP: usize = 45;
const MIN_CHUNK_CHARS: usize = 20;
const PDF_PARALLEL_PAGE_THRESHOLD: usize = 48;

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub source_label: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ExtractionOutput {
    pub status: String,
    pub extractor: Option<String>,
    pub text_length: i64,
    pub chunks: Vec<TextChunk>,
    pub error_message: Option<String>,
}

pub fn placeholder_output(kind: &str, extension: &str) -> ExtractionOutput {
    if supports_content_extraction(kind, extension) {
        ExtractionOutput {
            status: "pending".to_string(),
            extractor: Some(extractor_name(extension, kind).to_string()),
            text_length: 0,
            chunks: Vec::new(),
            error_message: None,
        }
    } else {
        unsupported_output()
    }
}

pub fn supports_content_extraction(kind: &str, extension: &str) -> bool {
    matches!(extension, "pdf" | "docx" | "pptx" | "xlsx") || matches!(kind, "text" | "code")
}

pub fn extract_file_text(path: &Path, kind: &str, extension: &str) -> ExtractionOutput {
    let result = match extension {
        "pdf" => extract_pdf(path),
        "docx" => extract_docx(path),
        "pptx" => extract_pptx(path),
        "xlsx" => extract_xlsx(path),
        _ if matches!(kind, "text" | "code") => extract_plain_text(path),
        _ => return unsupported_output(),
    };

    match result {
        Ok(sections) => build_output(sections, extractor_name(extension, kind)),
        Err(error) => ExtractionOutput {
            status: "error".to_string(),
            extractor: Some(extractor_name(extension, kind).to_string()),
            text_length: 0,
            chunks: Vec::new(),
            error_message: Some(error.to_string()),
        },
    }
}

fn extract_plain_text(path: &Path) -> Result<Vec<(Option<String>, String)>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if looks_binary(&bytes) {
        return Ok(Vec::new());
    }

    let text = normalize_whitespace(&String::from_utf8_lossy(&bytes));
    Ok(vec![(Some("Document".to_string()), text)])
}

fn extract_pdf(path: &Path) -> Result<Vec<(Option<String>, String)>> {
    let mut document = PdfDocument::open(path)
        .with_context(|| format!("failed to open pdf {}", path.display()))?;
    let page_count = document
        .page_count()
        .with_context(|| format!("failed to read pdf page count for {}", path.display()))?;

    let pages = if page_count >= PDF_PARALLEL_PAGE_THRESHOLD {
        drop(document);
        extract_all_text_parallel(path)
            .with_context(|| format!("failed to extract pdf text from {}", path.display()))?
    } else {
        let mut pages = Vec::with_capacity(page_count);
        for page_index in 0..page_count {
            let page_text = document.extract_text(page_index).with_context(|| {
                format!(
                    "failed to extract pdf text from {} page {}",
                    path.display(),
                    page_index + 1
                )
            })?;
            pages.push(page_text);
        }
        pages
    };

    Ok(pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            (
                Some(format!("Page {}", index + 1)),
                normalize_whitespace(&page),
            )
        })
        .collect())
}

fn extract_docx(path: &Path) -> Result<Vec<(Option<String>, String)>> {
    let mut archive = open_zip(path)?;
    let mut entries = archive
        .file_names()
        .map(|name| name.to_string())
        .filter(|name| {
            name == "word/document.xml"
                || name.starts_with("word/header")
                || name.starts_with("word/footer")
                || name == "word/footnotes.xml"
                || name == "word/endnotes.xml"
        })
        .collect::<Vec<_>>();
    entries.sort();

    let mut sections = Vec::new();
    for entry in entries {
        let xml = read_zip_entry(&mut archive, &entry)?;
        let text = normalize_whitespace(&strip_xml_tags(&xml));
        if text.is_empty() {
            continue;
        }

        let label = if entry == "word/document.xml" {
            "Document".to_string()
        } else if entry.starts_with("word/header") {
            "Header".to_string()
        } else if entry.starts_with("word/footer") {
            "Footer".to_string()
        } else {
            "Notes".to_string()
        };

        sections.push((Some(label), text));
    }

    Ok(sections)
}

fn extract_pptx(path: &Path) -> Result<Vec<(Option<String>, String)>> {
    let mut archive = open_zip(path)?;
    let mut entries = archive
        .file_names()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .collect::<Vec<_>>();
    entries.sort();

    let mut sections = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let xml = read_zip_entry(&mut archive, entry)?;
        let text = normalize_whitespace(&strip_xml_tags(&xml));
        if text.is_empty() {
            continue;
        }

        sections.push((Some(format!("Slide {}", index + 1)), text));
    }

    Ok(sections)
}

fn extract_xlsx(path: &Path) -> Result<Vec<(Option<String>, String)>> {
    let mut archive = open_zip(path)?;
    let mut entries = archive
        .file_names()
        .map(|name| name.to_string())
        .filter(|name| {
            name == "xl/sharedStrings.xml"
                || (name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
        })
        .collect::<Vec<_>>();
    entries.sort();

    let mut sections = Vec::new();
    for entry in entries {
        let xml = read_zip_entry(&mut archive, &entry)?;
        let text = normalize_whitespace(&strip_xml_tags(&xml));
        if text.is_empty() {
            continue;
        }

        let label = if entry == "xl/sharedStrings.xml" {
            "Workbook text".to_string()
        } else {
            let stem = PathBuf::from(&entry)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("sheet")
                .replace("sheet", "Sheet ");
            stem
        };

        sections.push((Some(label), text));
    }

    Ok(sections)
}

fn open_zip(path: &Path) -> Result<ZipArchive<File>> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    ZipArchive::new(file).with_context(|| format!("failed to read zip archive {}", path.display()))
}

fn read_zip_entry(archive: &mut ZipArchive<File>, entry_name: &str) -> Result<String> {
    let mut entry = archive
        .by_name(entry_name)
        .with_context(|| format!("failed to read zip entry {entry_name}"))?;
    let mut buffer = String::new();
    entry
        .read_to_string(&mut buffer)
        .with_context(|| format!("failed to decode zip entry {entry_name}"))?;
    Ok(buffer)
}

fn build_output(sections: Vec<(Option<String>, String)>, extractor: &str) -> ExtractionOutput {
    let mut chunks = Vec::new();
    for (source_label, text) in sections {
        chunks.extend(chunk_text(source_label, &text));
    }

    let text_length = chunks
        .iter()
        .map(|chunk| chunk.text.len() as i64)
        .sum::<i64>();
    if chunks.is_empty() {
        return ExtractionOutput {
            status: "empty".to_string(),
            extractor: None,
            text_length: 0,
            chunks,
            error_message: None,
        };
    }

    ExtractionOutput {
        status: "indexed".to_string(),
        extractor: Some(extractor.to_string()),
        text_length,
        chunks,
        error_message: None,
    }
}

fn unsupported_output() -> ExtractionOutput {
    ExtractionOutput {
        status: "unsupported".to_string(),
        extractor: None,
        text_length: 0,
        chunks: Vec::new(),
        error_message: None,
    }
}

fn extractor_name(extension: &str, kind: &str) -> &'static str {
    match extension {
        "pdf" => "pdf_oxide",
        "docx" | "pptx" | "xlsx" => "ooxml-zip",
        _ if matches!(kind, "text" | "code") => "plain-text",
        _ => "unsupported",
    }
}

fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(512)];
    let nulls = sample.iter().filter(|byte| **byte == 0).count();
    nulls > 4
}

fn chunk_text(source_label: Option<String>, text: &str) -> Vec<TextChunk> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + CHUNK_WORD_TARGET).min(words.len());
        let chunk_text = words[start..end].join(" ");
        if chunk_text.len() >= MIN_CHUNK_CHARS {
            chunks.push(TextChunk {
                source_label: source_label.clone(),
                text: chunk_text,
            });
        }

        if end == words.len() {
            break;
        }

        start = end.saturating_sub(CHUNK_WORD_OVERLAP);
    }

    chunks
}

fn normalize_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_was_whitespace = false;

    for character in input.chars() {
        if character.is_whitespace() {
            if !last_was_whitespace {
                result.push(' ');
                last_was_whitespace = true;
            }
        } else {
            result.push(character);
            last_was_whitespace = false;
        }
    }

    result.trim().to_string()
}

fn strip_xml_tags(xml: &str) -> String {
    let mut output = String::with_capacity(xml.len());
    let mut in_tag = false;

    for character in xml.chars() {
        match character {
            '<' => {
                in_tag = true;
                output.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }

    decode_xml_entities(&output)
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", " ")
        .replace("&#13;", " ")
}
