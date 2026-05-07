use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover, HoverContents, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, MarkedString,
    MessageType, Position, Range, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug, Default)]
struct DocumentStore {
    text: RwLock<std::collections::HashMap<Url, String>>,
}

impl DocumentStore {
    async fn set(&self, uri: Url, text: String) {
        self.text.write().await.insert(uri, text);
    }

    async fn get(&self, uri: &Url) -> Option<String> {
        self.text.read().await.get(uri).cloned()
    }
}

struct Backend {
    client: Client,
    documents: Arc<DocumentStore>,
}

impl Backend {
    fn base_dir(uri: &Url) -> std::path::PathBuf {
        uri.to_file_path()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()))
    }

    async fn publish_diagnostics(&self, uri: Url, text: String) {
        let diagnostics = match lane::compile_program_with_base_dir(&text, Self::base_dir(&uri)) {
            Ok(_) => Vec::new(),
            Err(error) => {
                let line = error.line().unwrap_or(1).saturating_sub(1) as u32;
                vec![Diagnostic {
                    range: Range::new(Position::new(line, 0), Position::new(line, 1)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: error.to_string(),
                    ..Diagnostic::default()
                }]
            }
        };
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    fn newest_text(changes: Vec<TextDocumentContentChangeEvent>) -> Option<String> {
        changes.into_iter().last().map(|change| change.text)
    }

    fn completion_items() -> Vec<CompletionItem> {
        let mut items = Vec::new();
        for (label, detail) in [
            (
                "const",
                "Emit a Lane binding even when only referenced by generated code",
            ),
            ("provided", "Declare a host-provided shader input"),
            ("Hom", "Function type constructor"),
            ("Func", "Function type constructor alias"),
            ("Object", "Current ambient SDF object type"),
            ("Object2D", "2D SDF object type"),
            ("Object3D", "3D SDF object type"),
            ("Type", "Type metatype"),
            ("Cat", "Category metatype"),
            ("#import", "Import a Lane module"),
            ("#prec", "Set default differential precision"),
            ("#2D", "Switch the program to 2D SDF mode"),
        ] {
            items.push(CompletionItem {
                label: label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(detail.to_string()),
                ..CompletionItem::default()
            });
        }
        for module in ["std", "raytracing"] {
            items.push(CompletionItem {
                label: module.to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some("built-in Lane module".to_string()),
                ..CompletionItem::default()
            });
        }
        for (label, detail, kind) in [
            (
                "R{n}",
                "generic real vector space dimension",
                CompletionItemKind::CLASS,
            ),
            (
                "Mat{n}x{m}",
                "generic real matrix type",
                CompletionItemKind::CLASS,
            ),
            (
                "E{n}{m}",
                "generic matrix basis element",
                CompletionItemKind::CONSTANT,
            ),
        ] {
            items.push(CompletionItem {
                label: label.to_string(),
                kind: Some(kind),
                detail: Some(detail.to_string()),
                ..CompletionItem::default()
            });
        }
        for primitive in lane::known_primitives() {
            let fields = primitive
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, field.domain))
                .collect::<Vec<_>>()
                .join(", ");
            items.push(CompletionItem {
                label: primitive.name,
                kind: Some(CompletionItemKind::CONSTRUCTOR),
                detail: Some(format!("{}({fields})", primitive.parameter_space)),
                documentation: Some(tower_lsp::lsp_types::Documentation::String(format!(
                    "{} primitive constructor",
                    primitive.dimension.label()
                ))),
                ..CompletionItem::default()
            });
        }
        for object in lane::known_builtin_objects() {
            items.push(CompletionItem {
                label: object.name,
                kind: Some(match object.kind {
                    lane::KnownBuiltinObjectKind::Function => CompletionItemKind::FUNCTION,
                    lane::KnownBuiltinObjectKind::Type => CompletionItemKind::CLASS,
                    lane::KnownBuiltinObjectKind::Category => CompletionItemKind::INTERFACE,
                }),
                detail: Some(object.ty),
                ..CompletionItem::default()
            });
        }
        items
    }

    fn hover_for_word(word: &str) -> Option<String> {
        if let Some(primitive) = lane::known_primitive(word) {
            let fields = primitive
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, field.domain))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "{}: {}\n\n{} primitive constructor with fields: {}",
                primitive.name,
                primitive.parameter_space,
                primitive.dimension.label(),
                fields
            ));
        }
        if let Some(object) = lane::known_builtin_object(word) {
            return Some(format!("{}: {}", object.name, object.ty));
        }
        match word {
            "const" => Some("const emits a Lane value, function, or object binding.".to_string()),
            "provided" => Some("provided declares a host-provided shader input.".to_string()),
            "Hom" | "Func" => Some(format!("{word}(A, B) is a function type from A to B.")),
            "Object" => Some("Object is the current ambient SDF object type.".to_string()),
            "Object2D" => Some("Object2D is a 2D SDF object type.".to_string()),
            "Object3D" => Some("Object3D is a 3D SDF object type.".to_string()),
            "R" => Some(
                "R is the real scalar type; R{n} denotes generic real vector spaces.".to_string(),
            ),
            "Mat" => Some("Mat{n}x{m} denotes generic real matrix types.".to_string()),
            "E" => Some("E{n}{m} denotes generic matrix basis elements.".to_string()),
            _ => None,
        }
    }

    fn word_at_position(text: &str, position: Position) -> Option<String> {
        let line = text.lines().nth(position.line as usize)?;
        let chars = line.chars().collect::<Vec<_>>();
        let mut index = (position.character as usize).min(chars.len());
        if index == chars.len() && index > 0 {
            index -= 1;
        }
        if index >= chars.len() {
            return None;
        }
        while index > 0 && !is_word_char(chars[index]) && is_word_char(chars[index - 1]) {
            index -= 1;
        }
        if !is_word_char(chars[index]) {
            return None;
        }
        let mut start = index;
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < chars.len() && is_word_char(chars[end]) {
            end += 1;
        }
        Some(chars[start..end].iter().collect())
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["#".to_string(), ".".to_string()]),
                    ..CompletionOptions::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "lane-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents.set(uri.clone(), text.clone()).await;
        self.publish_diagnostics(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = Self::newest_text(params.content_changes) {
            self.documents.set(uri.clone(), text.clone()).await;
            self.publish_diagnostics(uri, text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .text
            .write()
            .await
            .remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            self.documents
                .set(params.text_document.uri.clone(), text.clone())
                .await;
            self.publish_diagnostics(params.text_document.uri, text)
                .await;
            return;
        }

        if let Some(text) = self.documents.get(&params.text_document.uri).await {
            self.publish_diagnostics(params.text_document.uri, text)
                .await;
        }
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(Self::completion_items())))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(word) = Self::word_at_position(&text, position) else {
            return Ok(None);
        };
        let Some(contents) = Self::hover_for_word(&word) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(contents)),
            range: None,
        }))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::new(DocumentStore::default()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_word_at_lsp_position() {
        let text = "const Object scene = Ball3D(r=1)\n";
        let position = Position::new(0, 24);

        assert_eq!(
            Backend::word_at_position(text, position).as_deref(),
            Some("Ball3D")
        );
    }

    #[test]
    fn hovers_known_primitive() {
        let hover = Backend::hover_for_word("Ball3D").unwrap();

        assert!(hover.contains("Ball3D"));
        assert!(hover.contains("r: R"));
    }

    #[test]
    fn completes_new_language_surface() {
        let labels = Backend::completion_items()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label == "#import"));
        assert!(labels.iter().any(|label| label == "raytracing"));
        assert!(labels.iter().any(|label| label == "Ball3D"));
        assert!(labels.iter().any(|label| label == "Mat{n}x{m}"));
    }
}
