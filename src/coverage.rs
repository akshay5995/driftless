use crate::refs::{extract_refs, md_files};
use crate::resolve::{def_path, language_for};
use crate::TOOL_NAME;
use std::path::Path;

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Go,
    Java,
    Kotlin,
    Py,
    Ruby,
    Rs,
    Ts,
}

fn lang_tag(path: &Path) -> Option<Lang> {
    match path.extension()?.to_str()? {
        "go" => Some(Lang::Go),
        "java" => Some(Lang::Java),
        "kt" | "kts" => Some(Lang::Kotlin),
        "py" => Some(Lang::Py),
        "rb" => Some(Lang::Ruby),
        "rs" => Some(Lang::Rs),
        "ts" | "tsx" | "js" | "jsx" => Some(Lang::Ts),
        _ => None,
    }
}

struct Def {
    chain: String,
    kind: &'static str,
}

fn coverage_kind(lang: Lang, kind: &str, in_rust_impl: bool) -> Option<&'static str> {
    match (lang, kind) {
        (Lang::Go, "function_declaration") => Some("fn"),
        (Lang::Go, "method_declaration") => Some("method"),
        (Lang::Go, "type_spec") => Some("type"),
        (Lang::Java, "class_declaration") => Some("class"),
        (Lang::Java, "interface_declaration") => Some("interface"),
        (Lang::Java, "enum_declaration") => Some("enum"),
        (Lang::Java, "method_declaration") => Some("method"),
        (Lang::Kotlin, "function_declaration") => Some("fn"),
        (Lang::Kotlin, "class_declaration") => Some("class"),
        (Lang::Kotlin, "object_declaration") => Some("object"),
        (Lang::Py, "function_definition") => Some("fn"),
        (Lang::Py, "class_definition") => Some("class"),
        (Lang::Ruby, "method") => Some("method"),
        (Lang::Ruby, "singleton_method") => Some("method"),
        (Lang::Ruby, "class") => Some("class"),
        (Lang::Ruby, "module") => Some("module"),
        (Lang::Rs, "function_item") if in_rust_impl => Some("method"),
        (Lang::Rs, "function_item") => Some("fn"),
        (Lang::Rs, "struct_item") => Some("struct"),
        (Lang::Rs, "enum_item") => Some("enum"),
        (Lang::Rs, "trait_item") => Some("trait"),
        (Lang::Ts, "function_declaration") => Some("fn"),
        (Lang::Ts, "class_declaration") | (Lang::Ts, "abstract_class_declaration") => Some("class"),
        (Lang::Ts, "method_definition") => Some("method"),
        (Lang::Ts, "interface_declaration") => Some("interface"),
        (Lang::Ts, "enum_declaration") => Some("enum"),
        _ => None,
    }
}

fn exported_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn node_text_contains(node: tree_sitter::Node, src: &[u8], needle: &str) -> bool {
    node.utf8_text(src).is_ok_and(|text| text.contains(needle))
}

fn is_public(lang: Lang, node: tree_sitter::Node, name: &str, src: &[u8], exported: bool) -> bool {
    match lang {
        Lang::Go => exported_name(name),
        Lang::Java => {
            let mut c = node.walk();
            let has_public = node
                .children(&mut c)
                .any(|ch| ch.kind() == "modifiers" && node_text_contains(ch, src, "public"));
            has_public
        }
        Lang::Kotlin => {
            let mut c = node.walk();
            let hidden = node.children(&mut c).any(|ch| {
                ch.kind() == "modifiers"
                    && (node_text_contains(ch, src, "private")
                        || node_text_contains(ch, src, "protected")
                        || node_text_contains(ch, src, "internal"))
            });
            !hidden
        }
        Lang::Py => !name.starts_with('_'),
        Lang::Ruby => !name.starts_with('_'),
        Lang::Rs => {
            let mut c = node.walk();
            let is_public = node.children(&mut c).any(|ch| {
                ch.kind() == "visibility_modifier" && matches!(ch.utf8_text(src), Ok("pub"))
            });
            is_public
        }
        Lang::Ts => {
            if name.starts_with('#') || name.starts_with('_') {
                return false;
            }
            let mut c = node.walk();
            let private = node.children(&mut c).any(|ch| {
                ch.kind() == "accessibility_modifier"
                    && matches!(ch.utf8_text(src), Ok("private") | Ok("protected"))
            });
            exported && !private
        }
    }
}

