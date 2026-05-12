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
    InitializedParams, MarkedString, MessageType, OneOf, ParameterInformation, ParameterLabel,
    Position, Range, SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, ServerCapabilities, SignatureHelp, SignatureHelpOptions,
    SignatureHelpParams, SignatureInformation, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct CallContext {
    name: String,
    active_parameter: u32,
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

    fn signature_help_for_context(context: &CallContext) -> Option<SignatureHelp> {
        let signature = signature_for_name(&context.name)?;
        Some(SignatureHelp {
            signatures: vec![signature],
            active_signature: Some(0),
            active_parameter: Some(context.active_parameter),
        })
    }

    fn call_context_at_position(text: &str, position: Position) -> Option<CallContext> {
        let offset = byte_offset_for_position(text, position)?;
        let prefix = &text[..offset];
        let open = innermost_open_paren(prefix)?;
        let name = call_name_before(prefix, open)?;
        Some(CallContext {
            name,
            active_parameter: active_parameter_index(&prefix[open + 1..]),
        })
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

fn byte_offset_for_position(text: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let line_text = line.strip_suffix('\n').unwrap_or(line);
        if line_index == position.line as usize {
            return Some(
                offset + byte_offset_for_character(line_text, position.character as usize),
            );
        }
        offset += line.len();
    }
    if position.line as usize == text.lines().count() {
        return Some(text.len());
    }
    None
}

fn byte_offset_for_character(line: &str, character: usize) -> usize {
    let mut utf16_units = 0;
    for (index, ch) in line.char_indices() {
        if utf16_units >= character {
            return index;
        }
        utf16_units += ch.len_utf16();
    }
    line.len()
}

fn innermost_open_paren(prefix: &str) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in prefix.char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn call_name_before(prefix: &str, open_paren: usize) -> Option<String> {
    let before = prefix[..open_paren].trim_end();
    let end = before.len();
    let start = before[..end]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_word_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| before[start..end].to_string())
}

fn active_parameter_index(source: &str) -> u32 {
    let mut active = 0;
    let mut depth = 0u32;
    for ch in source.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ',' if depth == 0 => active += 1,
            _ => {}
        }
    }
    active
}

fn signature_for_name(name: &str) -> Option<SignatureInformation> {
    if let Some(primitive) = lane::known_primitive(name) {
        let params = primitive
            .fields
            .into_iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>();
        return Some(signature_information(
            format!("{}({})", primitive.name, params.join(", ")),
            params,
            Some(format!(
                "{} primitive constructor",
                primitive.dimension.label()
            )),
        ));
    }

    let object = lane::known_builtin_object(name)?;
    if object.kind != lane::KnownBuiltinObjectKind::Function {
        return None;
    }
    signature_from_function_type(&object.name, &object.ty)
}

fn signature_from_function_type(name: &str, ty: &str) -> Option<SignatureInformation> {
    let (domain, codomain) = hom_domain_and_codomain(ty)?;
    let params = parameter_labels_from_domain(&domain);
    Some(signature_information(
        format!("{}({}) -> {}", name, params.join(", "), codomain),
        params,
        Some(ty.to_string()),
    ))
}

fn hom_domain_and_codomain(ty: &str) -> Option<(String, String)> {
    let ty = ty.trim();
    let inner = ty
        .strip_prefix("Hom(")
        .or_else(|| ty.strip_prefix("Func("))?
        .strip_suffix(')')?;
    split_top_level_comma(inner)
}

fn split_top_level_comma(source: &str) -> Option<(String, String)> {
    let mut depth = 0u32;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                let left = source[..index].trim().to_string();
                let right = source[index + 1..].trim().to_string();
                return Some((left, right));
            }
            _ => {}
        }
    }
    None
}

fn parameter_labels_from_domain(domain: &str) -> Vec<String> {
    if domain == "*" {
        return Vec::new();
    }
    let parts = split_top_level_product(domain);
    if parts.is_empty() {
        vec![domain.to_string()]
    } else {
        parts
    }
}

fn split_top_level_product(source: &str) -> Vec<String> {
    let mut depth = 0u32;
    let mut parts = Vec::new();
    let mut start = 0;
    let chars = source.char_indices().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let (byte, ch) = chars[index];
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            'x' | '×' if depth == 0 && product_separator_at(source, byte, ch) => {
                parts.push(source[start..byte].trim().to_string());
                start = byte + ch.len_utf8();
            }
            _ => {}
        }
        index += 1;
    }
    if start > 0 {
        parts.push(source[start..].trim().to_string());
    }
    parts.retain(|part| !part.is_empty());
    parts
}

fn product_separator_at(source: &str, byte: usize, ch: char) -> bool {
    if ch == '×' {
        return true;
    }
    let before = source[..byte].chars().next_back();
    let after = source[byte + ch.len_utf8()..].chars().next();
    before.is_some_and(char::is_whitespace) && after.is_some_and(char::is_whitespace)
}

fn signature_information(
    label: String,
    params: Vec<String>,
    documentation: Option<String>,
) -> SignatureInformation {
    SignatureInformation {
        label,
        documentation: documentation.map(tower_lsp::lsp_types::Documentation::String),
        parameters: Some(
            params
                .into_iter()
                .map(|label| ParameterInformation {
                    label: ParameterLabel::Simple(label),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: None,
    }
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

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(text) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(context) = Self::call_context_at_position(&text, position) else {
            return Ok(None);
        };
        Ok(Self::signature_help_for_context(&context))
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
