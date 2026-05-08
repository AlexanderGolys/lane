use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult,
    InitializedParams, MarkedString, MessageType, OneOf, Position, Range, ServerCapabilities,
    TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
    Url,
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
        let diagnostics = lane::lane_diagnostics_with_base_dir(&text, Self::base_dir(&uri))
            .into_iter()
            .map(|diagnostic| {
                let line = diagnostic.line.saturating_sub(1) as u32;
                Diagnostic {
                    range: Range::new(Position::new(line, 0), Position::new(line, 1)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: diagnostic.message,
                    ..Diagnostic::default()
                }
            })
            .collect();
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    fn newest_text(changes: Vec<TextDocumentContentChangeEvent>) -> Option<String> {
        changes.into_iter().last().map(|change| change.text)
    }

    fn completion_items() -> Vec<CompletionItem> {
        lane::lane_completion_items()
            .into_iter()
            .map(|item| CompletionItem {
                label: item.label,
                kind: Some(completion_kind(item.kind)),
                detail: item.detail,
                documentation: item
                    .documentation
                    .map(tower_lsp::lsp_types::Documentation::String),
                ..CompletionItem::default()
            })
            .collect()
    }

    fn hover_for_word(word: &str) -> Option<String> {
        lane::lane_hover_for_word(word)
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
                document_formatting_provider: Some(OneOf::Left(true)),
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

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let formatted = lane::format_lane_source(&text);
        if formatted == text {
            return Ok(Some(Vec::new()));
        }
        Ok(Some(vec![TextEdit {
            range: whole_document_range(&text),
            new_text: formatted,
        }]))
    }
}

fn completion_kind(kind: lane::LaneCompletionKind) -> CompletionItemKind {
    match kind {
        lane::LaneCompletionKind::Keyword => CompletionItemKind::KEYWORD,
        lane::LaneCompletionKind::Module => CompletionItemKind::MODULE,
        lane::LaneCompletionKind::Constructor => CompletionItemKind::CONSTRUCTOR,
        lane::LaneCompletionKind::Function => CompletionItemKind::FUNCTION,
        lane::LaneCompletionKind::Type => CompletionItemKind::CLASS,
        lane::LaneCompletionKind::Category => CompletionItemKind::INTERFACE,
        lane::LaneCompletionKind::Constant => CompletionItemKind::CONSTANT,
    }
}

fn whole_document_range(text: &str) -> Range {
    let line_count = text.lines().count() as u32;
    Range::new(
        Position::new(0, 0),
        Position::new(line_count.saturating_add(1), 0),
    )
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

    #[test]
    fn formats_whole_document_range() {
        let range = whole_document_range("R radius = 1\nconst R diameter = 2\n");

        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end.line, 3);
    }
}
