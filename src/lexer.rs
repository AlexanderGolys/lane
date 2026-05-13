//! Turns Lane expression source fragments into parser tokens.
//! Lexing is separated from parsing so character-level scanning, string/number handling, and symbolic-token recognition do not mix with AST construction.
//! It runs inside the parsing stage whenever expression syntax needs to be tokenized before the expression parser consumes it.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Token {
    Ident(String),
    Number(String),
    StringLiteral(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Tilde,
    Amp,
    Dot,
    At,
    Arrow,
}

/// Converts an expression source fragment into Lane tokens.
pub(super) fn tokenize(source: &str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch == '"' {
            index += 1;
            let start = index;
            while index < chars.len() && chars[index] != '"' {
                index += 1;
            }
            tokens.push(Token::StringLiteral(chars[start..index].iter().collect()));
            if index < chars.len() {
                index += 1;
            }
            continue;
        }
        if ch == '|' && chars.get(index + 1) == Some(&'-') && chars.get(index + 2) == Some(&'>') {
            tokens.push(Token::Arrow);
            index += 3;
            continue;
        }
        if ch == '=' && chars.get(index + 1) == Some(&'=') {
            tokens.push(Token::EqualEqual);
            index += 2;
            continue;
        }
        if ch == '!' && chars.get(index + 1) == Some(&'=') {
            tokens.push(Token::BangEqual);
            index += 2;
            continue;
        }
        if ch == '<' && chars.get(index + 1) == Some(&'=') {
            tokens.push(Token::LessEqual);
            index += 2;
            continue;
        }
        if ch == '>' && chars.get(index + 1) == Some(&'=') {
            tokens.push(Token::GreaterEqual);
            index += 2;
            continue;
        }
        if ch.is_ascii_digit()
            || (ch == '.' && chars.get(index + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
                index += 1;
            }
            if index < chars.len() && matches!(chars[index], 'e' | 'E') {
                let exponent_start = index;
                index += 1;
                if index < chars.len() && matches!(chars[index], '+' | '-') {
                    index += 1;
                }
                let digit_start = index;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
                if digit_start == index {
                    index = exponent_start;
                }
            }
            tokens.push(Token::Number(chars[start..index].iter().collect()));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            while index < chars.len() && chars[index] == '{' {
                let brace_start = index;
                index += 1;
                let inner_start = index;
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
                if inner_start == index || index >= chars.len() || chars[index] != '}' {
                    index = brace_start;
                    break;
                }
                index += 1;
            }
            tokens.push(Token::Ident(chars[start..index].iter().collect()));
            continue;
        }
        let token = match ch {
            '(' => Token::LParen,
            ')' => Token::RParen,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ':' => Token::Colon,
            ',' => Token::Comma,
            '=' => Token::Equal,
            '<' => Token::Less,
            '>' => Token::Greater,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '~' => Token::Tilde,
            '&' => Token::Amp,
            '.' => Token::Dot,
            '@' => Token::At,
            _ => {
                return Err(Error::new(format!(
                    "unsupported token '{ch}' in expression"
                )))
            }
        };
        tokens.push(token);
        index += 1;
    }

    Ok(tokens)
}
