//! Extracts LSP document symbols from Lane source.
//! Symbol extraction is separated from diagnostics and semantic tokens because editors need a structural outline built from syntax positions rather than typed compiler output.
//! It belongs to the LSP/editor tooling stage after parsing source text for navigation features.

use crate::position;
use tower_lsp::lsp_types::{DocumentSymbol, Range, SymbolKind};
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
        "category_type_declaration" | "product_type_declaration" => {
            type_declaration_head_symbol(source, line_start_bytes, node, SymbolKind::STRUCT)
                .into_iter()
                .collect()
        }
        "input_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::VARIABLE)
        }
        "arrow_function_declaration" => {
            named_field_symbols(source, line_start_bytes, node, SymbolKind::FUNCTION)
        }
        "binding_declaration" => last_identifier_before_equals_symbol(
            source,
            line_start_bytes,
            node,
            binding_symbol_kind(source, node),
        )
        .into_iter()
        .collect(),
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
    make_symbol(source, line_start_bytes, node, name, kind)
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
        .filter(|name| name.is_named())
        .filter(|name| {
            name.parent()
                .is_none_or(|parent| parent.kind() != "product_field_list")
        })
        .filter(|name| !is_inside_angle_list_before_equals(source, node, *name))
        .filter_map(|name| make_symbol(source, line_start_bytes, node, name, kind))
        .collect()
}

/// Returns true for generic/product-field names that should not become outline symbols.
fn is_inside_angle_list_before_equals(source: &str, node: Node<'_>, name: Node<'_>) -> bool {
    let before_name = &source[node.start_byte()..name.start_byte()];
    let after_name = &source[name.end_byte()..node.end_byte()];
    let last_angle = before_name.rfind('<');
    let last_equals = before_name.rfind('=');
    last_angle > last_equals && after_name.find('>') < after_name.find('=')
}

/// Produces the declaration symbol for typed value/function bindings.
fn last_identifier_before_equals_symbol(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
    let equals = source[node.start_byte()..node.end_byte()]
        .find('=')
        .map(|offset| node.start_byte() + offset)
        .unwrap_or(node.end_byte());
    let mut cursor = node.start_byte();
    let mut seen = Vec::new();
    while cursor < equals {
        let Some(start_offset) = source[cursor..equals]
            .char_indices()
            .find_map(|(offset, ch)| (ch == '_' || ch.is_ascii_alphabetic()).then_some(offset))
        else {
            break;
        };
        let start = cursor + start_offset;
        let end = source[start..equals]
            .char_indices()
            .find_map(|(offset, ch)| {
                (!(ch == '_' || ch.is_ascii_alphanumeric())).then_some(start + offset)
            })
            .unwrap_or(equals);
        seen.push((start, end));
        cursor = end;
    }
    let angle = source[node.start_byte()..equals]
        .find('<')
        .map(|offset| node.start_byte() + offset);
    let (name_start, name_end, kind) = if let Some(angle) = angle {
        let (start, end) = seen.into_iter().rev().find(|(_, end)| *end <= angle)?;
        (start, end, SymbolKind::STRUCT)
    } else {
        seen.into_iter()
            .last()
            .map(|(start, end)| (start, end, kind))?
    };
    make_text_symbol(source, line_start_bytes, node, name_start, name_end, kind)
}

/// Produces the declaration-head symbol for type declarations with nested named parameters.
fn type_declaration_head_symbol(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
    let mut cursor = node.start_byte();
    let mut seen = Vec::new();
    while cursor < node.end_byte() {
        let Some(start_offset) = source[cursor..node.end_byte()]
            .char_indices()
            .find_map(|(offset, ch)| (ch == '_' || ch.is_ascii_alphabetic()).then_some(offset))
        else {
            break;
        };
        let start = cursor + start_offset;
        let end = source[start..node.end_byte()]
            .char_indices()
            .find_map(|(offset, ch)| {
                (!(ch == '_' || ch.is_ascii_alphanumeric())).then_some(start + offset)
            })
            .unwrap_or(node.end_byte());
        seen.push((start, end));
        cursor = end;
        if source
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| *byte == b'<')
        {
            break;
        }
    }
    let name_index = if seen
        .first()
        .is_some_and(|(start, end)| source[*start..*end].eq("const"))
    {
        2
    } else {
        1
    };
    let (name_start, name_end) = seen.get(name_index).copied()?;
    make_text_symbol(source, line_start_bytes, node, name_start, name_end, kind)
}

/// Builds a symbol whose declaration name comes from explicit source byte bounds.
fn make_text_symbol(
    source: &str,
    line_start_bytes: &[usize],
    node: Node<'_>,
    name_start: usize,
    name_end: usize,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
    let name = source[name_start..name_end].to_string();
    Some(DocumentSymbol {
        name,
        detail: Some(detail_for_node(node).to_string()),
        kind,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range: node_range(source, line_start_bytes, node),
        selection_range: byte_range(source, line_start_bytes, name_start, name_end),
        children: None,
    })
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
    name_node: Node<'_>,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
    let name = node_text(source, name_node)?;
    Some(DocumentSymbol {
        name,
        detail: Some(detail_for_node(node).to_string()),
        kind,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range: node_range(source, line_start_bytes, node),
        selection_range: node_range(source, line_start_bytes, name_node),
        children: None,
    })
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
    byte_range(source, line_start_bytes, node.start_byte(), node.end_byte())
}

/// Returns a range for explicit byte bounds based on precomputed line starts.
fn byte_range(source: &str, line_start_bytes: &[usize], start: usize, end: usize) -> Range {
    Range::new(
        position::byte_to_position(source, line_start_bytes, start),
        position::byte_to_position(source, line_start_bytes, end),
    )
}
