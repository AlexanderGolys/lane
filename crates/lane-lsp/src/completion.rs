//! Owns Lane completion data for editor and interactive tooling.
//! The compiler exposes semantic indexes such as known primitives and built-in objects; this layer decides which labels become completions.

use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemKind, Documentation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Module,
    Constructor,
    Function,
    Type,
    Category,
    Constant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>, kind: CompletionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            documentation: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }
}

/// Builds the completion catalogue shared by lane-lsp and REPL tooling.
pub fn items() -> Vec<CompletionItem> {
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
        items.push(CompletionItem::new(label, CompletionKind::Keyword).with_detail(detail));
    }
    for module in ["std", "complex", "quat", "raytracing"] {
        items.push(
            CompletionItem::new(module, CompletionKind::Module).with_detail("built-in Lane module"),
        );
    }
    for (label, detail, kind) in [
        (
            "Mat{n}x{m}",
            "generic real matrix type",
            CompletionKind::Type,
        ),
        (
            "Mat{n}",
            "generic square real matrix type",
            CompletionKind::Type,
        ),
        (
            "E{n}{m}",
            "generic matrix basis element",
            CompletionKind::Constant,
        ),
    ] {
        items.push(CompletionItem::new(label, kind).with_detail(detail));
    }
    for primitive in lane::known_primitives() {
        let fields = primitive
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(
            CompletionItem::new(primitive.name, CompletionKind::Constructor)
                .with_detail(format!("{}({fields})", primitive.parameter_space))
                .with_documentation(format!(
                    "{} primitive constructor",
                    primitive.dimension.label()
                )),
        );
    }
    for object in lane::known_builtin_objects() {
        let kind = match object.kind {
            lane::KnownBuiltinObjectKind::Function => CompletionKind::Function,
            lane::KnownBuiltinObjectKind::Type => CompletionKind::Type,
            lane::KnownBuiltinObjectKind::Category => CompletionKind::Category,
        };
        items.push(CompletionItem::new(object.name, kind).with_detail(object.ty));
    }
    items
}

/// Builds LSP protocol completion items.
pub fn lsp_items() -> Vec<LspCompletionItem> {
    items()
        .into_iter()
        .map(|item| LspCompletionItem {
            label: item.label,
            kind: Some(completion_item_kind(item.kind)),
            detail: item.detail,
            documentation: item.documentation.map(Documentation::String),
            ..LspCompletionItem::default()
        })
        .collect()
}

fn completion_item_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::Module => CompletionItemKind::MODULE,
        CompletionKind::Constructor => CompletionItemKind::CONSTRUCTOR,
        CompletionKind::Function => CompletionItemKind::FUNCTION,
        CompletionKind::Type => CompletionItemKind::CLASS,
        CompletionKind::Category => CompletionItemKind::INTERFACE,
        CompletionKind::Constant => CompletionItemKind::CONSTANT,
    }
}
