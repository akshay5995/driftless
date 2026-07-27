use crate::SOURCE_EXTENSIONS;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser as MdParser, Tag};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct MdRef {
    pub(crate) file: String,
    pub(crate) symbol: String,
    pub(crate) span: std::ops::Range<usize>,
}

impl MdRef {
    pub(crate) fn key(&self) -> String {
        format!("{}#{}", self.file, self.symbol)
    }
}

fn valid_source_path(file: &str) -> bool {
    !file.is_empty()
        && SOURCE_EXTENSIONS
            .iter()
            .any(|e| file.ends_with(&format!(".{}", e)))
        && file
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "./_-".contains(c))
}

fn valid_symbol(sym: &str) -> bool {
    !sym.is_empty()
        && sym.split('.').all(|part| !part.is_empty())
        && sym
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn looks_like_ref(s: &str) -> Option<(String, String)> {
    let (file, sym) = s.split_once('#')?;
    if !valid_source_path(file) || !valid_symbol(sym) {
        return None;
    }
    let norm = normalize(Path::new(file))?;
    Some((norm.to_string_lossy().replace('\\', "/"), sym.to_string()))
}

fn normalize(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => return None,
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

fn link_ref(dest: &str, doc_dir: &Path) -> Option<(String, String)> {
    if dest.contains("://") {
        return None;
    }
    let (file, sym) = dest.split_once('#')?;
    if !valid_source_path(file) || !valid_symbol(sym) {
        return None;
    }
    let norm = normalize(&doc_dir.join(file))?;
    Some((norm.to_string_lossy().replace('\\', "/"), sym.to_string()))
}

pub(crate) fn extract_refs(md: &str, doc_dir: &Path) -> Vec<MdRef> {
    let mut refs = Vec::new();
    let parser = MdParser::new_ext(md, Options::empty());
    for (event, span) in parser.into_offset_iter() {
        match event {
            Event::Code(code) => {
                if let Some((file, symbol)) = looks_like_ref(code.as_ref()) {
                    refs.push(MdRef { file, symbol, span });
                }
            }
            Event::Start(Tag::Link(_, dest, _)) => {
                if let Some((file, symbol)) = link_ref(dest.as_ref(), doc_dir) {
                    refs.push(MdRef {
                        file,
                        symbol,
                        span: span.clone(),
                    });
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                for tok in info.split_whitespace() {
                    if let Some(rest) = tok.strip_prefix("ref=") {
                        if let Some((file, symbol)) = looks_like_ref(rest) {
                            refs.push(MdRef {
                                file,
                                symbol,
                                span: span.clone(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    refs
}

/// Walks the project tree skipping heavy generated folders (`node_modules`,
/// `target`, dotfiles). Shared by every full-repo walk (Markdown files here,
/// source files for `driftless coverage`) so the skip list has one home.
pub(crate) fn walk_files(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            !(n == "node_modules" || n == "target" || n.starts_with('.'))
        })
        .filter_map(|e| e.ok())
}

pub(crate) fn md_files(root: &Path) -> Vec<PathBuf> {
    walk_files(root)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.into_path())
        .collect()
}

pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// True when `rel` should be processed given an (often empty, meaning "no
/// restriction") set of path prefixes, as used by `driftless update`'s scope
/// argument and `driftless coverage`'s `--include`.
pub(crate) fn path_in_scope(rel: &str, prefixes: &[String]) -> bool {
    prefixes.is_empty() || prefixes.iter().any(|p| rel.starts_with(p.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_refs_stay_inside_root() {
        assert_eq!(
            looks_like_ref("src/auth.py#AuthService.login"),
            Some(("src/auth.py".to_string(), "AuthService.login".to_string()))
        );
        assert!(looks_like_ref("/tmp/auth.py#AuthService.login").is_none());
        assert!(looks_like_ref("../auth.py#AuthService.login").is_none());
        assert!(looks_like_ref("src/auth.py#AuthService.").is_none());
    }

    #[test]
    fn links_are_relative_to_markdown_file() {
        assert_eq!(
            link_ref("../src/auth.py#AuthService.login", Path::new("docs")),
            Some(("src/auth.py".to_string(), "AuthService.login".to_string()))
        );
        assert!(link_ref("../../src/auth.py#AuthService.login", Path::new("docs")).is_none());
        assert!(link_ref("/tmp/auth.py#AuthService.login", Path::new("docs")).is_none());
    }

    #[test]
    fn extracts_inline_fenced_and_markdown_link_refs() {
        let md = r#"
Inline `src/auth.rs#login`.

```rust ref=src/session.rs#Session.start
```

See [logout](../src/auth.rs#logout).
"#;
        let refs = extract_refs(md, Path::new("docs"));
        let keys = refs.iter().map(MdRef::key).collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "src/auth.rs#login",
                "src/session.rs#Session.start",
                "src/auth.rs#logout"
            ]
        );
    }

    #[test]
    fn line_of_counts_from_one() {
        let text = "first\nsecond\nthird";
        assert_eq!(line_of(text, 0), 1);
        assert_eq!(line_of(text, text.find("second").unwrap()), 2);
        assert_eq!(line_of(text, text.len()), 3);
    }

    #[test]
    fn path_in_scope_with_no_prefixes_matches_everything() {
        assert!(path_in_scope("README.md", &[]));
    }

    #[test]
    fn path_in_scope_requires_a_matching_prefix() {
        let prefixes = vec!["docs/".to_string()];
        assert!(path_in_scope("docs/architecture.md", &prefixes));
        assert!(!path_in_scope("README.md", &prefixes));
    }
}
