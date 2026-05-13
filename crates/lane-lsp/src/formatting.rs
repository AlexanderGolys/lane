//! Implements LSP formatting helpers for Lane documents.
//! Formatting is isolated from semantic compiler passes because it rewrites source presentation while preserving meaning.
//! It runs in the editor tooling pipeline when the LSP receives formatting requests.

use tower_lsp::lsp_types::{Position, Range};

/// Returns a full-document range from first to last line for formatting edits.
pub(crate) fn whole_document_range(text: &str) -> Range {
    let line_count = text.lines().count() as u32;
    Range::new(
        Position::new(0, 0),
        Position::new(line_count.saturating_add(1), 0),
    )
}
