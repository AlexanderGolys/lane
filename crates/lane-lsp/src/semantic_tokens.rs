//! Produces LSP semantic tokens for Lane syntax.
//! Semantic highlighting is separated from compiler typechecking because editor coloring needs fast source classification with Lane-specific taxonomy.
//! It runs in the LSP tooling pipeline after reading document text and before returning semantic-token responses.

use std::collections::HashSet;

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};
use tree_sitter::{Query, QueryCapture, QueryCursor, StreamingIterator};

const QUERY: &str = include_str!("../../../tree-sitter-lane/queries/semantic_tokens.scm");

const TOKEN_TYPE_DIRECTIVE: u32 = 0;
const TOKEN_TYPE_FUNCTOR: u32 = 1;
const TOKEN_TYPE_NAMESPACE: u32 = 2;
const TOKEN_TYPE_TYPE: u32 = 3;
const TOKEN_TYPE_TYPE_PARAMETER: u32 = 4;
const TOKEN_TYPE_FUNCTION: u32 = 5;
const TOKEN_TYPE_PARAMETER: u32 = 6;
const TOKEN_TYPE_VARIABLE: u32 = 7;
const TOKEN_TYPE_PROPERTY: u32 = 8;
const TOKEN_TYPE_NUMBER: u32 = 9;
const TOKEN_TYPE_OPERATOR: u32 = 10;
const TOKEN_TYPE_COMMENT: u32 = 11;
const TOKEN_TYPE_STRING: u32 = 12;
const TOKEN_TYPE_KEYWORD: u32 = 13;
const TOKEN_TYPE_CATEGORY: u32 = 14;

const TOKEN_MODIFIER_DECLARATION: u32 = 1 << 0;
const TOKEN_MODIFIER_DEFAULT_LIBRARY: u32 = 1 << 1;

/// Token classification payload used while collecting semantic tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenSpec {
    token_type: u32,
    token_modifiers_bitset: u32,
}

/// Fully positioned token in absolute document coordinates before delta encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

/// Returns the semantic token legend consumed by LSP clients.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::new("directive"),
            SemanticTokenType::new("functor"),
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::TYPE_PARAMETER,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::NUMBER,
            SemanticTokenType::OPERATOR,
            SemanticTokenType::COMMENT,
            SemanticTokenType::STRING,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::new("category"),
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFAULT_LIBRARY,
        ],
    }
}

/// Runs semantic-token queries over a source file and encodes token positions.
pub fn tokens(source: &str) -> SemanticTokens {
    let Some(tree) = super::parse_lane_tree(source) else {
        return SemanticTokens {
            result_id: None,
            data: Vec::new(),
        };
    };

    let query = match Query::new(&super::LANGUAGE_LANE.into(), QUERY) {
        Ok(query) => query,
        Err(error) => panic!("lane semantic token query should compile: {error}"),
    };
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut absolute_tokens = Vec::new();
    let mut seen = HashSet::new();
    let line_start_bytes = super::line_start_bytes(source);

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            let capture_name = &capture_names[capture.index as usize];
            let Some(spec) = token_spec(capture_name, *capture, source) else {
                continue;
            };
            let range = capture.node.byte_range();
            let key = (
                range.start,
                range.end,
                spec.token_type,
                spec.token_modifiers_bitset,
            );
            if !seen.insert(key) {
                continue;
            }
            push_token_segments(
                &mut absolute_tokens,
                source,
                &line_start_bytes,
                *capture,
                spec,
            );
        }
    }

    absolute_tokens.sort_by_key(|token| (token.line, token.start, token.length, token.token_type));
    let mut previous_line = 0;
    let mut previous_start = 0;
    let data = absolute_tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line.saturating_sub(previous_line);
            let delta_start = if delta_line == 0 {
                token.start.saturating_sub(previous_start)
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;
            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            }
        })
        .collect();

    SemanticTokens {
        result_id: None,
        data,
    }
}

