use crate::config::{is_excluded, load_config};
use crate::lockfile::{load_lock, lock_path, save_lock, LockEntry, Lockfile};
use crate::refs::{extract_refs, line_of, md_files, MdRef};
use crate::resolve::{
    language_for, parse_tree, resolve_symbol_in_tree, short_hash, symbol_hash, symbol_source,
    ResolveError,
};
use crate::TOOL_NAME;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug)]
pub(crate) enum RefStatus {
    Ok,
    FileMissing,
    SymbolMissing,
    AmbiguousSymbol,
    SigDrift {
        expected: String,
        actual: String,
        doc_reviewed: bool,
    },
    BodyDrift {
        expected: String,
        actual: String,
        doc_reviewed: bool,
    },
    Unlocked {
        actual: String,
    },
}

/// True when the doc section covering a ref has visibly changed since the
/// last time this exact ref was locked. A locked entry with no doc hash
/// (older lockfile format) is treated as never-reviewed, matching the
/// stricter pre-existing behavior.
fn doc_reviewed_since_lock(entry: &LockEntry, current_doc_hash: &str) -> bool {
    !entry.doc_hash.is_empty() && entry.doc_hash != current_doc_hash
}

/// Lockfile state is tracked per (doc, ref) pair rather than per ref alone,
/// so that reviewing one doc's mention of a symbol doesn't silently mark a
/// different doc's mention of the same symbol as reviewed too.
pub(crate) fn doc_lock_key(doc_rel: &str, r: &MdRef) -> String {
    format!("{doc_rel} :: {}", r.key())
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
    source_overrides: HashMap<String, String>,
}

impl CheckContext {
    pub(crate) fn with_source_overrides(source_overrides: HashMap<String, String>) -> Self {
        Self {
            files: HashMap::new(),
            source_overrides,
        }
    }

