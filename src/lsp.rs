use crate::check::{CheckContext, RefStatus};
use crate::lockfile::load_lock;
use crate::refs::extract_refs;
use crate::{LOCKFILE_NAME, SOURCE_EXTENSIONS, TOOL_NAME};
use std::collections::HashMap;
use std::path::PathBuf;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    root: std::sync::Mutex<Option<PathBuf>>,
    client_can_watch_files: std::sync::Mutex<bool>,
    open_docs: std::sync::Mutex<HashMap<Url, String>>,
}

fn uri_extension(uri: &Url) -> Option<String> {
    uri.to_file_path()
        .ok()
        .and_then(|p| p.extension().and_then(|x| x.to_str()).map(str::to_string))
}

fn is_markdown_uri(uri: &Url) -> bool {
    matches!(uri_extension(uri).as_deref(), Some("md" | "markdown"))
}

fn is_source_uri(uri: &Url) -> bool {
    uri_extension(uri).is_some_and(|ext| {
        SOURCE_EXTENSIONS
            .iter()
            .any(|source_ext| *source_ext == ext)
    })
}

fn is_lockfile_uri(uri: &Url) -> bool {
    uri.to_file_path()
        .ok()
        .and_then(|p| {
            p.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .is_some_and(|name| name == LOCKFILE_NAME)
}

fn watch_kind_all() -> WatchKind {
    WatchKind::Create | WatchKind::Change | WatchKind::Delete
}

fn watched_files_registration() -> Registration {
    let options = DidChangeWatchedFilesRegistrationOptions {
        watchers: vec![
            FileSystemWatcher {
                glob_pattern: GlobPattern::String(format!(
                    "**/*.{{{}}}",
                    SOURCE_EXTENSIONS.join(",")
                )),
                kind: Some(watch_kind_all()),
            },
            FileSystemWatcher {
                glob_pattern: GlobPattern::String(format!("**/{LOCKFILE_NAME}")),
                kind: Some(watch_kind_all()),
            },
        ],
    };
    Registration {
        id: "driftless-file-watchers".into(),
        method: "workspace/didChangeWatchedFiles".into(),
        register_options: Some(serde_json::to_value(options).unwrap()),
    }
}

fn to_position(text: &str, offset: usize) -> Position {
    let offset = offset.min(text.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for b in text[..offset].chars() {
        if b == '\n' {
            line += 1;
            col = 0;
        } else {
            col += b.len_utf16() as u32;
        }
    }
    Position::new(line, col)
}

impl Backend {
    async fn check_doc(&self, uri: Url, text: String) {
        let root = self.root.lock().unwrap().clone();
        let Some(root) = root else { return };
        let lock = match load_lock(&root) {
            Ok(lock) => lock,
            Err(err) => {
                self.client
                    .publish_diagnostics(
                        uri,
                        vec![Diagnostic {
                            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some(TOOL_NAME.into()),
                            message: format!("failed to read {LOCKFILE_NAME}: {err}"),
                            ..Default::default()
                        }],
                        None,
                    )
                    .await;
                return;
            }
        };
        let doc_dir = uri
            .to_file_path()
            .ok()
            .and_then(|p| p.strip_prefix(&root).ok().map(|x| x.to_path_buf()))
            .and_then(|p| p.parent().map(|x| x.to_path_buf()))
            .unwrap_or_default();
        let mut diags = Vec::new();
        let mut context = CheckContext::default();
        for r in extract_refs(&text, &doc_dir) {
            let (msg, severity) = match context.check_ref(&root, &r, Some(&lock)) {
                RefStatus::Ok => continue,
                RefStatus::FileMissing => (
                    format!("{}: file not found", r.key()),
                    DiagnosticSeverity::ERROR,
                ),
                RefStatus::SymbolMissing => (
                    format!("{}: symbol not found", r.key()),
                    DiagnosticSeverity::ERROR,
                ),
                RefStatus::SigDrift { .. } => (
                    format!("{}: signature changed since docs were locked", r.key()),
                    DiagnosticSeverity::ERROR,
                ),
                RefStatus::BodyDrift { .. } => (
                    format!("{}: body changed since docs were locked", r.key()),
                    DiagnosticSeverity::WARNING,
                ),
                RefStatus::Unlocked { .. } => (
                    format!(
                        "{}: not in {} (run `{} update`)",
                        r.key(),
                        LOCKFILE_NAME,
                        TOOL_NAME
                    ),
                    DiagnosticSeverity::ERROR,
                ),
            };
            diags.push(Diagnostic {
                range: Range::new(
                    to_position(&text, r.span.start),
                    to_position(&text, r.span.end),
                ),
                severity: Some(severity),
                source: Some(TOOL_NAME.into()),
                message: msg,
                ..Default::default()
            });
        }
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn check_open_docs(&self) {
        let docs = self.open_docs.lock().unwrap().clone();
        for (uri, text) in docs {
            self.check_doc(uri, text).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root = params
            .root_uri
            .as_ref()
            .and_then(|u| u.to_file_path().ok())
            .or_else(|| std::env::current_dir().ok());
        *self.root.lock().unwrap() = root;
        let can_register_file_watchers = params
            .capabilities
            .workspace
            .and_then(|workspace| workspace.did_change_watched_files)
            .and_then(|caps| caps.dynamic_registration)
            .unwrap_or(false);
        *self.client_can_watch_files.lock().unwrap() = can_register_file_watchers;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: TOOL_NAME.into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        if *self.client_can_watch_files.lock().unwrap() {
            let _ = self
                .client
                .register_capability(vec![watched_files_registration()])
                .await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        if is_markdown_uri(&p.text_document.uri) {
            self.open_docs
                .lock()
                .unwrap()
                .insert(p.text_document.uri.clone(), p.text_document.text.clone());
            self.check_doc(p.text_document.uri, p.text_document.text)
                .await;
        }
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if is_markdown_uri(&p.text_document.uri) {
            if let Some(change) = p.content_changes.into_iter().last() {
                self.open_docs
                    .lock()
                    .unwrap()
                    .insert(p.text_document.uri.clone(), change.text.clone());
                self.check_doc(p.text_document.uri, change.text).await;
            }
        } else if is_source_uri(&p.text_document.uri) {
            self.check_open_docs().await;
        }
    }

    async fn did_close(&self, p: DidCloseTextDocumentParams) {
        self.open_docs.lock().unwrap().remove(&p.text_document.uri);
        self.client
            .publish_diagnostics(p.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_save(&self, p: DidSaveTextDocumentParams) {
        if is_markdown_uri(&p.text_document.uri) {
            if let Ok(path) = p.text_document.uri.to_file_path() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    self.open_docs
                        .lock()
                        .unwrap()
                        .insert(p.text_document.uri.clone(), text.clone());
                    self.check_doc(p.text_document.uri, text).await;
                }
            }
        } else if is_source_uri(&p.text_document.uri) {
            self.check_open_docs().await;
        }
    }

    async fn did_change_watched_files(&self, p: DidChangeWatchedFilesParams) {
        if p.changes
            .iter()
            .any(|change| is_source_uri(&change.uri) || is_lockfile_uri(&change.uri))
        {
            self.check_open_docs().await;
        }
    }
}

pub(crate) async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        root: std::sync::Mutex::new(None),
        client_can_watch_files: std::sync::Mutex::new(false),
        open_docs: std::sync::Mutex::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_use_lsp_utf16_columns() {
        let text = "a💡b\nc";
        assert_eq!(to_position(text, "a💡".len()), Position::new(0, 3));
        assert_eq!(to_position(text, "a💡b\nc".len()), Position::new(1, 1));
    }

    #[test]
    fn watched_files_registration_tracks_sources_and_lockfile() {
        let registration = watched_files_registration();
        assert_eq!(registration.method, "workspace/didChangeWatchedFiles");

        let options = registration.register_options.expect("registration options");
        let watchers = options["watchers"].as_array().expect("watchers array");
        let source_pattern = format!("**/*.{{{}}}", SOURCE_EXTENSIONS.join(","));
        let lockfile_pattern = format!("**/{LOCKFILE_NAME}");
        assert!(watchers
            .iter()
            .any(|watcher| { watcher["globPattern"].as_str() == Some(source_pattern.as_str()) }));
        assert!(watchers
            .iter()
            .any(|watcher| watcher["globPattern"].as_str() == Some(lockfile_pattern.as_str())));
    }

    #[test]
    fn lockfile_uri_detection_matches_lockfile_name() {
        let lock_uri = Url::from_file_path("/tmp/project/.driftless.lock").unwrap();
        let readme_uri = Url::from_file_path("/tmp/project/README.md").unwrap();

        assert!(is_lockfile_uri(&lock_uri));
        assert!(!is_lockfile_uri(&readme_uri));
    }
}
