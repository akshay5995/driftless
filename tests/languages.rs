#[path = "common/language.rs"]
mod language;
#[path = "common/process.rs"]
mod process;

use language::{
    assert_language_roundtrip, assert_language_roundtrip_refs, assert_missing_doc_is_reported,
};
use process::{driftless, text, write};
use std::fs;

#[test]
fn go_refs_and_coverage_work() {
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

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}

#[test]
fn go_underscore_names_are_not_public_coverage() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/auth.go",
        r#"package auth

func _helper() bool {
    return true
}
"#,
    );
    write(dir.path(), "README.md", "# API\n\nNo public symbols.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}

#[test]
fn go_coverage_fails_for_missing_public_symbol_docs() {
    assert_missing_doc_is_reported(
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
fn java_refs_and_coverage_work() {
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
fn java_coverage_fails_for_missing_public_symbol_docs() {
    assert_missing_doc_is_reported(
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
fn kotlin_refs_and_coverage_work() {
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
fn kotlin_coverage_fails_for_missing_public_symbol_docs() {
    assert_missing_doc_is_reported(
        "src/Auth.kt",
        r#"fun login(user: String): Boolean {
    return user == "admin"
}
"#,
        "src/Auth.kt#login",
    );
}

#[test]
fn ruby_refs_and_coverage_work() {
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
fn ruby_coverage_fails_for_missing_public_symbol_docs() {
    assert_missing_doc_is_reported(
        "src/auth.rb",
        r#"def login(user)
  user == "admin"
end
"#,
        "src/auth.rb#login",
    );
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
fn python_private_methods_are_not_public_coverage() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/auth.py",
        r#"class AuthService:
    def _token(self):
        return "secret"
"#,
    );
    write(dir.path(), "README.md", "See `src/auth.py#AuthService`.\n");

    let update = driftless(dir.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
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
