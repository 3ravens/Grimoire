/// Split a long line into sentences at punctuation boundaries.
/// Only splits when a sentence-ending character (`.`, `!`, `?`) is followed
/// by whitespace and then an uppercase letter — i.e. a real sentence boundary.
fn split_at_punctuation(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        buf.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let next = chars.get(i + 1).copied();
            let after = chars.get(i + 2).copied();
            if matches!(next, Some(' ') | Some('\t'))
                && matches!(after, Some(c) if c.is_uppercase())
            {
                let s = buf.trim().to_string();
                if !s.is_empty() {
                    parts.push(s);
                }
                buf.clear();
            }
        }
    }

    let tail = buf.trim().to_string();
    if !tail.is_empty() {
        parts.push(tail);
    }
    if parts.is_empty() {
        parts.push(text.trim().to_string());
    }
    parts
}

/// Split `text` into individual sentences.
/// First splits on newlines (one idea per line is common in notes), then
/// further splits long lines at sentence-ending punctuation.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.split_whitespace().count() > 30 {
            sentences.extend(split_at_punctuation(line));
        } else {
            sentences.push(line.to_string());
        }
    }

    if sentences.is_empty() {
        let s = text.trim().to_string();
        if !s.is_empty() {
            sentences.push(s);
        }
    }
    sentences
}

/// Group a flat list of sentences into overlapping chunks.
/// `per_chunk` is the number of sentences per chunk; `overlap` is how many
/// sentences the next chunk re-uses from the end of the previous one.
pub fn chunk_sentences(
    sentences: Vec<String>,
    per_chunk: usize,
    overlap: usize,
) -> Vec<String> {
    if sentences.is_empty() {
        return vec![String::new()];
    }
    if sentences.len() <= per_chunk {
        return vec![sentences.join(" ")];
    }
    let step = per_chunk.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + per_chunk).min(sentences.len());
        chunks.push(sentences[start..end].join(" "));
        if end == sentences.len() {
            break;
        }
        start += step;
    }
    chunks
}

/// Target size for CSV row blocks (characters). Keeps embeddings within a reasonable budget.
pub const CSV_CHUNK_MAX_CHARS: usize = 6000;

/// Pack CSV rows into text blocks without splitting a row across chunks. Each row is one line
/// (tab-separated cells).
pub fn chunk_csv_row_blocks(rows: Vec<String>, max_chunk_chars: usize) -> Vec<String> {
    if rows.is_empty() {
        return vec![];
    }
    let max = max_chunk_chars.max(256);
    let mut blocks = Vec::new();
    let mut cur = String::new();

    for row in rows {
        let next_len = if cur.is_empty() {
            row.len()
        } else {
            cur.len() + 1 + row.len()
        };

        if next_len > max && !cur.is_empty() {
            blocks.push(std::mem::take(&mut cur));
            cur = row;
        } else if cur.is_empty() {
            cur = row;
        } else {
            cur.push('\n');
            cur.push_str(&row);
        }
    }

    if !cur.trim().is_empty() {
        blocks.push(cur);
    }

    blocks
}
