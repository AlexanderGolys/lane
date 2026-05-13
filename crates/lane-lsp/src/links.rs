use std::path::Path;

use tower_lsp::lsp_types::{DocumentLink, Position, Range, Url};

/// Builds import links for all valid `#import` directives in a source file.
pub(crate) fn import_links(text: &str, base_dir: impl AsRef<Path>) -> Vec<DocumentLink> {
    let base_dir = base_dir.as_ref();
    text.lines()
        .enumerate()
        .filter_map(|(line_index, line)| import_link_for_line(line, line_index, base_dir))
        .collect()
}

/// Builds one document link from a single `#import` line, if valid.
fn import_link_for_line(line: &str, line_index: usize, base_dir: &Path) -> Option<DocumentLink> {
    let directive_start = line.find("#import")?;
    if !line[..directive_start].trim().is_empty() {
        return None;
    }
    let (path_start, path_end) = import_path_span(line, directive_start)?;
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

/// Locates the span of an import path in one `#import` directive line.
fn import_path_span(line: &str, directive_start: usize) -> Option<(usize, usize)> {
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
    Some((path_start, path_end))
}
