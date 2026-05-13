use tower_lsp::lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};

use crate::position;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CallContext {
    pub(crate) name: String,
    pub(crate) active_parameter: u32,
}

/// Builds signature help response for a function-like call context.
pub(crate) fn signature_help_for_context(context: &CallContext) -> Option<SignatureHelp> {
    let signature = signature_for_name(&context.name)?;
    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter: Some(context.active_parameter),
    })
}

/// Infers call context (name + active parameter index) from cursor position.
pub(crate) fn call_context_at_position(text: &str, position: Position) -> Option<CallContext> {
    let offset = position::byte_offset_for_position(text, position)?;
    let prefix = &text[..offset];
    let open = innermost_open_paren(prefix)?;
    let name = call_name_before(prefix, open)?;
    Some(CallContext {
        name,
        active_parameter: active_parameter_index(&prefix[open + 1..]),
    })
}

/// Checks if a character can appear inside the call name before a signature-help trigger.
fn is_call_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Finds the matching open parenthesis for the cursor prefix, respecting nesting.
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

/// Extracts the function name immediately before a given open parenthesis.
fn call_name_before(prefix: &str, open_paren: usize) -> Option<String> {
    let before = prefix[..open_paren].trim_end();
    let end = before.len();
    let start = before[..end]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_call_name_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| before[start..end].to_string())
}

/// Computes which argument index is currently active for signature-help.
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

/// Builds signature information for a known primitive or function name.
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

/// Constructs a signature from a raw function type string.
fn signature_from_function_type(name: &str, ty: &str) -> Option<SignatureInformation> {
    let (domain, codomain) = hom_domain_and_codomain(ty)?;
    let params = parameter_labels_from_domain(&domain);
    Some(signature_information(
        format!("{}({}) -> {}", name, params.join(", "), codomain),
        params,
        Some(ty.to_string()),
    ))
}

/// Splits `Hom`/`Func` type strings into domain and codomain parts.
fn hom_domain_and_codomain(ty: &str) -> Option<(String, String)> {
    let ty = ty.trim();
    let inner = ty
        .strip_prefix("Hom(")
        .or_else(|| ty.strip_prefix("Func("))?
        .strip_suffix(')')?;
    split_top_level_comma(inner)
}

/// Splits source at the top-level comma, accounting for nested groups.
fn split_top_level_comma(source: &str) -> Option<(String, String)> {
    let index = find_top_level_delimiter(source, ',')?;
    if index == 0 || index + 1 >= source.len() {
        return None;
    }
    let left = source[..index].trim().to_string();
    let right = source[index + 1..].trim().to_string();
    Some((left, right))
}

/// Produces parameter labels from a function domain expression.
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

/// Splits a product type expression into top-level factors.
fn split_top_level_product(source: &str) -> Vec<String> {
    let mut depth = 0u32;
    let mut parts = Vec::new();
    let mut start = 0;
    for (byte, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            'x' | '×' if depth == 0 && product_separator_at(source, byte, ch) => {
                parts.push(source[start..byte].trim().to_string());
                start = byte + ch.len_utf8();
            }
            _ => {}
        }
    }
    if start > 0 {
        parts.push(source[start..].trim().to_string());
    }
    parts.retain(|part| !part.is_empty());
    parts
}

/// Returns true when a character can be treated as top-level product separator.
fn product_separator_at(source: &str, byte: usize, ch: char) -> bool {
    if ch == '×' {
        return true;
    }
    let before = source[..byte].chars().next_back();
    let after = source[byte + ch.len_utf8()..].chars().next();
    before.is_some_and(char::is_whitespace) && after.is_some_and(char::is_whitespace)
}

fn find_top_level_delimiter(source: &str, delimiter: char) -> Option<usize> {
    let mut depth = 0u32;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ch if ch == delimiter && depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

/// Builds a typed signature object for function calls.
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
