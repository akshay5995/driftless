#[path = "common/language.rs"]
mod language;
#[path = "common/process.rs"]
mod process;

use language::{
    assert_language_roundtrip, assert_language_roundtrip_refs, assert_metadata_drift_is_reported,
    assert_symbol_drift_is_reported, assert_unrelated_change_is_ignored,
};
use process::{driftless, text, write};
use std::fs;

#[test]
fn go_refs_roundtrip() {
    assert_language_roundtrip(
        "src/auth.go",
        r#"package auth

func Login(user string) bool {
    return user == "admin"
}
"#,
        "src/auth.go#Login",
    );
}

#[test]
fn go_receiver_methods_use_receiver_qualified_refs() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/auth.go",
        r#"package auth

type User struct{}

func (u *User) Login() bool {
    return true
}
"#,
    );
    write(
        dir.path(),
        "README.md",
        "# API\n\nDocumented by `src/auth.go#User` and `src/auth.go#User.Login`.\n",
    );

    let update = driftless(dir.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));
    let lock = fs::read_to_string(dir.path().join(".driftless.lock")).expect("read lockfile");
    assert!(lock.contains("src/auth.go#User.Login"), "lockfile:\n{lock}");

    let check = driftless(dir.path(), &["check"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));
}

#[test]
fn java_refs_roundtrip() {
    assert_language_roundtrip(
        "src/Auth.java",
        r#"public class Auth {
    public boolean login(String user) {
        return user.equals("admin");
    }
}
"#,
        "src/Auth.java#Auth",
    );
}

#[test]
fn java_method_refs_roundtrip_inside_public_classes() {
    assert_language_roundtrip_refs(
        "src/Auth.java",
        r#"public class Auth {
    public boolean login(String user) {
        return user.equals("admin");
    }
}
"#,
        &["src/Auth.java#Auth", "src/Auth.java#Auth.login"],
    );
}

#[test]
fn java_interface_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/SessionStore.java",
        r#"public interface SessionStore {
    public String load(String id);
}
"#,
        &[
            "src/SessionStore.java#SessionStore",
            "src/SessionStore.java#SessionStore.load",
        ],
    );
}

#[test]
fn java_annotation_drift_fails_check() {
    let before = r#"public @interface GetMapping {
    String value();
}

public class Auth {
    @GetMapping("/old")
    public boolean login(String user) {
        return user.equals("admin");
    }
}
"#;
    let after = before.replace("/old", "/new");

    assert_metadata_drift_is_reported("src/Auth.java", before, &after, "src/Auth.java#Auth.login");
}

#[test]
fn java_annotated_field_change_does_not_drift_following_method() {
    let before = r#"public class Auth {
    @Deprecated
    private int counter = 0;
    public boolean login(String user) {
        return user.equals("admin");
    }
}
"#;
    let after = before.replace("counter = 0", "counter = 1");

    assert_unrelated_change_is_ignored("src/Auth.java", before, &after, "src/Auth.java#Auth.login");
}

#[test]
fn kotlin_refs_roundtrip() {
    assert_language_roundtrip(
        "src/Auth.kt",
        r#"fun login(user: String): Boolean {
    return user == "admin"
}
"#,
        "src/Auth.kt#login",
    );
}

#[test]
fn kotlin_class_method_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/Auth.kt",
        r#"class Auth {
    fun login(user: String): Boolean {
        return user == "admin"
    }
}
"#,
        &["src/Auth.kt#Auth", "src/Auth.kt#Auth.login"],
    );
}

#[test]
fn kotlin_object_method_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/Sessions.kt",
        r#"object Sessions {
    fun start(user: String): String {
        return user
    }
}
"#,
        &["src/Sessions.kt#Sessions", "src/Sessions.kt#Sessions.start"],
    );
}

#[test]
fn kotlin_annotation_drift_fails_check() {
    let before = r#"annotation class Get(val path: String)

class Auth {
    @Get("/old")
    fun login(user: String): Boolean {
        return user == "admin"
    }
}
"#;
    let after = before.replace("/old", "/new");

    assert_metadata_drift_is_reported("src/Auth.kt", before, &after, "src/Auth.kt#Auth.login");
}

#[test]
fn ruby_refs_roundtrip() {
    assert_language_roundtrip(
        "src/auth.rb",
        r#"def login(user)
  user == "admin"
end
"#,
        "src/auth.rb#login",
    );
}

#[test]
fn ruby_class_method_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/auth.rb",
        r#"class Auth
  def login(user)
    user == "admin"
  end
end
"#,
        &["src/auth.rb#Auth", "src/auth.rb#Auth.login"],
    );
}

#[test]
fn ruby_instance_variable_change_does_not_drift_following_method() {
    let before = r#"class Auth
  @counter = 0
  def login(user)
    user == "admin"
  end
end
"#;
    let after = before.replace("@counter = 0", "@counter = 1");

    assert_unrelated_change_is_ignored("src/auth.rb", before, &after, "src/auth.rb#Auth.login");
}

#[test]
fn rust_inner_doc_change_does_not_drift_following_function() {
    let before = r#"//! Crate overview version one.

pub fn login(user: &str) -> bool {
    user == "admin"
}
"#;
    let after = before.replace("version one", "version two");

    assert_unrelated_change_is_ignored("src/lib.rs", before, &after, "src/lib.rs#login");
}

