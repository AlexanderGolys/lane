use std::sync::Arc;

mod document_symbols;
mod semantic_tokens;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams, DocumentLink,
    DocumentLinkOptions, DocumentLinkParams, DocumentSymbolParams, DocumentSymbolResponse, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult,
    InitializedParams, MarkedString, MessageType, OneOf, Position, Range,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    ServerCapabilities, TextDocumentContentChangeEvent, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url,
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

    fn import_links(text: &str, base_dir: impl AsRef<std::path::Path>) -> Vec<DocumentLink> {
        let base_dir = base_dir.as_ref();
        text.lines()
            .enumerate()
            .filter_map(|(line_index, line)| import_link_for_line(line, line_index, base_dir))
            .collect()
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
                document_symbol_provider: Some(OneOf::Left(true)),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensOptions {
                        legend: semantic_tokens::legend(),
                        range: None,
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        ..SemanticTokensOptions::default()
                    }
                    .into(),
                ),
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

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(
            document_symbols::symbols(&text),
        )))
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        Ok(Some(Self::import_links(&text, Self::base_dir(&uri))))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        Ok(Some(semantic_tokens::tokens(&text).into()))
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

fn import_link_for_line(
    line: &str,
    line_index: usize,
    base_dir: &std::path::Path,
) -> Option<DocumentLink> {
    let directive_start = line.find("#import")?;
    if !line[..directive_start].trim().is_empty() {
        return None;
    }

    let rest_start = directive_start + "#import".len();
    let path_start_offset = line[rest_start..].find(|ch: char| !ch.is_whitespace())?;
    let mut path_start = rest_start + path_start_offset;
    let mut path_end = line[path_start..]
        .find(|ch: char| ch.is_whitespace())
        .map(|offset| path_start + offset)
        .unwrap_or(line.len());

    if line[path_start..].starts_with('"') {
        path_start += 1;
        let quoted_end = line[path_start..].find('"')?;
        path_end = path_start + quoted_end;
    }

    let import_path = line[path_start..path_end].trim();
    if import_path.is_empty() {
        return None;
    }

    let target_path = lane::resolve_import_path(import_path, base_dir).ok()?;
    let target = Url::from_file_path(target_path).ok()?;
    let line = line_index as u32;
    Some(DocumentLink {
        range: Range::new(
            Position::new(line, path_start as u32),
            Position::new(line, path_end as u32),
        ),
        target: Some(target),
        tooltip: Some(format!("Open Lane module {import_path}")),
        data: None,
    })
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
mod tests;