fn collect_public_defs(lang: Lang, ts_lang: tree_sitter::Language, src: &str) -> Vec<Def> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return vec![];
    }
    let Some(tree) = parser.parse(src, None) else {
        return vec![];
    };
    let bytes = src.as_bytes();
    let mut out = Vec::new();

    #[derive(Clone, Copy)]
    struct Scope {
        exported: bool,
        pub_chain: bool,
        rust_impl_depth: usize,
    }

    fn walk(
        node: tree_sitter::Node,
        lang: Lang,
        bytes: &[u8],
        chain: &mut Vec<String>,
        scope: Scope,
        out: &mut Vec<Def>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let exported = scope.exported || child.kind() == "export_statement";
            if let Some(path) = def_path(child, bytes) {
                let name = path.last().expect("definition path has a name").clone();
                let this_pub = is_public(lang, child, &name, bytes, exported);
                let ctor = matches!(name.as_str(), "constructor" | "__init__" | "new");
                let in_rust_impl = lang == Lang::Rs && scope.rust_impl_depth > 0;
                if let Some(kind) = coverage_kind(lang, child.kind(), in_rust_impl) {
                    if this_pub && scope.pub_chain && !ctor {
                        out.push(Def {
                            chain: chain
                                .iter()
                                .chain(path.iter())
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("."),
                            kind,
                        });
                    }
                }
                let path_len = path.len();
                chain.extend(path);
                let is_rust_impl = lang == Lang::Rs && child.kind() == "impl_item";
                let next_pub_chain = if is_rust_impl {
                    scope.pub_chain
                } else {
                    scope.pub_chain && this_pub
                };
                walk(
                    child,
                    lang,
                    bytes,
                    chain,
                    Scope {
                        exported,
                        pub_chain: next_pub_chain,
                        rust_impl_depth: scope.rust_impl_depth + usize::from(is_rust_impl),
                    },
                    out,
                );
                let new_len = chain.len().saturating_sub(path_len);
                chain.truncate(new_len);
            } else {
                walk(child, lang, bytes, chain, Scope { exported, ..scope }, out);
            }
        }
    }
    let mut chain = Vec::new();
    walk(
        tree.root_node(),
        lang,
        bytes,
        &mut chain,
        Scope {
            exported: false,
            pub_chain: true,
            rust_impl_depth: 0,
        },
        &mut out,
    );
    out
}

pub(crate) fn run_coverage(root: &Path, include: &[String], json: bool) -> i32 {
    let mut referenced = std::collections::HashSet::new();
    for md_path in md_files(root) {
        if let Ok(text) = std::fs::read_to_string(&md_path) {
            let rel = md_path.strip_prefix(root).unwrap_or(&md_path).to_path_buf();
            let doc_dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            for r in extract_refs(&text, &doc_dir) {
                referenced.insert(r.key());
            }
        }
    }
    let mut missing = 0usize;
    let mut records: Vec<serde_json::Value> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            !(n == "node_modules" || n == "target" || n.starts_with('.'))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        if !include.is_empty() && !include.iter().any(|p| rel.starts_with(p.as_str())) {
            continue;
        }
        let (Some(lang), Some(ts_lang)) = (lang_tag(path), language_for(path)) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        for def in collect_public_defs(lang, ts_lang, &src) {
            let parts: Vec<&str> = def.chain.split('.').collect();
            let covered = (1..=parts.len())
                .any(|i| referenced.contains(&format!("{}#{}", rel, parts[..i].join("."))));
            if !covered {
                missing += 1;
                if json {
                    let reference = format!("{}#{}", rel, def.chain);
                    records.push(serde_json::json!({
                        "schema_version": 1,
                        "status": "undocumented",
                        "severity": "error",
                        "blocks_exit": true,
                        "kind": def.kind,
                        "source_file": rel.clone(),
                        "symbol": def.chain.clone(),
                        "ref": reference,
                    }));
                } else {
                    eprintln!("undocumented {:<9} {}#{}", def.kind, rel, def.chain);
                }
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&records).unwrap());
    }
    if missing > 0 {
        if !json {
            eprintln!("{}: {} undocumented public symbol(s)", TOOL_NAME, missing);
        }
        1
    } else if json {
        0
    } else {
        println!("{}: coverage ok", TOOL_NAME);
        0
    }
}
