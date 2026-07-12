use super::attachments::is_attached_statement;
use std::ops::Range;

pub(super) fn symbol_range(
    node: tree_sitter::Node,
    bytes: &[u8],
    symbol_name: &str,
) -> Range<usize> {
    let base = semantic_wrapper_node(node);
    let mut range = base.byte_range();
    let mut prev = base.prev_named_sibling();
    while let Some(sibling) = prev {
        let sibling_range = sibling.byte_range();
        if !only_whitespace_between(bytes, sibling_range.end, range.start) {
            break;
        }
        if !is_leading_metadata(sibling, bytes) {
            break;
        }
        range.start = sibling_range.start;
        prev = sibling.prev_named_sibling();
    }
    let mut next = base.next_named_sibling();
    while let Some(sibling) = next {
        let sibling_range = sibling.byte_range();
        if !only_whitespace_between(bytes, range.end, sibling_range.start) {
            break;
        }
        if !is_attached_statement(sibling, bytes, symbol_name) {
            break;
        }
        range.end = sibling_range.end;
        next = sibling.next_named_sibling();
    }
    range
}

fn semantic_wrapper_node(mut node: tree_sitter::Node) -> tree_sitter::Node {
    while let Some(parent) = node.parent() {
        if !is_semantic_wrapper(parent) {
            break;
        }
        if matches!(
            parent.kind(),
            "lexical_declaration" | "variable_declaration"
        ) && parent.named_child_count() > 1
        {
            break;
        }
        node = parent;
    }
    node
}

fn is_semantic_wrapper(node: tree_sitter::Node) -> bool {
    matches!(
        node.kind(),
        "decorated_definition"
            | "export_statement"
            | "lexical_declaration"
            | "variable_declaration"
    )
}

fn only_whitespace_between(bytes: &[u8], start: usize, end: usize) -> bool {
    bytes
        .get(start..end)
        .is_some_and(|gap| gap.iter().all(|b| b.is_ascii_whitespace()))
}

fn is_leading_metadata(node: tree_sitter::Node, bytes: &[u8]) -> bool {
    match node.kind() {
        "attribute_item" | "decorator" | "annotation" | "marker_annotation" => true,
        "line_comment" | "block_comment" => node
            .utf8_text(bytes)
            .ok()
            .map(str::trim_start)
            .is_some_and(|text| text.starts_with("///") || text.starts_with("/**")),
        _ => false,
    }
}