/// Maps a Tree-sitter capture to token type + modifiers, or skips unsupported captures.
fn token_spec(capture_name: &str, capture: QueryCapture<'_>, source: &str) -> Option<TokenSpec> {
    let text = &source[capture.node.byte_range()];
    let mut token_modifiers_bitset = 0;
    let token_type = match capture_name {
        "comment" => TOKEN_TYPE_COMMENT,
        "number" => TOKEN_TYPE_NUMBER,
        "string" => TOKEN_TYPE_STRING,
        "operator" if matches!(text, "|->") => TOKEN_TYPE_FUNCTOR,
        "operator" => TOKEN_TYPE_OPERATOR,
        "keyword" => TOKEN_TYPE_KEYWORD,
        "directive" => TOKEN_TYPE_DIRECTIVE,
        "namespace" => TOKEN_TYPE_NAMESPACE,
        "functor" => TOKEN_TYPE_FUNCTOR,
        "category" => TOKEN_TYPE_CATEGORY,
        "type" if is_known_category_name(text) => TOKEN_TYPE_CATEGORY,
        "type" => TOKEN_TYPE_TYPE,
        "type.declaration" => {
            token_modifiers_bitset |= TOKEN_MODIFIER_DECLARATION;
            TOKEN_TYPE_TYPE
        }
        "typeParameter" => TOKEN_TYPE_TYPE_PARAMETER,
        "parameter" => TOKEN_TYPE_PARAMETER,
        "parameter.declaration" => {
            token_modifiers_bitset |= TOKEN_MODIFIER_DECLARATION;
            TOKEN_TYPE_PARAMETER
        }
        "function" => TOKEN_TYPE_FUNCTION,
        "function.declaration" => {
            token_modifiers_bitset |= TOKEN_MODIFIER_DECLARATION;
            TOKEN_TYPE_FUNCTION
        }
        "variable" => TOKEN_TYPE_VARIABLE,
        "variable.declaration" => {
            token_modifiers_bitset |= TOKEN_MODIFIER_DECLARATION;
            TOKEN_TYPE_VARIABLE
        }
        "property" => TOKEN_TYPE_PROPERTY,
        _ => return None,
    };

    if is_default_library(token_type, text) {
        token_modifiers_bitset |= TOKEN_MODIFIER_DEFAULT_LIBRARY;
    }

    Some(TokenSpec {
        token_type,
        token_modifiers_bitset,
    })
}

/// Checks whether a token name resolves to known built-ins for default-library styling.
fn is_default_library(token_type: u32, text: &str) -> bool {
    match token_type {
        TOKEN_TYPE_NAMESPACE => matches!(text, "std" | "raytracing"),
        TOKEN_TYPE_FUNCTOR => matches!(text, "Hom" | "Func"),
        TOKEN_TYPE_TYPE => is_known_type_name(text),
        TOKEN_TYPE_CATEGORY => is_known_category_name(text),
        TOKEN_TYPE_FUNCTION => is_default_library_function(text),
        _ => false,
    }
}

fn is_default_library_function(name: &str) -> bool {
    lane::known_primitive(name).is_some()
        || lane::known_builtin_object(name)
            .is_some_and(|detail| matches!(detail.kind, lane::KnownBuiltinObjectKind::Function))
}

/// Checks if `name` belongs to the built-in category surface.
fn is_known_category_name(name: &str) -> bool {
    lane::known_category_names()
        .into_iter()
        .any(|text| text == name)
        || lane::known_builtin_object(name)
            .is_some_and(|detail| matches!(detail.kind, lane::KnownBuiltinObjectKind::Category))
        || name == lane::CATEGORY_METATYPE_NAME
}

/// Checks if `name` is a known built-in or intrinsic type name.
fn is_known_type_name(name: &str) -> bool {
    lane::known_type_names()
        .into_iter()
        .any(|text| text == name)
        || lane::known_builtin_object(name)
            .is_some_and(|detail| matches!(detail.kind, lane::KnownBuiltinObjectKind::Type))
        || matches!(
            name,
            lane::CATEGORY_METATYPE_NAME | lane::TYPE_METATYPE_NAME
        )
}

/// Splits multi-line captures into per-line token segments for stable LSP deltas.
fn push_token_segments(
    tokens: &mut Vec<AbsoluteToken>,
    source: &str,
    line_start_bytes: &[usize],
    capture: QueryCapture<'_>,
    spec: TokenSpec,
) {
    let start_byte = capture.node.start_byte();
    let end_byte = capture.node.end_byte();
    let start_row = capture.node.start_position().row;
    let end_row = capture.node.end_position().row;

    for row in start_row..=end_row {
        let line_start = line_start_bytes[row];
        let next_line_start = line_start_bytes
            .get(row + 1)
            .copied()
            .unwrap_or(source.len());
        let line_end = source[line_start..next_line_start]
            .find('\n')
            .map(|offset| line_start + offset)
            .unwrap_or(next_line_start);
        let segment_start = start_byte.max(line_start);
        let segment_end = end_byte.min(line_end);
        if segment_start >= segment_end {
            continue;
        }

        let start = utf16_len(&source[line_start..segment_start]) as u32;
        let length = utf16_len(&source[segment_start..segment_end]) as u32;
        if length == 0 {
            continue;
        }
        tokens.push(AbsoluteToken {
            line: row as u32,
            start,
            length,
            token_type: spec.token_type,
            token_modifiers_bitset: spec.token_modifiers_bitset,
        });
    }
}

/// Counts UTF-16 code units for LSP-safe column/length calculations.
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
#[path = "../tests/unit/semantic_tokens.rs"]
mod tests;
