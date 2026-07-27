mod attachments;
mod hash;
mod names;
mod range;

use std::{ops::Range, path::Path};

pub(crate) use hash::{short_hash, symbol_hash, symbol_source};
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
    /// Comment nodes within the definition's own span (not the leading
    /// metadata prepended by `symbol_range`, so attached doc comments still
    /// count toward the hash). Some grammars (e.g. Python) place a comment
    /// that opens a body as a sibling of the body's own node rather than
    /// nesting it inside, so these are collected over the whole definition
    /// rather than just its body field.
    comment_ranges: Vec<Range<usize>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveError {
    NotFound,
    Ambiguous,
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
) -> Result<Resolved, ResolveError> {
    let parts: Vec<&str> = dotted.split('.').collect();
    let bytes = src.as_bytes();

    fn collect_comment_ranges(node: tree_sitter::Node) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        fn walk(node: tree_sitter::Node, out: &mut Vec<Range<usize>>) {
            if node.kind().contains("comment") {
                out.push(node.byte_range());
                return;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk(child, out);
            }
        }
        walk(node, &mut out);
        out
    }

    // Candidates are kept lightweight (just the matched node and its name)
    // during the walk; building a `Resolved` runs `symbol_range` and a full
    // subtree comment scan, which is wasted work for candidates that turn
    // out to be discarded below (most commonly a type's own `impl` block,
    // which matches every bare reference to that type alongside its
    // declaration).
    fn walk<'tree>(
        node: tree_sitter::Node<'tree>,
        bytes: &[u8],
        parts: &[&str],
        depth: usize,
        matches: &mut Vec<(bool, tree_sitter::Node<'tree>, String)>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let mut next_depth = depth;
            let mut skip_recurse = false;
            if let Some(path) = def_path(child, bytes) {
                if parts[depth..].starts_with(&path.iter().map(String::as_str).collect::<Vec<_>>())
                {
                    let matched_depth = depth + path.len();
                    if matched_depth == parts.len() {
                        let symbol_name = path.last().cloned().unwrap_or(parts[depth].to_string());
                        matches.push((child.kind() == "impl_item", child, symbol_name));
                        skip_recurse = true;
                    } else {
                        next_depth = matched_depth;
                    }
                } else {
                    continue;
                }
            }
            if !skip_recurse {
                walk(child, bytes, parts, next_depth, matches);
            }
        }
    }

    let mut matches = Vec::new();
    walk(tree.root_node(), bytes, &parts, 0, &mut matches);

    // A bare type name matches both its own declaration (struct/class/...)
    // and any `impl` blocks for that type, since an impl block's def_path is
    // just the type name too. That's not a real ambiguity between distinct
    // definitions, so the declaration wins and impl-only matches are
    // dropped whenever a non-impl match exists.
    if matches.iter().any(|(is_impl, _, _)| !is_impl) {
        matches.retain(|(is_impl, _, _)| !is_impl);
    }

    match matches.len() {
        0 => Err(ResolveError::NotFound),
        1 => {
            let (_, node, symbol_name) = matches.pop().expect("checked len == 1");
            let body_node = node.child_by_field_name("body");
            Ok(Resolved {
                range: symbol_range(node, bytes, &symbol_name),
                body: body_node.map(|b| b.byte_range()),
                comment_ranges: collect_comment_ranges(node),
            })
        }
        _ => Err(ResolveError::Ambiguous),
    }
}

#[cfg(test)]
mod tests;
