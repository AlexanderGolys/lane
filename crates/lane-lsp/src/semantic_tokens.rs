use std::collections::HashSet;

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};
use tree_sitter::{Parser, Query, QueryCapture, QueryCursor, StreamingIterator};
use tree_sitter_language::LanguageFn;

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

unsafe extern "C" {
    fn tree_sitter_lane() -> *const ();
}

const LANGUAGE_LANE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_lane) };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenSpec {
    token_type: u32,
    token_modifiers_bitset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

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

pub fn tokens(source: &str) -> SemanticTokens {
    let mut parser = Parser::new();
    parser
        .set_language(&LANGUAGE_LANE.into())
        .expect("lane tree-sitter parser should load");
    let Some(tree) = parser.parse(source, None) else {
        return SemanticTokens {
            result_id: None,
            data: Vec::new(),
        };
    };

    let query =
        Query::new(&LANGUAGE_LANE.into(), QUERY).expect("lane semantic token query should compile");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut absolute_tokens = Vec::new();
    let mut seen = HashSet::new();
    let line_start_bytes = line_start_bytes(source);

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
        "type" if is_category_token_text(text) => TOKEN_TYPE_CATEGORY,
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

fn is_default_library(token_type: u32, text: &str) -> bool {
    match token_type {
        TOKEN_TYPE_NAMESPACE => matches!(text, "std" | "raytracing"),
        TOKEN_TYPE_FUNCTOR => matches!(text, "Hom" | "Func"),
        TOKEN_TYPE_TYPE => {
            lane::known_type_names()
                .into_iter()
                .any(|name| name == text)
                || lane::known_builtin_object(text)
                    .is_some_and(|detail| matches!(detail.kind, lane::KnownBuiltinObjectKind::Type))
                || matches!(
                    text,
                    lane::CATEGORY_METATYPE_NAME | lane::TYPE_METATYPE_NAME
                )
        }
        TOKEN_TYPE_CATEGORY => {
            lane::known_category_names()
                .into_iter()
                .any(|name| name == text)
                || lane::known_builtin_object(text).is_some_and(|detail| {
                    matches!(detail.kind, lane::KnownBuiltinObjectKind::Category)
                })
                || text == lane::CATEGORY_METATYPE_NAME
        }
        TOKEN_TYPE_FUNCTION => {
            lane::known_primitive(text).is_some()
                || lane::known_builtin_object(text).is_some_and(|detail| {
                    matches!(detail.kind, lane::KnownBuiltinObjectKind::Function)
                })
        }
        _ => false,
    }
}

fn is_category_token_text(text: &str) -> bool {
    lane::known_category_names()
        .into_iter()
        .any(|name| name == text)
        || lane::known_builtin_object(text)
            .is_some_and(|detail| matches!(detail.kind, lane::KnownBuiltinObjectKind::Category))
        || text == lane::CATEGORY_METATYPE_NAME
}

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

fn line_start_bytes(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + ch.len_utf8());
        }
    }
    starts
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded_tokens(source: &str) -> Vec<(u32, u32, u32, u32, u32)> {
        let mut line = 0;
        let mut start = 0;
        tokens(source)
            .data
            .into_iter()
            .map(|token| {
                line += token.delta_line;
                start = if token.delta_line == 0 {
                    start + token.delta_start
                } else {
                    token.delta_start
                };
                (
                    line,
                    start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }

    #[test]
    fn legend_includes_lane_custom_types() {
        let legend = legend();

        assert_eq!(
            legend.token_types[TOKEN_TYPE_DIRECTIVE as usize],
            SemanticTokenType::new("directive")
        );
        assert_eq!(
            legend.token_types[TOKEN_TYPE_FUNCTOR as usize],
            SemanticTokenType::new("functor")
        );
        assert_eq!(
            legend.token_types[TOKEN_TYPE_CATEGORY as usize],
            SemanticTokenType::new("category")
        );
    }

    #[test]
    fn emits_lane_semantic_tokens_for_directives_and_functors() {
        let source =
            "#import std\nprovided Hom({n}, R) fold = x |-> x\nconst R shader = \"return ${x};\"\n";
        let tokens = decoded_tokens(source);

        assert!(tokens.iter().any(|token| token.3 == TOKEN_TYPE_DIRECTIVE));
        assert!(tokens.iter().any(|token| {
            token.3 == TOKEN_TYPE_NAMESPACE && token.4 & TOKEN_MODIFIER_DEFAULT_LIBRARY != 0
        }));
        assert!(tokens.iter().any(|token| {
            token.3 == TOKEN_TYPE_FUNCTOR && token.4 & TOKEN_MODIFIER_DEFAULT_LIBRARY != 0
        }));
        assert!(tokens
            .iter()
            .any(|token| token.3 == TOKEN_TYPE_TYPE_PARAMETER));
        assert!(tokens.iter().any(|token| token.3 == TOKEN_TYPE_STRING));
    }

    #[test]
    fn emits_distinct_tokens_for_categories_and_types() {
        let source = "provided Grp G\nprovided G g\nconst Type t = Type\n";
        let tokens = decoded_tokens(source);

        let lines = source.lines().collect::<Vec<_>>();
        let grp_start = lines[0].find("Grp").unwrap() as u32;
        let custom_type_start = lines[1].find("G").unwrap() as u32;
        let type_start = lines[2].find("Type").unwrap() as u32;

        assert!(tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == grp_start
                && token.2 == "Grp".len() as u32
                && token.3 == TOKEN_TYPE_CATEGORY
                && token.4 & TOKEN_MODIFIER_DEFAULT_LIBRARY != 0
        }));
        assert!(tokens.iter().any(|token| {
            token.0 == 1
                && token.1 == custom_type_start
                && token.2 == "G".len() as u32
                && token.3 == TOKEN_TYPE_TYPE
        }));
        assert!(tokens.iter().any(|token| {
            token.0 == 2
                && token.1 == type_start
                && token.2 == "Type".len() as u32
                && token.3 == TOKEN_TYPE_TYPE
        }));
    }

    #[test]
    fn classifies_provided_function_arrow_as_functor() {
        let source = "provided f: X -> Y\nconst Hom(R, R) g = x |-> x\n";
        let tokens = decoded_tokens(source);

        let arrow_start = source.lines().next().unwrap().find("->").unwrap() as u32;
        assert!(tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == arrow_start
                && token.2 == "->".len() as u32
                && token.3 == TOKEN_TYPE_FUNCTOR
        }));
        assert!(!tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == arrow_start
                && token.2 == "->".len() as u32
                && token.3 == TOKEN_TYPE_OPERATOR
        }));
    }

    #[test]
    fn classifies_provided_names_as_variables_not_parameters() {
        let source = "provided R time\nconst Hom(R, R) g = x |-> time + x\n";
        let tokens = decoded_tokens(source);

        let provided_start = source.lines().next().unwrap().find("time").unwrap() as u32;
        let closure_line = source.lines().nth(1).unwrap();
        let closure_parameter_start = closure_line.find("x |->").unwrap() as u32;

        assert!(tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == provided_start
                && token.2 == "time".len() as u32
                && token.3 == TOKEN_TYPE_VARIABLE
                && token.4 & TOKEN_MODIFIER_DECLARATION != 0
        }));
        assert!(!tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == provided_start
                && token.2 == "time".len() as u32
                && token.3 == TOKEN_TYPE_PARAMETER
        }));
        assert!(tokens.iter().any(|token| {
            token.0 == 1
                && token.1 == closure_parameter_start
                && token.2 == "x".len() as u32
                && token.3 == TOKEN_TYPE_PARAMETER
                && token.4 & TOKEN_MODIFIER_DECLARATION != 0
        }));
    }

    #[test]
    fn classifies_function_product_x_as_operator() {
        let source = "Hom(R x R, R x R) h = f x g\n";
        let tokens = decoded_tokens(source);
        let product_start = source.lines().next().unwrap().rfind("x").unwrap() as u32;

        assert!(tokens.iter().any(|token| {
            token.0 == 0
                && token.1 == product_start
                && token.2 == "x".len() as u32
                && token.3 == TOKEN_TYPE_OPERATOR
        }));
    }

    #[test]
    fn preserves_utf16_columns_for_non_ascii_tokens() {
        let source = "R2 uv = p × q\n";
        let tokens = decoded_tokens(source);
        let operator_start = "R2 uv = p ".encode_utf16().count() as u32;
        let operator = tokens
            .into_iter()
            .find(|token| {
                token.1 == operator_start
                    && token.3 == TOKEN_TYPE_OPERATOR
                    && token.2 == "×".encode_utf16().count() as u32
            })
            .expect("operator token should exist");

        assert_eq!(operator.0, 0);
        assert_eq!(operator.1, operator_start);
        assert_eq!(operator.2, "×".encode_utf16().count() as u32);
    }
}
