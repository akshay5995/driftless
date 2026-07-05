use crate::lockfile::{load_lock, lock_path, save_lock, Lockfile};
use crate::refs::{extract_refs, line_of, md_files, MdRef};
use crate::resolve::{
    language_for, parse_tree, resolve_symbol_in_tree, symbol_hash, symbol_source,
};
use crate::TOOL_NAME;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug)]
pub(crate) enum RefStatus {
    Ok,
    FileMissing,
    SymbolMissing,
    SigDrift { expected: String, actual: String },
    BodyDrift { expected: String, actual: String },
    Unlocked { actual: String },
}

struct SourceFile {
    src: String,
    tree: tree_sitter::Tree,
}

enum SourceCacheEntry {
    Ready(SourceFile),
    FileMissing,
    ParseFailed,
}

#[derive(Default)]
pub(crate) struct CheckContext {
    files: HashMap<String, SourceCacheEntry>,
}

impl CheckContext {
    fn source_file(&mut self, root: &Path, r: &MdRef) -> Result<&SourceFile, RefStatus> {
        let target = root.join(&r.file);
        let entry = self.files.entry(r.file.clone()).or_insert_with(|| {
            let Ok(src) = std::fs::read_to_string(&target) else {
                return SourceCacheEntry::FileMissing;
            };
            let Some(lang) = language_for(&target) else {
                return SourceCacheEntry::FileMissing;
            };
            let Some(tree) = parse_tree(lang, &src) else {
                return SourceCacheEntry::ParseFailed;
            };
            SourceCacheEntry::Ready(SourceFile { src, tree })
        });

        match entry {
            SourceCacheEntry::Ready(file) => Ok(file),
            SourceCacheEntry::FileMissing => Err(RefStatus::FileMissing),
            SourceCacheEntry::ParseFailed => Err(RefStatus::SymbolMissing),
        }
    }

    pub(crate) fn check_ref(
        &mut self,
        root: &Path,
        r: &MdRef,
        lock: Option<&Lockfile>,
    ) -> RefStatus {
        let source = match self.source_file(root, r) {
            Ok(source) => source,
            Err(status) => return status,
        };
        let Some(res) = resolve_symbol_in_tree(&source.tree, &source.src, &r.symbol) else {
            return RefStatus::SymbolMissing;
        };
        let actual = symbol_hash(&source.src, &res);
        match lock.and_then(|l| l.refs.get(&r.key())) {
            Some(expected) if *expected == actual => RefStatus::Ok,
            Some(expected) => {
                let (es, eb) = expected.split_once(':').unwrap_or((expected.as_str(), ""));
                let (as_, ab) = actual.split_once(':').unwrap_or((actual.as_str(), ""));
                if !eb.is_empty() && !ab.is_empty() && es == as_ && eb != ab {
                    RefStatus::BodyDrift {
                        expected: expected.clone(),
                        actual,
                    }
                } else {
                    RefStatus::SigDrift {
                        expected: expected.clone(),
                        actual,
                    }
                }
            }
            None => RefStatus::Unlocked { actual },
        }
    }

    pub(crate) fn symbol_source(&mut self, root: &Path, r: &MdRef) -> Option<String> {
        let source = self.source_file(root, r).ok()?;
        let res = resolve_symbol_in_tree(&source.tree, &source.src, &r.symbol)?;
        Some(symbol_source(&source.src, &res).to_string())
    }
}

