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
fn rust_leading_attributes_are_part_of_symbol_source_and_hash() {
    let before = r#"
#[derive(ClapParser)]
#[command(
    about = "old help",
    after_help = "Agent loop:
  driftless init

Ref examples:
  `src/lib.rs#login`"
)]
struct Cli {
    cmd: Cmd,
}
"#;
    let after = before.replace("old help", "new help");

    let before_res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), before, "Cli").unwrap();
    let after_res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), &after, "Cli").unwrap();

    let before_source = symbol_source(before, &before_res);
    assert!(before_source.starts_with("#[derive(ClapParser)]"));
    assert!(before_source.contains("#[command("));
    assert!(before_source.contains("old help"));
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn rust_current_cli_help_attribute_is_part_of_cli_symbol_source() {
    let src = include_str!("../main.rs");
    let res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), src, "Cli").unwrap();
    let source = symbol_source(src, &res);

    assert!(source.contains("Agent loop:"), "{source}");
}

#[test]
fn at_metadata_blocks_are_part_of_symbol_source_and_hash() {
    let before = r#"
@Route(
    path = "/old",
    methods = ["GET"],
)
class AuthService:
    pass
"#;
    let after = before.replace("/old", "/new");

    let before_res =
        resolve_symbol(tree_sitter_python::LANGUAGE.into(), before, "AuthService").unwrap();
    let after_res =
        resolve_symbol(tree_sitter_python::LANGUAGE.into(), &after, "AuthService").unwrap();
    let before_source = symbol_source(before, &before_res);

    assert!(before_source.starts_with("@Route("), "{before_source}");
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn exported_wrapper_is_part_of_symbol_source_and_hash() {
    let before = r#"
export function login(user: string): boolean {
    return user === "admin";
}
"#;
    let after = before.replacen("export function", "function", 1);

    let before_res =
        resolve_symbol(tree_sitter_typescript::LANGUAGE_TSX.into(), before, "login").unwrap();
    let after_res =
        resolve_symbol(tree_sitter_typescript::LANGUAGE_TSX.into(), &after, "login").unwrap();
    let before_source = symbol_source(before, &before_res);

    assert!(
        before_source.starts_with("export function login"),
        "{before_source}"
    );
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn exported_const_wrapper_is_part_of_symbol_source() {
    let src = r#"
export const Button = () => {
    return <button>Save</button>;
};
"#;

    let res = resolve_symbol(tree_sitter_typescript::LANGUAGE_TSX.into(), src, "Button").unwrap();
    let source = symbol_source(src, &res);

    assert!(source.starts_with("export const Button"), "{source}");
}

#[test]
fn attached_static_assignment_is_part_of_symbol_source_and_hash() {
    let before = r#"
export function Button() {
    return <button>Save</button>;
}

Button.displayName = "OldButton";
"#;
    let after = before.replace("OldButton", "NewButton");

    let before_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        before,
        "Button",
    )
    .unwrap();
    let after_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        &after,
        "Button",
    )
    .unwrap();
    let before_source = symbol_source(before, &before_res);

    assert!(
        before_source.contains("Button.displayName"),
        "{before_source}"
    );
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn attached_default_export_wrapper_is_part_of_symbol_source_and_hash() {
    let before = r#"
function Button() {
    return <button>Save</button>;
}

export default memo(Button);
"#;
    let after = before.replace("memo(Button)", "forwardRef(Button)");

    let before_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        before,
        "Button",
    )
    .unwrap();
    let after_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        &after,
        "Button",
    )
    .unwrap();
    let before_source = symbol_source(before, &before_res);

    assert!(
        before_source.contains("export default memo(Button)"),
        "{before_source}"
    );
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn attached_commonjs_export_is_part_of_symbol_source_and_hash() {
    let before = r#"
function Button() {
    return "save";
}

module.exports.Button = Button;
"#;
    let after = before.replace("module.exports.Button", "exports.Button");

    let before_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        before,
        "Button",
    )
    .unwrap();
    let after_res = resolve_symbol(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        &after,
        "Button",
    )
    .unwrap();
    let before_source = symbol_source(before, &before_res);

    assert!(
        before_source.contains("module.exports.Button"),
        "{before_source}"
    );
    assert_ne!(
        symbol_hash(before, &before_res),
        symbol_hash(&after, &after_res)
    );
}

#[test]
fn unrelated_adjacent_export_is_not_part_of_symbol_source() {
    let src = r#"
export function Button() {
    return "save";
}

export function Link() {
    return "go";
}
"#;

    let res = resolve_symbol(tree_sitter_typescript::LANGUAGE_TSX.into(), src, "Button").unwrap();
    let source = symbol_source(src, &res);

    assert!(source.contains("export function Button"), "{source}");
    assert!(!source.contains("export function Link"), "{source}");
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

    let before_res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), before, "login").unwrap();
    let after_res = resolve_symbol(tree_sitter_rust::LANGUAGE.into(), after, "login").unwrap();
    let before_hash = symbol_hash(before, &before_res);
    let after_hash = symbol_hash(after, &after_res);
    let (before_sig, before_body) = before_hash.split_once(':').unwrap();
    let (after_sig, after_body) = after_hash.split_once(':').unwrap();

    assert_eq!(before_sig, after_sig);
    assert_ne!(before_body, after_body);
}
