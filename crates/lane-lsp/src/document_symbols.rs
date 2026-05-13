use tower_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use tree_sitter::Node;

/// Returns top-level document symbols discovered from a Lane source buffer.
pub fn symbols(source: &str) -> Vec<DocumentSymbol> {
    let Some(tree) = super::parse_lane_tree(source) else {
        return Vec::new();
    };

    let line_start_bytes = super::line_start_bytes(source);
    let mut cursor = tree.walk();
    tree.root_node()
        .named_children(&mut cursor)
        .flat_map(|node| symbols_for_declaration(source, &line_start_bytes, node))
        .collect()
}

/// Collects symbols for one top-level declaration node.
fn symbols_for_declaration(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
) -> Vec<DocumentSymbol> {
    match node.kind() {
        "directive" => directive_symbol(source, line_start_bytes, node)
            .into_iter()
            .collect(),
        "provided_category_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::INTERFACE)
        }
        "category_type_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::STRUCT)
        }
        "product_type_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::STRUCT)
        }
        "input_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::VARIABLE)
        }
        "arrow_function_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::FUNCTION)
        }
        "binding_declaration" => named_field_symbols(
            source,
            line_start_bytes,
            node,
            binding_symbol_kind(source, node),
        ),
        "inferred_binding_declaration" => named_field_symbols(
            source,
            line_start_bytes,
            node,
            binding_symbol_kind(source, node),
        ),
        _ => Vec::new(),
    }
}

/// Produces a namespace/module symbol entry for a directive declaration.
fn directive_symbol(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
) -> Option<DocumentSymbol> {
    let mut cursor = node.walk();
    let name = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "directive_token")?;
    let name_text = node_text(source, name)?;
    let kind = if name_text == "#module" {
        SymbolKind::MODULE
    } else {
        SymbolKind::NAMESPACE
    };
    Some(make_symbol(source, line_start_bytes, node, name, kind))
}

/// Produces symbol entries for declaration nodes that carry named fields.
fn named_field_symbols(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
    kind: SymbolKind,
) -> Vec<DocumentSymbol> {
    let mut cursor = node.walk();
    node.children_by_field_name("name", &mut cursor)
        .filter(|child| child.is_named())
        .filter_map(|name| {
            if node_text(source, name).is_some() {
                Some(make_symbol(source, line_start_bytes, node, name, kind))
            } else {
                None
            }
        })
        .collect()
}

/// Determines whether a binding should be surfaced as a constant or variable symbol.
fn binding_symbol_kind(source: &str, node: Node<'_>) -> SymbolKind {
    if modifier_text(source, node).as_deref() == Some("const") {
        SymbolKind::CONSTANT
    } else {
        SymbolKind::VARIABLE
    }
}

/// Extracts the optional declaration modifier text from a declaration node.
fn modifier_text(source: &str, node: Node<'_>) -> Option<String> {
    let modifier = node.child_by_field_name("modifier")?;
    node_text(source, modifier)
}

/// Builds a fully-populated symbol from an arbitrary named declaration node.
fn make_symbol(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
    name: Node<'_>,
    kind: SymbolKind,
) -> DocumentSymbol {
    DocumentSymbol {
        name: node_text(source, name).unwrap_or_default(),
        detail: Some(detail_for_node(node).to_string()),
        kind,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range: node_range(source, line_start_bytes, node),
        selection_range: node_range(source, line_start_bytes, name),
        children: None,
    }
}

/// Maps declaration kinds to human-readable symbol details for the UI.
fn detail_for_node(node: Node<'_>) -> &'static str {
    match node.kind() {
        "directive" => "directive",
        "provided_category_declaration" => "provided category",
        "category_type_declaration" => "category type",
        "product_type_declaration" => "product type",
        "input_declaration" => "provided input",
        "arrow_function_declaration" => "provided function",
        "binding_declaration" => "binding",
        "inferred_binding_declaration" => "inferred binding",
        _ => "declaration",
    }
}

/// Reads the UTF-8 text covered by a Tree-sitter node, when available.
fn node_text(source: &str, node: Node<'_>) -> Option<String> {
    node.utf8_text(source.as_bytes()).ok().map(str::to_string)
}

/// Returns a symbol range for the given node based on precomputed line starts.
fn node_range(source: &str, line_start_bytes: &[usize], node: Node<'_>) -> Range {
    Range::new(
        byte_to_position(source, line_start_bytes, node.start_byte()),
        byte_to_position(source, line_start_bytes, node.end_byte()),
    )
}

/// Converts a byte offset to LSP position using `line_start_bytes`.
fn byte_to_position(source: &str, line_start_bytes: &[usize], byte: usize) -> Position {
    let line = match line_start_bytes.binary_search(&byte) {
        Ok(line) => line,
        Err(next_line) => next_line.saturating_sub(1),
    };
    let line_start = line_start_bytes.get(line).copied().unwrap_or(0);
    let byte = byte.min(source.len());
    let character = source[line_start..byte]
        .chars()
        .map(char::len_utf16)
        .sum::<usize>();
    Position::new(line as u32, character as u32)
}
