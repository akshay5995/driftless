mod attachments;
mod hash;
mod names;
mod range;

use std::{ops::Range, path::Path};

pub(crate) use hash::{symbol_hash, symbol_source};
pub(crate) use names::def_path;
use range::symbol_range;

pub(crate) fn language_for(path: &Path) -> Option<tree_sitter::Language> {
    match path.extension()?.to_str()? {
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "kt" | "kts" => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" | "js" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => None,
    }
}

pub(crate) struct Resolved {
    range: Range<usize>,
    body: Option<Range<usize>>,
}

pub(crate) fn parse_tree(lang: tree_sitter::Language, src: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    parser.parse(src, None)
}

pub(crate) fn resolve_symbol_in_tree(
    tree: &tree_sitter::Tree,
    src: &str,
    dotted: &str,
) -> Option<Resolved> {
    let parts: Vec<&str> = dotted.split('.').collect();
    let bytes = src.as_bytes();

    fn walk(
        node: tree_sitter::Node,
        bytes: &[u8],
        parts: &[&str],
        depth: usize,
    ) -> Option<Resolved> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let mut next_depth = depth;
            if let Some(path) = def_path(child, bytes) {
                if parts[depth..].starts_with(&path.iter().map(String::as_str).collect::<Vec<_>>())
                {
                    let matched_depth = depth + path.len();
                    if matched_depth == parts.len() {
                        let symbol_name = path.last().map(String::as_str).unwrap_or(parts[depth]);
                        return Some(Resolved {
                            range: symbol_range(child, bytes, symbol_name),
                            body: child.child_by_field_name("body").map(|b| b.byte_range()),
                        });
                    }
                    next_depth = matched_depth;
                } else {
                    continue;
                }
            }
            if let Some(r) = walk(child, bytes, parts, next_depth) {
                return Some(r);
            }
        }
        None
    }
    walk(tree.root_node(), bytes, &parts, 0)
}

#[cfg(test)]
mod tests;