pub(crate) fn run_check(root: &Path, write_lock: bool, warn_body: bool, json: bool) -> i32 {
    let mut records: Vec<serde_json::Value> = Vec::new();
    let mut lock = match load_lock(root) {
        Ok(lock) => lock,
        Err(err) => {
            eprintln!(
                "error    failed to read {}: {}",
                lock_path(root).display(),
                err
            );
            return 1;
        }
    };
    let mut failures = 0usize;
    let mut new_lock = Lockfile::default();
    let mut context = CheckContext::default();
    for md_path in md_files(root) {
        let Ok(text) = std::fs::read_to_string(&md_path) else {
            continue;
        };
        let rel = md_path
            .strip_prefix(root)
            .unwrap_or(&md_path)
            .display()
            .to_string();
        let doc_dir = Path::new(&rel)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        for r in extract_refs(&text, &doc_dir) {
            let line = line_of(&text, r.span.start);
            let status = context.check_ref(root, &r, Some(&lock));
            if json && !write_lock {
                let record_meta = match &status {
                    RefStatus::Ok => None,
                    RefStatus::FileMissing => Some(("file_missing", "error", true, None, None)),
                    RefStatus::SymbolMissing => Some(("symbol_missing", "error", true, None, None)),
                    RefStatus::SigDrift { expected, actual } => {
                        Some(("sig_drift", "error", true, Some(expected), Some(actual)))
                    }
                    RefStatus::BodyDrift { expected, actual } => {
                        let (severity, blocks_exit) = if warn_body {
                            ("warning", false)
                        } else {
                            ("error", true)
                        };
                        Some((
                            "body_drift",
                            severity,
                            blocks_exit,
                            Some(expected),
                            Some(actual),
                        ))
                    }
                    RefStatus::Unlocked { actual } => {
                        Some(("unlocked", "error", true, None, Some(actual)))
                    }
                };
                if let Some((kind, severity, blocks_exit, expected_hash, actual_hash)) = record_meta
                {
                    let (heading, section) = enclosing_section(&text, r.span.start);
                    records.push(serde_json::json!({
                        "schema_version": 1,
                        "status": kind,
                        "severity": severity,
                        "blocks_exit": blocks_exit,
                        "ref": r.key(),
                        "source_file": r.file.clone(),
                        "symbol": r.symbol.clone(),
                        "expected_hash": expected_hash,
                        "actual_hash": actual_hash,
                        "doc": { "file": rel, "line": line, "heading": heading, "section": section },
                        "symbol_source": context.symbol_source(root, &r),
                    }));
                }
            }
            match status {
                RefStatus::Ok => {
                    new_lock.refs.insert(r.key(), lock.refs[&r.key()].clone());
                }
                RefStatus::Unlocked { actual } => {
                    if write_lock {
                        new_lock.refs.insert(r.key(), actual);
                        println!("locked   {}:{} {}", rel, line, r.key());
                    } else {
                        failures += 1;
                        eprintln!(
                            "error    {}:{} {} not in lockfile (run `{} update`)",
                            rel,
                            line,
                            r.key(),
                            TOOL_NAME
                        );
                    }
                }
                RefStatus::SigDrift { expected, actual } => {
                    if write_lock {
                        new_lock.refs.insert(r.key(), actual);
                        println!("relocked {}:{} {}", rel, line, r.key());
                    } else {
                        failures += 1;
                        eprintln!(
                            "error    {}:{} {} signature changed ({} -> {}); update docs, then `{} update`",
                            rel, line, r.key(), expected, actual, TOOL_NAME
                        );
                    }
                }
                RefStatus::BodyDrift { expected, actual } => {
                    if write_lock {
                        new_lock.refs.insert(r.key(), actual);
                        println!("relocked {}:{} {}", rel, line, r.key());
                    } else if warn_body {
                        eprintln!(
                            "warning  {}:{} {} body changed ({} -> {}); verify prose, then `{} update`",
                            rel, line, r.key(), expected, actual, TOOL_NAME
                        );
                        new_lock.refs.insert(r.key(), lock.refs[&r.key()].clone());
                    } else {
                        failures += 1;
                        eprintln!(
                            "error    {}:{} {} body changed ({} -> {}); update docs, then `{} update`",
                            rel, line, r.key(), expected, actual, TOOL_NAME
                        );
                    }
                }
                RefStatus::FileMissing => {
                    failures += 1;
                    eprintln!("error    {}:{} {} file not found", rel, line, r.key());
                }
                RefStatus::SymbolMissing => {
                    failures += 1;
                    eprintln!("error    {}:{} {} symbol not found", rel, line, r.key());
                }
            }
        }
    }
    if write_lock {
        if failures > 0 {
            eprintln!("{}: {} error(s); lockfile not updated", TOOL_NAME, failures);
            return 1;
        }
        lock.refs = new_lock.refs;
        save_lock(root, &lock).expect("write lockfile");
        println!("wrote {}", lock_path(root).display());
        0
    } else if json {
        println!("{}", serde_json::to_string_pretty(&records).unwrap());
        if failures > 0 {
            1
        } else {
            0
        }
    } else if failures > 0 {
        eprintln!("{}: {} error(s)", TOOL_NAME, failures);
        1
    } else {
        println!("{}: ok", TOOL_NAME);
        0
    }
}

fn enclosing_section(md: &str, offset: usize) -> (Option<String>, String) {
    let mut start = 0usize;
    let mut heading = None;
    let mut pos = 0usize;
    let mut lines: Vec<(usize, &str)> = Vec::new();
    for line in md.split_inclusive('\n') {
        lines.push((pos, line));
        pos += line.len();
    }
    let mut end = md.len();
    for (off, line) in lines {
        if line.trim_start().starts_with('#') {
            if off <= offset {
                start = off;
                heading = Some(line.trim().trim_start_matches('#').trim().to_string());
            } else {
                end = off;
                break;
            }
        }
    }
    (heading, md[start..end].trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enclosing_section_returns_heading_and_current_section() {
        let md = "# First\nA\n\n# Second\nB `src/lib.rs#thing`.\n\n# Third\nC\n";
        let offset = md.find("src/lib.rs#thing").unwrap();

        let (heading, section) = enclosing_section(md, offset);

        assert_eq!(heading.as_deref(), Some("Second"));
        assert_eq!(section, "# Second\nB `src/lib.rs#thing`.");
    }

    #[test]
    fn enclosing_section_without_heading_starts_at_top() {
        let md = "Intro `src/lib.rs#thing`.\n\n# Later\nC\n";
        let offset = md.find("src/lib.rs#thing").unwrap();

        let (heading, section) = enclosing_section(md, offset);

        assert_eq!(heading, None);
        assert_eq!(section, "Intro `src/lib.rs#thing`.");
    }
}
