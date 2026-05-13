use super::*;

/// Decodes relative semantic token output into absolute positions for assertions.
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

/// Legend contains all semantic types used for Lane custom token classes.
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

/// Verifies directive/functor/token emission for preview-like snippets.
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

/// Distinguishes category names from type names using expected token kinds.
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

/// Ensures provided arrow syntax is consistently rendered as a functor token.
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

/// Checks that provided names are classified as variables and closure args as parameters.
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

/// Guards against `x` in type signatures being misclassified as identifiers.
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

/// Confirms UTF-16 positions are preserved when tokens contain non-ASCII characters.
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
