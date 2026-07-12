pub(super) fn is_attached_statement(
    node: tree_sitter::Node,
    bytes: &[u8],
    symbol_name: &str,
) -> bool {
    let kind = node.kind();
    if !matches!(
        kind,
        "expression_statement" | "export_statement" | "assignment" | "call_expression"
    ) {
        return false;
    }
    node.utf8_text(bytes)
        .ok()
        .map(str::trim)
        .is_some_and(|text| text_attaches_to_symbol(node, bytes, text, symbol_name))
}

fn text_attaches_to_symbol(
    node: tree_sitter::Node,
    bytes: &[u8],
    text: &str,
    symbol_name: &str,
) -> bool {
    if symbol_name.is_empty() {
        return false;
    }
    let assignment = is_assignment(node);
    (starts_symbol_member(text, symbol_name) && assignment)
        || starts_with_symbol_export(text, symbol_name)
        || (starts_with_commonjs_export(node, bytes, text, symbol_name) && assignment)
        || starts_with_object_assign(text, symbol_name)
}

fn is_assignment(node: tree_sitter::Node) -> bool {
    let expression = if node.kind() == "expression_statement" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    };
    matches!(
        expression.kind(),
        "assignment" | "assignment_expression" | "augmented_assignment_expression"
    )
}

fn starts_symbol_member(text: &str, symbol_name: &str) -> bool {
    text.strip_prefix(symbol_name)
        .and_then(|rest| rest.as_bytes().first().copied())
        .is_some_and(|next| matches!(next, b'.' | b'['))
}

fn starts_with_symbol_export(text: &str, symbol_name: &str) -> bool {
    let Some(rest) = text.strip_prefix("export") else {
        return false;
    };
    let rest = rest.trim_start();
    if let Some(default_expr) = rest.strip_prefix("default") {
        return contains_identifier(default_expr, symbol_name);
    }
    rest.starts_with('{') && contains_identifier(rest, symbol_name)
}

fn starts_with_commonjs_export(
    node: tree_sitter::Node,
    bytes: &[u8],
    text: &str,
    symbol_name: &str,
) -> bool {
    let property_export = text
        .strip_prefix("module.exports.")
        .or_else(|| text.strip_prefix("exports."))
        .is_some_and(|rest| starts_with_identifier(rest, symbol_name));
    if property_export {
        return true;
    }

    let direct_export = text
        .strip_prefix("module.exports")
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix('='))
        .is_some();
    direct_export && assignment_rhs_references_symbol(node, bytes, symbol_name)
}

fn assignment_rhs_references_symbol(
    node: tree_sitter::Node,
    bytes: &[u8],
    symbol_name: &str,
) -> bool {
    let assignment = if node.kind() == "expression_statement" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    };
    assignment
        .child_by_field_name("right")
        .is_some_and(|right| node_references_symbol(right, bytes, symbol_name))
}

fn node_references_symbol(node: tree_sitter::Node, bytes: &[u8], symbol_name: &str) -> bool {
    if matches!(
        node.kind(),
        "function_expression"
            | "function_declaration"
            | "generator_function"
            | "generator_function_declaration"
            | "arrow_function"
            | "method_definition"
    ) {
        return function_scope_references_symbol(node, bytes, symbol_name);
    }
    if matches!(node.kind(), "class" | "class_declaration") {
        return class_scope_references_symbol(node, bytes, symbol_name);
    }
    if matches!(node.kind(), "identifier" | "shorthand_property_identifier")
        && node.utf8_text(bytes).ok() == Some(symbol_name)
    {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| node_references_symbol(child, bytes, symbol_name));
    found
}

fn function_scope_references_symbol(
    node: tree_sitter::Node,
    bytes: &[u8],
    symbol_name: &str,
) -> bool {
    let function_name_binds = node.kind() != "method_definition"
        && node
            .child_by_field_name("name")
            .is_some_and(|name| name.utf8_text(bytes).ok() == Some(symbol_name));
    let parameter_binds = node
        .child_by_field_name("parameters")
        .is_some_and(|parameters| binding_node_contains(parameters, bytes, symbol_name));
    if function_name_binds || parameter_binds {
        return false;
    }
    node.child_by_field_name("body")
        .is_some_and(|body| node_references_symbol(body, bytes, symbol_name))
}

fn class_scope_references_symbol(node: tree_sitter::Node, bytes: &[u8], symbol_name: &str) -> bool {
    if node
        .child_by_field_name("name")
        .is_some_and(|name| name.utf8_text(bytes).ok() == Some(symbol_name))
    {
        return false;
    }
    node.child_by_field_name("body")
        .is_some_and(|body| node_references_symbol(body, bytes, symbol_name))
}

fn binding_node_contains(node: tree_sitter::Node, bytes: &[u8], symbol_name: &str) -> bool {
    if matches!(
        node.kind(),
        "identifier" | "shorthand_property_identifier_pattern"
    ) && node.utf8_text(bytes).ok() == Some(symbol_name)
    {
        return true;
    }
    if matches!(
        node.kind(),
        "required_parameter" | "optional_parameter" | "rest_parameter"
    ) {
        return node
            .child_by_field_name("pattern")
            .or_else(|| node.child_by_field_name("name"))
            .is_some_and(|binding| binding_node_contains(binding, bytes, symbol_name));
    }
    if node.kind() == "pair_pattern" {
        return node
            .child_by_field_name("value")
            .is_some_and(|binding| binding_node_contains(binding, bytes, symbol_name));
    }
    if matches!(
        node.kind(),
        "assignment_pattern" | "object_assignment_pattern"
    ) {
        return node
            .child_by_field_name("left")
            .is_some_and(|binding| binding_node_contains(binding, bytes, symbol_name));
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| binding_node_contains(child, bytes, symbol_name));
    found
}

fn starts_with_object_assign(text: &str, symbol_name: &str) -> bool {
    text.strip_prefix("Object.assign(")
        .is_some_and(|rest| starts_with_identifier(rest.trim_start(), symbol_name))
}

fn contains_identifier(text: &str, symbol_name: &str) -> bool {
    let mut start = 0;
    while let Some(offset) = text[start..].find(symbol_name) {
        let idx = start + offset;
        let before = idx
            .checked_sub(1)
            .is_some_and(|prev| identifier_at(text, prev));
        let after = identifier_at(text, idx + symbol_name.len());
        if !before && !after {
            return true;
        }
        start = idx + symbol_name.len();
    }
    false
}

fn starts_with_identifier(text: &str, symbol_name: &str) -> bool {
    text.strip_prefix(symbol_name)
        .is_some_and(|_| !identifier_at(text, symbol_name.len()))
}

fn identifier_at(text: &str, idx: usize) -> bool {
    text.as_bytes()
        .get(idx)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
}
