use super::Resolved;
use sha2::{Digest, Sha256};

pub(crate) fn symbol_hash(src: &str, r: &Resolved) -> String {
    match &r.body {
        Some(b) if b.start >= r.range.start && b.end <= r.range.end => {
            let sig = format!(
                "{}{}",
                &src[r.range.start..b.start],
                &src[b.end..r.range.end]
            );
            format!("{}:{}", hash(&sig), hash(&src[b.clone()]))
        }
        _ => hash(&src[r.range.clone()]),
    }
}

pub(crate) fn symbol_source<'a>(src: &'a str, r: &Resolved) -> &'a str {
    &src[r.range.clone()]
}

fn hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    hex(&out[..8])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
