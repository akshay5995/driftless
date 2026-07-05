use sha2::{Digest, Sha256};
use std::path::Path;

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

pub(crate) struct Resolved {
    range: std::ops::Range<usize>,
    body: Option<std::ops::Range<usize>>,
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
                        return Some(Resolved {
                            range: child.byte_range(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_symbol(lang: tree_sitter::Language, src: &str, dotted: &str) -> Option<Resolved> {
        let tree = parse_tree(lang, src)?;
        resolve_symbol_in_tree(&tree, src, dotted)
    }

    #[test]
    fn resolves_rust_impl_methods() {
        let src = r#"
pub struct Account;

	impl Account {
	    pub fn login(&self) -> bool {
	        true
	    }
	}
	"#;
        let resolved = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), src, "Account.login");

        assert!(resolved.is_some());
        let source = &src[resolved.unwrap().range];
        assert!(source.contains("pub fn login"));
    }

    #[test]
    fn resolves_go_receiver_methods() {
        let src = r#"
package auth

type User struct{}

func (u *User) Login() bool {
    return true
}
"#;
        let resolved = resolve_symbol(tree_sitter_go::LANGUAGE.into(), src, "User.Login");

        assert!(resolved.is_some());
        let source = &src[resolved.unwrap().range];
        assert!(source.contains("func (u *User) Login"));
    }

    #[test]
    fn body_only_changes_keep_signature_hash_stable() {
        let before = r#"pub fn login(user: &str) -> bool {
    user == "admin"
}
"#;
        let after = r#"pub fn login(user: &str) -> bool {
    user == "root"
}
"#;

        let before_res =
            resolve_symbol(tree_sitter_rust::LANGUAGE.into(), before, "login").unwrap();
        let after_res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), after, "login").unwrap();
        let before_hash = symbol_hash(before, &before_res);
        let after_hash = symbol_hash(after, &after_res);
        let (before_sig, before_body) = before_hash.split_once(':').unwrap();
        let (after_sig, after_body) = after_hash.split_once(':').unwrap();

        assert_eq!(before_sig, after_sig);
        assert_ne!(before_body, after_body);
    }
}
