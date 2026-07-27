use super::Resolved;
use sha2::{Digest, Sha256};

pub(crate) fn symbol_hash(src: &str, r: &Resolved) -> String {
    match &r.body {
        Some(b) if b.start >= r.range.start && b.end <= r.range.end => {
            let sig_pre = strip_ranges(
                &src[r.range.start..b.start],
                r.range.start,
                &r.comment_ranges,
            );
            let sig_post = strip_ranges(&src[b.end..r.range.end], b.end, &r.comment_ranges);
            let sig = normalize_whitespace(&format!("{sig_pre}{sig_post}"));
            let body_stripped = strip_ranges(&src[b.clone()], b.start, &r.comment_ranges);
            let body_text = normalize_whitespace(&body_stripped);
            format!("{}:{}", short_hash(&sig), short_hash(&body_text))
        }
        _ => short_hash(&src[r.range.clone()]),
    }
}

pub(crate) fn symbol_source<'a>(src: &'a str, r: &Resolved) -> &'a str {
    &src[r.range.clone()]
}

/// Removes byte ranges (e.g. comment nodes) from `text`, where `ranges` are
/// absolute offsets into the original source and `text` starts at `text_start`.
fn strip_ranges(text: &str, text_start: usize, ranges: &[std::ops::Range<usize>]) -> String {
    if ranges.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for r in ranges {
        let start = r.start.saturating_sub(text_start);
        let end = r.end.saturating_sub(text_start);
        if start > text.len() || end > text.len() || start < cursor {
            continue;
        }
        out.push_str(&text[cursor..start]);
        cursor = end;
    }
    out.push_str(&text[cursor.min(text.len())..]);
    out
}

/// Collapses runs of whitespace to a single space so indentation and blank
/// lines left behind by removing comments (or plain reformatting) don't
/// register as body drift.
fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_whitespace = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_whitespace = true;
            continue;
        }
        if in_whitespace && !out.is_empty() {
            out.push(' ');
        }
        in_whitespace = false;
        out.push(c);
    }
    out
}

pub(crate) fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    hex(&out[..8])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