#[test]
fn python_class_method_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/auth.py",
        r#"class AuthService:
    def login(self, user):
        return user == "admin"
"#,
        &["src/auth.py#AuthService", "src/auth.py#AuthService.login"],
    );
}

#[test]
fn python_decorator_drift_fails_check() {
    let before = r#"def route(path):
    def wrap(fn):
        return fn
    return wrap

class AuthService:
    @route("/old")
    def login(self, user):
        return user == "admin"
"#;
    let after = before.replace("/old", "/new");

    assert_metadata_drift_is_reported(
        "src/auth.py",
        before,
        &after,
        "src/auth.py#AuthService.login",
    );
}

#[test]
fn typescript_exported_class_and_function_refs_roundtrip() {
    assert_language_roundtrip_refs(
        "src/auth.ts",
        r#"export function login(user: string): boolean {
    return user === "admin";
}

export class SessionStore {
    load(id: string): string {
        return id;
    }
}
"#,
        &[
            "src/auth.ts#login",
            "src/auth.ts#SessionStore",
            "src/auth.ts#SessionStore.load",
        ],
    );
}

#[test]
fn typescript_decorator_drift_fails_check() {
    let before = r#"function Component(config: { selector: string }) {
    return function (_target: unknown) {
        return config.selector;
    };
}

@Component({
    selector: "old-auth",
})
export class AuthService {
    login(user: string): boolean {
        return user === "admin";
    }
}
"#;
    let after = before.replace("old-auth", "new-auth");

    assert_metadata_drift_is_reported("src/auth.ts", before, &after, "src/auth.ts#AuthService");
}

#[test]
fn typescript_export_removal_drift_fails_check() {
    let before = r#"export function login(user: string): boolean {
    return user === "admin";
}
"#;
    let after = before.replacen("export function", "function", 1);

    assert_symbol_drift_is_reported("src/auth.ts", before, &after, "src/auth.ts#login");
}

#[test]
fn typescript_exported_const_refs_roundtrip() {
    assert_language_roundtrip(
        "src/button.tsx",
        r#"export const Button = () => {
    return <button>Save</button>;
};
"#,
        "src/button.tsx#Button",
    );
}

#[test]
fn tsx_static_assignment_drift_fails_check() {
    let before = r#"export function Button() {
    return <button>Save</button>;
}

Button.displayName = "OldButton";
"#;
    let after = before.replace("OldButton", "NewButton");

    assert_symbol_drift_is_reported("src/button.tsx", before, &after, "src/button.tsx#Button");
}

#[test]
fn tsx_adjacent_member_call_change_does_not_drift_symbol() {
    let before = r#"export function Button() {
    return <button>Save</button>;
}

Button.render("first caller");
"#;
    let after = before.replace("first caller", "second caller");

    assert_unrelated_change_is_ignored("src/button.tsx", before, &after, "src/button.tsx#Button");
}

#[test]
fn tsx_default_wrapper_drift_fails_check() {
    let before = r#"function Button() {
    return <button>Save</button>;
}

export default memo(Button);
"#;
    let after = before.replace("memo(Button)", "forwardRef(Button)");

    assert_symbol_drift_is_reported("src/button.tsx", before, &after, "src/button.tsx#Button");
}

#[test]
fn javascript_exported_function_refs_roundtrip() {
    assert_language_roundtrip(
        "src/auth.js",
        r#"export function login(user) {
    return user === "admin";
}
"#,
        "src/auth.js#login",
    );
}

#[test]
fn javascript_commonjs_export_drift_fails_check() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports.login = login;
"#;
    let after = before.replace("module.exports.login", "exports.login");

    assert_symbol_drift_is_reported("src/auth.js", before, &after, "src/auth.js#login");
}

#[test]
fn javascript_direct_commonjs_export_drift_fails_check() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports = login;
"#;
    let after = before.replace("module.exports = login", "module.exports = wrap(login)");

    assert_symbol_drift_is_reported("src/auth.js", before, &after, "src/auth.js#login");
}

#[test]
fn javascript_commonjs_string_literal_does_not_attach_symbol() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports = "login";
"#;
    let after = before.replace("\"login\"", "\"logout\"");

    assert_unrelated_change_is_ignored("src/auth.js", before, &after, "src/auth.js#login");
}

#[test]
fn javascript_commonjs_function_binding_does_not_attach_symbol() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports = function login() {
    return "first export";
};
"#;
    let after = before.replace("first export", "second export");

    assert_unrelated_change_is_ignored("src/auth.js", before, &after, "src/auth.js#login");
}

#[test]
fn javascript_commonjs_arrow_wrapper_drift_fails_check() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports = (...args) => login(...args);
"#;
    let after = before.replace("login(...args)", "login.call(null, ...args)");

    assert_symbol_drift_is_reported("src/auth.js", before, &after, "src/auth.js#login");
}

#[test]
fn javascript_commonjs_parameter_binding_does_not_attach_symbol() {
    let before = r#"function login(user) {
    return user === "admin";
}

module.exports = (login) => login("first argument");
"#;
    let after = before.replace("first argument", "second argument");

    assert_unrelated_change_is_ignored("src/auth.js", before, &after, "src/auth.js#login");
}
