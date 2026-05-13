use std::sync::Arc;

mod document_symbols;
mod formatting;
mod links;
mod position;
mod semantic_tokens;
mod signature;

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
    ServerCapabilities, SignatureHelp, SignatureHelpOptions, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
    Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Parser;
use tree_sitter_language::LanguageFn;

// Binds the generated Lane grammar language function for this process.
unsafe extern "C" {
    // Declares the C ABI constructor for Lane's generated tree-sitter parser.
    fn tree_sitter_lane() -> *const ();
}

const LANGUAGE_LANE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_lane) };

#[derive(Debug, Default)]
struct DocumentStore {
    text: RwLock<std::collections::HashMap<Url, String>>,
}

impl DocumentStore {
    /// Stores updated document text for an opened URI in memory.
    async fn set(&self, uri: Url, text: String) {
        self.text.write().await.insert(uri, text);
    }

    /// Loads cached document text for an opened URI.
    async fn get(&self, uri: &Url) -> Option<String> {
        self.text.read().await.get(uri).cloned()
    }
}

struct Backend {
    client: Client,
    documents: Arc<DocumentStore>,
}

impl Backend {
    /// Resolves a document URI into its directory for import and path resolution.
    fn base_dir(uri: &Url) -> std::path::PathBuf {
        uri.to_file_path()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()))
    }

    /// Sends lane compiler diagnostics for a full document to the LSP client.
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

    /// Picks the latest change text from a full document change notification.
    fn newest_text(changes: Vec<TextDocumentContentChangeEvent>) -> Option<String> {
        changes.into_iter().last().map(|change| change.text)
    }

    /// Converts lane completion catalog entries into LSP completion items.
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

    /// Looks up hover text for a word using the compiler hover index.
    fn hover_for_word(word: &str) -> Option<String> {
        lane::lane_hover_for_word(word)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// Advertises server capabilities and configures supported LSP features.
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
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    ..SignatureHelpOptions::default()
                }),
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

    /// Receives post-init notification and confirms server readiness.
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "lane-lsp initialized")
            .await;
    }

    /// Handles shutdown requests.
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// Stores opened document text and emits diagnostics.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents.set(uri.clone(), text.clone()).await;
        self.publish_diagnostics(uri, text).await;
    }

    /// Applies latest document text change and refreshes diagnostics.
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = Self::newest_text(params.content_changes) {
            self.documents.set(uri.clone(), text.clone()).await;
            self.publish_diagnostics(uri, text).await;
        }
    }

    /// Removes cached content and clears diagnostics when document is closed.
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

    /// Republishes diagnostics from latest saved state or provided snapshot.
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

    /// Returns completion items for identifiers in position-free completion requests.
    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(Self::completion_items())))
    }

    /// Returns hover content for the token under the cursor.
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(word) = position::word_at_position(&text, position) else {
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

    /// Returns function signature help for active call expression context.
    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(context) = signature::call_context_at_position(&text, position) else {
            return Ok(None);
        };
        Ok(signature::signature_help_for_context(&context))
    }

    /// Rewrites the full document with formatted Lane source when changed.
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
            range: formatting::whole_document_range(&text),
            new_text: formatted,
        }]))
    }

    /// Produces hierarchical document symbols from parser declarations.
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

    /// Produces import links for `#import` directives in the current document.
    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        Ok(Some(links::import_links(&text, Self::base_dir(&uri))))
    }

    /// Returns semantic tokens for the whole document.
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

/// Maps lane completion kinds to LSP completion item kinds.
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

/// Parses Lane source into a Tree-sitter tree when possible.
fn parse_lane_tree(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE_LANE.into()).unwrap_or_else(|error| {
        panic!("lane tree-sitter parser must load for LSP startup: {error}")
    });
    parser.parse(source, None)
}

/// Precomputes start byte offsets for each source line.
fn line_start_bytes(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

/// Starts the LSP server stdin/stdout loop.
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
#[path = "../tests/unit/lsp.rs"]
mod tests;
