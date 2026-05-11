use tower_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use tree_sitter::{Node, Parser};
use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_lane() -> *const ();
}

const LANGUAGE_LANE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_lane) };

pub fn symbols(source: &str) -> Vec<DocumentSymbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&LANGUAGE_LANE.into())
        .expect("lane tree-sitter parser should load");
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let line_start_bytes = line_start_bytes(source);
    let mut cursor = tree.walk();
    tree.root_node()
        .named_children(&mut cursor)
        .flat_map(|node| symbols_for_declaration(source, &line_start_bytes, node))
        .collect()
}

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

fn binding_symbol_kind(source: &str, node: Node<'_>) -> SymbolKind {
    if modifier_text(source, node).as_deref() == Some("const") {
        SymbolKind::CONSTANT
    } else {
        SymbolKind::VARIABLE
    }
}

fn modifier_text(source: &str, node: Node<'_>) -> Option<String> {
    let modifier = node.child_by_field_name("modifier")?;
    node_text(source, modifier)
}

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

fn node_text(source: &str, node: Node<'_>) -> Option<String> {
    node.utf8_text(source.as_bytes()).ok().map(str::to_string)
}

fn node_range(source: &str, line_start_bytes: &[usize], node: Node<'_>) -> Range {
    Range::new(
        byte_to_position(source, line_start_bytes, node.start_byte()),
        byte_to_position(source, line_start_bytes, node.end_byte()),
    )
}

fn line_start_bytes(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

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
