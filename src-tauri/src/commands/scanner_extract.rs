// Copyright (C) 2026 Wim Palland
//
// This file is part of Grimoire.
//
// Grimoire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Text extraction for the file scanner — one path per format family.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use epub::doc::{EpubDoc, NavPoint};
use roxmltree::Node;
use scraper::{Html, Selector};
use zip::ZipArchive;

use crate::chunking::{self, chunk_csv_row_blocks, CSV_CHUNK_MAX_CHARS};
use crate::{AppError, AppResult};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const TEXT_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";

/// Result of extracting a file: either full prose for sentence chunking, or pre-sized chunks
/// (CSV row blocks; EPUB already split per chapter + sentences).
#[derive(Debug)]
pub enum ScanExtract {
    FullText(String),
    Chunks(Vec<String>),
}

/// Flatten extracted content into a single note body (import-as-note).
pub fn flatten_for_note(ex: &ScanExtract) -> String {
    match ex {
        ScanExtract::FullText(s) => s.clone(),
        ScanExtract::Chunks(parts) => parts.join("\n\n---\n\n"),
    }
}

/// Extract indexable text (and chunking strategy) from a supported file.
pub fn extract(path: &Path) -> AppResult<ScanExtract> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("txt") | Some("md") => Ok(ScanExtract::FullText(read_utf8(path)?)),
        Some("log") => Ok(ScanExtract::FullText(read_log_with_fallback(path)?)),
        Some("pdf") => pdf_to_scan_extract(path),
        Some("csv") => extract_csv(path),
        Some("html") | Some("htm") => Ok(ScanExtract::FullText(html_file_to_structured(path)?)),
        Some("docx") => extract_docx(path).map(ScanExtract::FullText),
        Some("odt") => extract_odt(path).map(ScanExtract::FullText),
        Some("epub") => extract_epub(path),
        Some("rtf") => extract_rtf(path).map(ScanExtract::FullText),
        _ => Err(AppError::InvalidInput(
            "Unsupported file type for extraction".into(),
        )),
    }
}

fn read_utf8(path: &Path) -> AppResult<String> {
    std::fs::read_to_string(path).map_err(Into::into)
}

/// UTF-8 first; if invalid, try Windows-1252 (minimal fallback for `.log` only).
/// `encoding_rs` does not expose ISO-8859-1 (see Encoding Standard); CP1252 is the usual Windows log fallback.
fn read_log_with_fallback(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok(s.to_string());
    }
    let cow = encoding_rs::WINDOWS_1252.decode(&bytes).0;
    let s = cow.into_owned();
    if !s.trim().is_empty() {
        return Ok(s);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn pdf_to_scan_extract(path: &Path) -> AppResult<ScanExtract> {
    let text = pdf_extract::extract_text(path).map_err(|e| AppError::Io(e.to_string()))?;
    if text.trim().is_empty() {
        return Err(AppError::Io(
            "No text could be extracted from this PDF. It may be a scanned image without embedded text."
                .into(),
        ));
    }
    Ok(ScanExtract::FullText(text))
}

fn extract_csv(path: &Path) -> AppResult<ScanExtract> {
    let mut rdr = csv::Reader::from_path(path).map_err(|e| AppError::Io(e.to_string()))?;
    let mut rows: Vec<String> = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Io(e.to_string()))?;
        let line = record
            .iter()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\t");
        rows.push(line);
    }
    if rows.is_empty() {
        return Err(AppError::Io("CSV has no rows.".into()));
    }
    let blocks = chunk_csv_row_blocks(rows, CSV_CHUNK_MAX_CHARS);
    Ok(ScanExtract::Chunks(blocks))
}

fn extract_docx(path: &Path) -> AppResult<String> {
    let file = File::open(path)?;
    let mut archive =
        ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| AppError::Io(e.to_string()))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| AppError::Io("Invalid DOCX: missing word/document.xml".into()))?
        .read_to_string(&mut xml)
        .map_err(|e| AppError::Io(e.to_string()))?;
    extract_wordprocessing_xml(&xml)
}

fn extract_wordprocessing_xml(xml: &str) -> AppResult<String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| AppError::Io(e.to_string()))?;
    let mut out = String::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "p" && node.tag_name().namespace() == Some(W_NS) {
            let mut para = String::new();
            for d in node.descendants() {
                if d.tag_name().name() == "t" && d.tag_name().namespace() == Some(W_NS) {
                    para.push_str(d.text().unwrap_or(""));
                }
            }
            let t = para.trim();
            if !t.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
    }
    if out.trim().is_empty() {
        return Err(AppError::Io("No text found in DOCX.".into()));
    }
    Ok(out)
}

fn extract_odt(path: &Path) -> AppResult<String> {
    let file = File::open(path)?;
    let mut archive =
        ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| AppError::Io(e.to_string()))?;
    let mut xml = String::new();
    archive
        .by_name("content.xml")
        .map_err(|_| AppError::Io("Invalid ODT: missing content.xml".into()))?
        .read_to_string(&mut xml)
        .map_err(|e| AppError::Io(e.to_string()))?;
    extract_odf_content_xml(&xml)
}