    fn source_file(&mut self, root: &Path, r: &MdRef) -> Result<&SourceFile, RefStatus> {
        let target = root.join(&r.file);
        let override_src = self.source_overrides.get(&r.file).cloned();
        let entry = self.files.entry(r.file.clone()).or_insert_with(|| {
            let src = if let Some(src) = override_src {
                src
            } else {
                let Ok(src) = std::fs::read_to_string(&target) else {
                    return SourceCacheEntry::FileMissing;
                };
                src
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
        lock_key: &str,
        doc_hash: &str,
        lock: Option<&Lockfile>,
    ) -> RefStatus {
        let source = match self.source_file(root, r) {
            Ok(source) => source,
            Err(status) => return status,
        };
        let res = match resolve_symbol_in_tree(&source.tree, &source.src, &r.symbol) {
            Ok(res) => res,
            Err(ResolveError::NotFound) => return RefStatus::SymbolMissing,
            Err(ResolveError::Ambiguous) => return RefStatus::AmbiguousSymbol,
        };
        let actual = symbol_hash(&source.src, &res);
        match lock.and_then(|l| l.refs.get(lock_key)) {
            Some(entry) if entry.hash == actual => RefStatus::Ok,
            Some(entry) => {
                let doc_reviewed = doc_reviewed_since_lock(entry, doc_hash);
                let (es, eb) = entry
                    .hash
                    .split_once(':')
                    .unwrap_or((entry.hash.as_str(), ""));
                let (as_, ab) = actual.split_once(':').unwrap_or((actual.as_str(), ""));
                if !eb.is_empty() && !ab.is_empty() && es == as_ && eb != ab {
                    RefStatus::BodyDrift {
                        expected: entry.hash.clone(),
                        actual,
                        doc_reviewed,
                    }
                } else {
                    RefStatus::SigDrift {
                        expected: entry.hash.clone(),
                        actual,
                        doc_reviewed,
                    }
                }
            }
            None => RefStatus::Unlocked { actual },
        }
    }

    pub(crate) fn symbol_source(&mut self, root: &Path, r: &MdRef) -> Option<String> {
        let source = self.source_file(root, r).ok()?;
        let res = resolve_symbol_in_tree(&source.tree, &source.src, &r.symbol).ok()?;
        Some(symbol_source(&source.src, &res).to_string())
    }
}

pub(crate) fn run_check(
    root: &Path,
    write_lock: bool,
    warn_body: bool,
    json: bool,
    scope: &[String],
) -> i32 {
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
    let mut locked_count = 0usize;
    let mut relocked_count = 0usize;
    let mut relocked_without_doc_edit = 0usize;
    // Scoped updates only touch lock entries for docs actually visited, so
    // start from the existing state instead of rebuilding it from scratch.
    let mut new_lock = if scope.is_empty() {
        Lockfile::default()
    } else {
        Lockfile {
            refs: lock.refs.clone(),
        }
    };
    let mut context = CheckContext::default();
    let config = load_config(root);
    for md_path in md_files(root) {
        let rel = md_path
            .strip_prefix(root)
            .unwrap_or(&md_path)
            .display()
            .to_string();
        if is_excluded(&rel, &config.exclude) {
            continue;
        }
        if !scope.is_empty() && !scope.iter().any(|p| rel.starts_with(p.as_str())) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&md_path) else {
            continue;
        };
        if !scope.is_empty() {
            let prefix = format!("{rel} :: ");
            new_lock.refs.retain(|k, _| !k.starts_with(prefix.as_str()));
        }
        let doc_dir = Path::new(&rel)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        for r in extract_refs(&text, &doc_dir) {
            let line = line_of(&text, r.span.start);
            let (heading, section) = enclosing_section(&text, r.span.start);
            let doc_hash = short_hash(&section);
            let lock_key = doc_lock_key(&rel, &r);
            let status = context.check_ref(root, &r, &lock_key, &doc_hash, Some(&lock));
            if json && !write_lock {
                let record_meta = match &status {
                    RefStatus::Ok => None,
                    RefStatus::FileMissing => {
                        Some(("file_missing", "error", true, None, None, None))
                    }
                    RefStatus::SymbolMissing => {
                        Some(("symbol_missing", "error", true, None, None, None))
                    }
                    RefStatus::AmbiguousSymbol => {
                        Some(("ambiguous_symbol", "error", true, None, None, None))
                    }
                    RefStatus::SigDrift {
                        expected,
                        actual,
                        doc_reviewed,
                    } => Some((
                        "sig_drift",
                        "error",
                        true,
                        Some(expected),
                        Some(actual),
                        Some(*doc_reviewed),
                    )),
                    RefStatus::BodyDrift {
                        expected,
                        actual,
                        doc_reviewed,
                    } => {
                        let (severity, blocks_exit) = if warn_body || *doc_reviewed {
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
                            Some(*doc_reviewed),
                        ))
                    }
                    RefStatus::Unlocked { actual } => {
                        Some(("unlocked", "error", true, None, Some(actual), None))
                    }
                };
                if let Some((
                    kind,
                    severity,
                    blocks_exit,
                    expected_hash,
                    actual_hash,
                    doc_reviewed,
                )) = record_meta
                {
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
                        "doc_reviewed": doc_reviewed,
                        "doc": { "file": rel, "line": line, "heading": heading, "section": section },
                        "symbol_source": context.symbol_source(root, &r),
                    }));
                }
            }
            match status {
                RefStatus::Ok => {
                    if write_lock {
                        let mut entry = lock.refs[&lock_key].clone();
                        entry.doc_hash = doc_hash.clone();
                        new_lock.refs.insert(lock_key.clone(), entry);
                    }
                }
                RefStatus::Unlocked { actual } => {
                    if write_lock {
                        new_lock.refs.insert(
                            lock_key.clone(),
                            LockEntry {
                                hash: actual,
                                doc_hash: doc_hash.clone(),
                            },
                        );
                        locked_count += 1;
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
                RefStatus::SigDrift {
                    expected,
                    actual,
                    doc_reviewed,
                } => {
                    if write_lock {
                        new_lock.refs.insert(
                            lock_key.clone(),
                            LockEntry {
                                hash: actual,
                                doc_hash: doc_hash.clone(),
                            },
                        );
                        relocked_count += 1;
                        if !doc_reviewed {
                            relocked_without_doc_edit += 1;
                        }
                        println!("relocked {}:{} {}", rel, line, r.key());
                    } else {
                        failures += 1;
                        let hint = if doc_reviewed {
                            "doc section already edited; run"
                        } else {
                            "update docs, then run"
                        };
                        eprintln!(
                            "error    {}:{} {} signature changed ({} -> {}); {} `{} update`",
                            rel,
                            line,
                            r.key(),
                            expected,
                            actual,
                            hint,
                            TOOL_NAME
                        );
                    }
                }
                RefStatus::BodyDrift {
                    expected,
                    actual,
                    doc_reviewed,
                } => {
                    if write_lock {
                        new_lock.refs.insert(
                            lock_key.clone(),
                            LockEntry {
                                hash: actual,
                                doc_hash: doc_hash.clone(),
                            },
                        );
                        relocked_count += 1;
                        if !doc_reviewed {
                            relocked_without_doc_edit += 1;
                        }
                        println!("relocked {}:{} {}", rel, line, r.key());
                    } else if warn_body || doc_reviewed {
                        let reason = if doc_reviewed {
                            "doc section already edited"
                        } else {
                            "--warn-body"
                        };
                        eprintln!(
                            "warning  {}:{} {} body changed ({} -> {}) [{}]; run `{} update`",
                            rel,
                            line,
                            r.key(),
                            expected,
                            actual,
                            reason,
                            TOOL_NAME
                        );
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
                RefStatus::AmbiguousSymbol => {
                    failures += 1;
                    eprintln!(
                        "error    {}:{} {} symbol is ambiguous (matches multiple definitions)",
                        rel,
                        line,
                        r.key()
                    );
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
        println!(
            "wrote {} ({} locked, {} relocked, {} relocked without a doc edit)",
            lock_path(root).display(),
            locked_count,
            relocked_count,
            relocked_without_doc_edit
        );
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

pub(crate) fn enclosing_section(md: &str, offset: usize) -> (Option<String>, String) {
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

    #[test]
    fn check_context_can_resolve_unsaved_source_overrides() {
        let dir = tempfile::tempdir().expect("create temp root");
        let mut overrides = HashMap::new();
        overrides.insert(
            "src/lib.rs".to_string(),
            "pub fn login() -> bool {\n    true\n}\n".to_string(),
        );
        let mut context = CheckContext::with_source_overrides(overrides);
        let reference = MdRef {
            file: "src/lib.rs".to_string(),
            symbol: "login".to_string(),
            span: 0..0,
        };

        let status = context.check_ref(
            dir.path(),
            &reference,
            "README.md :: src/lib.rs#login",
            "doc-hash",
            None,
        );

        assert!(matches!(status, RefStatus::Unlocked { .. }));
    }
}
