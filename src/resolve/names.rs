pub(crate) fn def_name<'a>(node: tree_sitter::Node<'a>, src: &'a [u8]) -> Option<String> {
    let kind = node.kind();
    let named = matches!(
        kind,
        "function_definition"
            | "class_definition"
            | "function_item"
            | "function_declaration"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "mod_item"
            | "const_item"
            | "static_item"
            | "type_item"
            | "class_declaration"
            | "method_declaration"
            | "method_definition"
            | "object_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "type_spec"
            | "enum_declaration"
            | "abstract_class_declaration"
            | "abstract_method_signature"
            | "variable_declarator"
            | "public_field_definition"
            | "method"
            | "class"
            | "module"
            | "singleton_method"
    );
    if named {
        if let Some(n) = node.child_by_field_name("name") {
            return n.utf8_text(src).ok().map(|s| s.to_string());
        }
    }
    if kind == "impl_item" {
        if let Some(t) = node.child_by_field_name("type") {
            return t.utf8_text(src).ok().map(|s| s.to_string());
        }
    }
    None
}

pub(crate) fn def_path<'a>(node: tree_sitter::Node<'a>, src: &'a [u8]) -> Option<Vec<String>> {
    if node.kind() == "method_declaration" {
        let receiver = node
            .child_by_field_name("receiver")
            .and_then(|n| go_receiver_type(n, src));
        let name = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
            .map(str::to_string);
        if let (Some(receiver), Some(name)) = (receiver, name) {
            return Some(vec![receiver, name]);
        }
    }
    def_name(node, src).map(|name| vec![name])
}

fn go_receiver_type(node: tree_sitter::Node, src: &[u8]) -> Option<String> {
    let kind = node.kind();
    if kind == "parameter_declaration" {
        return node
            .child_by_field_name("type")
            .and_then(|n| go_receiver_type(n, src));
    }
    if matches!(kind, "type_identifier" | "identifier") {
        return node.utf8_text(src).ok().map(|s| s.to_string());
    }
    if kind == "pointer_type" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(name) = go_receiver_type(child, src) {
                return Some(name);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "type_identifier" | "qualified_type" | "generic_type"
        ) {
            return child.utf8_text(src).ok().map(clean_go_receiver_type);
        }
        if let Some(name) = go_receiver_type(child, src) {
            return Some(name);
        }
    }
    None
}

fn clean_go_receiver_type(s: &str) -> String {
    s.rsplit('.')
        .next()
        .unwrap_or(s)
        .split('[')
        .next()
        .unwrap_or(s)
        .trim_start_matches('*')
        .trim()
        .to_string()
}