fn extract_odf_content_xml(xml: &str) -> AppResult<String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| AppError::Io(e.to_string()))?;
    let mut out = String::new();
    for node in doc.descendants() {
        if node.tag_name().namespace() != Some(TEXT_NS) {
            continue;
        }
        match node.tag_name().name() {
            "h" => {
                let level = node
                    .attribute("outline-level")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 6);
                let title = collect_descendant_text(node);
                let title = title.trim();
                if !title.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(&"#".repeat(level));
                    out.push(' ');
                    out.push_str(title);
                }
            }
            "p" => {
                let t = collect_descendant_text(node);
                let t = t.trim();
                if !t.is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(t);
                }
            }
            _ => {}
        }
    }
    if out.trim().is_empty() {
        return Err(AppError::Io("No text found in ODT.".into()));
    }
    Ok(out)
}

fn collect_descendant_text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .concat()
}

fn html_file_to_structured(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    Ok(html_to_structured_plain(&raw))
}

/// Minimal structure: headings as Markdown `#` lines, blocks separated by blank lines.
pub fn html_to_structured_plain(html: &str) -> String {
    let doc = Html::parse_document(html);
    let sel = match Selector::parse("h1, h2, h3, h4, h5, h6, p, li, td, th") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    let mut out = String::new();
    for el in doc.select(&sel) {
        let name = el.value().name();
        let text = el.text().collect::<Vec<_>>().join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }

        let prefix = match name {
            "h1" => "\n\n# ",
            "h2" => "\n\n## ",
            "h3" => "\n\n### ",
            "h4" => "\n\n#### ",
            "h5" => "\n\n##### ",
            "h6" => "\n\n###### ",
            "li" => "\n- ",
            "td" | "th" => "\n",
            _ => "\n\n",
        };

        if !out.is_empty() || name != "p" {
            out.push_str(prefix);
        } else {
            out.push_str("\n\n");
        }
        out.push_str(&text);
    }

    normalize_blank_lines(out.trim())
}

fn normalize_blank_lines(s: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut prev_blank = false;
    for line in s.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        lines.push(line.to_string());
        prev_blank = blank;
    }
    lines.join("\n")
}

fn toc_label_for_spine_index<R: Read + std::io::Seek>(
    doc: &EpubDoc<R>,
    idx: usize,
) -> Option<String> {
    fn walk<R: Read + std::io::Seek>(
        nav: &[NavPoint],
        doc: &EpubDoc<R>,
        idx: usize,
    ) -> Option<String> {
        for np in nav {
            if let Some(ch) = doc.resource_uri_to_chapter(&np.content) {
                if ch == idx {
                    return Some(np.label.clone());
                }
            }
            if let Some(s) = walk(&np.children, doc, idx) {
                return Some(s);
            }
        }
        None
    }
    walk(&doc.toc, doc, idx)
}

fn extract_epub(path: &Path) -> AppResult<ScanExtract> {
    let mut doc =
        EpubDoc::new(path).map_err(|e| AppError::Io(format!("EPUB: {e}")))?;
    let n = doc.get_num_chapters();
    if n == 0 {
        return Err(AppError::Io("EPUB has no spine chapters.".into()));
    }

    let mut all_chunks: Vec<String> = Vec::new();

    for i in 0..n {
        if !doc.set_current_chapter(i) {
            continue;
        }
        let Some((html, mime)) = doc.get_current_str() else {
            continue;
        };
        if !mime.contains("html") && !mime.contains("xml") && !mime.contains("xhtml") {
            continue;
        }

        let chapter_title = toc_label_for_spine_index(&doc, i)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("Chapter {}", i + 1));

        let body = html_to_structured_plain(&html);
        let chapter_header = format!("\n## {}\n\n", chapter_title.trim());
        let full = format!("{chapter_header}{body}");

        let sentences = chunking::split_sentences(&full);
        // Group sentences so full-length EPUBs do not produce tens of thousands of vectors.
        let mut parts = chunking::chunk_sentences(sentences, 4, 1);
        all_chunks.append(&mut parts);
    }

    if all_chunks.is_empty() {
        return Err(AppError::Io(
            "No readable HTML chapters in EPUB.".into(),
        ));
    }

    Ok(ScanExtract::Chunks(all_chunks))
}

fn extract_rtf(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    let rtf_str = String::from_utf8_lossy(&bytes).into_owned();
    let document = rtf_parser::RtfDocument::try_from(rtf_str)
        .map_err(|e| AppError::Io(format!("RTF: {e}")))?;
    let text = document.get_text();
    if text.trim().is_empty() {
        return Err(AppError::Io("No text in RTF.".into()));
    }
    Ok(text)
}
