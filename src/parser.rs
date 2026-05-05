use super::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Ident(String),
    Number(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    At,
    Arrow,
}

pub(super) struct Parser<'a> {
    source: &'a str,
}

impl<'a> Parser<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub(super) fn parse_program(&self) -> Result<Program, Error> {
        let mut custom_types: HashMap<String, AlgebraicCategory> = HashMap::new();
        let mut inputs = Vec::new();
        let mut funcs = Vec::new();
        let mut value_bindings = Vec::new();
        let mut bindings = Vec::new();
        let mut inferred_bindings = Vec::new();
        let mut output = None;
        let mut ambient_dimension = ShapeDimension::D3;
        let mut directives_open = true;

        for (line_index, raw_line) in self.source.lines().enumerate() {
            let line_number = line_index + 1;
            let line = strip_line_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                if !directives_open {
                    return Err(Error::new("directives must appear before declarations")
                        .with_line(line_number));
                }
                parse_directive(line, &mut ambient_dimension)
                    .map_err(|err| err.with_line(line_number))?;
                continue;
            }
            directives_open = false;

            match self
                .parse_decl(line, line_number, ambient_dimension)
                .or_else(|_| {
                    self.parse_decl_with_custom_types(
                        line,
                        line_number,
                        &custom_types,
                        ambient_dimension,
                    )
                })
                .map_err(|err| err.with_line(line_number))?
            {
                Decl::ProvidedType(provided_type) => {
                    if is_known_type_name(&provided_type.name)
                        || is_known_category_name(&provided_type.name)
                    {
                        return Err(Error::new(format!(
                            "'{}' cannot be used as a provided type name",
                            provided_type.name
                        ))
                        .with_line(line_number));
                    }
                    if custom_types
                        .insert(provided_type.name.clone(), provided_type.category)
                        .is_some()
                    {
                        return Err(Error::new(format!(
                            "duplicate provided type '{}'",
                            provided_type.name
                        ))
                        .with_line(line_number));
                    }
                }
                Decl::Input(input) => inputs.push(input),
                Decl::Func(func) => funcs.push(func),
                Decl::ValueBinding(binding) => value_bindings.push(binding),
                Decl::Binding(binding) => bindings.push(binding),
                Decl::InferredBinding(binding) => inferred_bindings.push(binding),
                Decl::Output(out) => {
                    if output.is_some() {
                        return Err(Error::new("multiple out declarations are not supported"));
                    }
                    output = Some(out);
                }
            }
        }

        let output = output.ok_or_else(|| Error::new("missing generate declaration"))?;

        Ok(Program {
            ambient_dimension,
            inputs,
            funcs,
            value_bindings,
            bindings,
            inferred_bindings,
            output,
        })
    }

    fn parse_decl(
        &self,
        line: &str,
        line_number: usize,
        ambient_dimension: ShapeDimension,
    ) -> Result<Decl, Error> {
        self.parse_decl_with_custom_types(line, line_number, &HashMap::new(), ambient_dimension)
    }

    fn parse_decl_with_custom_types(
        &self,
        line: &str,
        line_number: usize,
        custom_types: &HashMap<String, AlgebraicCategory>,
        ambient_dimension: ShapeDimension,
    ) -> Result<Decl, Error> {
        if let Some(rest) = line.strip_prefix("provided ") {
            let (ty, name) = split_type_name(rest.trim())?;
            if let Some(category) = category_by_name(ty) {
                return Ok(Decl::ProvidedType(ProvidedTypeDecl {
                    name: name.to_string(),
                    category,
                }));
            }
            return Ok(Decl::Input(InputDecl {
                name: name.to_string(),
                ty: parse_type_with_custom_types_for_ambient(ty, custom_types, ambient_dimension)?,
                line: line_number,
            }));
        }

        if let Some(rest) = line
            .strip_prefix("generate ")
            .or_else(|| line.strip_prefix("gen "))
        {
            let expr_source = rest.trim();
            if let Some((left, _)) = expr_source.split_once('=') {
                if parse_type(left.trim()).is_ok() {
                    return Err(Error::new(
                        "use 'generate value' instead of 'generate type = value'",
                    ));
                }
            }
            let expr = ExprParser::new(expr_source).parse()?;
            return Ok(Decl::Output(OutputDecl {
                expr,
                line: line_number,
            }));
        }

        let generated = line.starts_with("construct ") || line.starts_with("const ");
        let line = line
            .strip_prefix("construct ")
            .or_else(|| line.strip_prefix("const "))
            .unwrap_or(line);
        let (left, expr_source) = split_once_required(line, '=')?;
        if left.contains(':') {
            return Err(Error::new(
                "use 'type name = value' for declarations instead of 'name : type = value'",
            ));
        }
        let left = left.trim();
        if !left.contains(char::is_whitespace) {
            let expr = ExprParser::new(expr_source.trim()).parse()?;
            return Ok(Decl::InferredBinding(InferredBindingDecl {
                name: left.to_string(),
                expr,
                generated,
                line: line_number,
            }));
        }
        let (ty_source, name) = split_type_name(left)?;
        let ty = parse_type_with_custom_types_for_ambient(
            ty_source.trim(),
            custom_types,
            ambient_dimension,
        )?;
        let expr = ExprParser::new(expr_source.trim()).parse()?;
        if matches!(ty, Type::Func(_, _)) {
            if generated {
                return Err(Error::new(
                    "'construct' currently only supports Object bindings",
                ));
            }
            return Ok(Decl::Func(FuncDecl {
                name: name.to_string(),
                ty,
                expr,
                line: line_number,
            }));
        }
        if !matches!(ty, Type::Object | Type::Object2D) {
            if generated {
                return Err(Error::new(
                    "'construct' currently only supports Object bindings",
                ));
            }
            return Ok(Decl::ValueBinding(ValueBindingDecl {
                name: name.to_string(),
                ty,
                expr,
                line: line_number,
            }));
        }
        Ok(Decl::Binding(BindingDecl {
            name: name.to_string(),
            ty,
            expr,
            generated,
            line: line_number,
        }))
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(before, _)| before)
}

fn parse_directive(line: &str, ambient_dimension: &mut ShapeDimension) -> Result<(), Error> {
    match line {
        "#2D" => {
            *ambient_dimension = ShapeDimension::D2;
            Ok(())
        }
        _ => Err(Error::new(format!("unsupported directive '{}'", line))),
    }
}

struct ExprParser {
    tokens: Vec<Token>,
    index: usize,
}

impl ExprParser {
    pub(super) fn new(source: &str) -> Self {
        Self {
            tokens: tokenize(source),
            index: 0,
        }
    }

    fn parse(mut self) -> Result<Expr, Error> {
        let expr = self.parse_add_sub()?;
        if self.peek().is_some() {
            return Err(Error::new(format!(
                "unexpected trailing token {} in expression",
                self.describe_current_token()
            )));
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.index += 1;
            let rhs = self.parse_mul_div()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_compose()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.index += 1;
            let rhs = self.parse_compose()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_compose(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_postfix()?;
        while matches!(self.peek(), Some(Token::At)) {
            self.index += 1;
            let rhs = self.parse_postfix()?;
            expr = Expr::Binary {
                op: BinOp::Compose,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Token::LParen) => {
                    let args = self.parse_positional_args()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                }
                Some(Token::LBracket) => {
                    self.index += 1;
                    let index = self.parse_add_sub()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::Index {
                        array: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.index += 1;
            let expr = self.parse_unary()?;
            return Ok(Expr::Binary {
                op: BinOp::Sub,
                left: Box::new(Expr::Number(0.0)),
                right: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value.parse::<f64>().unwrap())),
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    let args = self.parse_mixed_args()?;
                    return Ok(Expr::Constructor { name, args });
                }
                Ok(Expr::Ident(name))
            }
            Some(Token::LParen) => self.parse_paren_or_tuple(),
            Some(Token::LBracket) => self.parse_array_literal(),
            _ => Err(Error::new(format!(
                "unexpected token {} in expression",
                self.describe_previous_token()
            ))),
        }
    }

    fn parse_paren_or_tuple(&mut self) -> Result<Expr, Error> {
        let first = self.parse_add_sub()?;
        if !matches!(self.peek(), Some(Token::Comma)) {
            self.expect(Token::RParen)?;
            return Ok(first);
        }

        let mut items = vec![first];
        while matches!(self.peek(), Some(Token::Comma)) {
            self.index += 1;
            items.push(self.parse_add_sub()?);
        }
        self.expect(Token::RParen)?;
        Ok(Expr::Tuple(items))
    }

    fn parse_array_literal(&mut self) -> Result<Expr, Error> {
        if matches!(self.peek(), Some(Token::RBracket)) {
            self.index += 1;
            return Ok(Expr::Array(Vec::new()));
        }

        let mut items = Vec::new();
        loop {
            items.push(self.parse_add_sub()?);
            match self.peek() {
                Some(Token::Comma) => self.index += 1,
                Some(Token::RBracket) => {
                    self.index += 1;
                    break;
                }
                _ => return Err(Error::new("expected ',' or ']' in array literal")),
            }
        }
        Ok(Expr::Array(items))
    }

    fn parse_positional_args(&mut self) -> Result<Vec<Expr>, Error> {
        self.expect(Token::LParen)?;
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(Vec::new());
        }
        let mut args = Vec::new();
        loop {
            args.push(self.parse_add_sub()?);
            match self.peek() {
                Some(Token::Comma) => self.index += 1,
                Some(Token::RParen) => {
                    self.index += 1;
                    break;
                }
                _ => return Err(Error::new("expected ',' or ')' in argument list")),
            }
        }
        Ok(args)
    }

    fn parse_mixed_args(&mut self) -> Result<ConstructorArgs, Error> {
        self.expect(Token::LParen)?;
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(ConstructorArgs::Positional(Vec::new()));
        }

        if !matches!(
            (self.peek(), self.peek_n(1)),
            (Some(Token::Ident(_)), Some(Token::Equal))
        ) {
            self.index -= 1;
            return Ok(ConstructorArgs::Positional(self.parse_positional_args()?));
        }

        let mut named = Vec::new();
        loop {
            let field_name = match (self.peek(), self.peek_n(1)) {
                (Some(Token::Ident(name)), Some(Token::Equal)) => name.clone(),
                _ => return Err(Error::new("expected named constructor arguments")),
            };
            self.index += 2;
            let expr = self.parse_add_sub()?;
            named.push((field_name, expr));
            match self.peek() {
                Some(Token::Comma) => self.index += 1,
                Some(Token::RParen) => {
                    self.index += 1;
                    break;
                }
                _ => return Err(Error::new("expected ',' or ')' in named argument list")),
            }
        }
        Ok(ConstructorArgs::Named(named))
    }

    fn expect(&mut self, expected: Token) -> Result<(), Error> {
        let token = self
            .next()
            .ok_or_else(|| Error::new("unexpected end of input"))?;
        if token == expected {
            return Ok(());
        }
        Err(Error::new(format!(
            "expected {}, got {}",
            Self::describe_token(&expected),
            Self::describe_token(&token)
        )))
    }

    fn describe_current_token(&self) -> String {
        self.peek()
            .map(Self::describe_token)
            .unwrap_or_else(|| "<end of input>".to_string())
    }

    fn describe_previous_token(&self) -> String {
        self.index
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(Self::describe_token)
            .unwrap_or_else(|| "<start of input>".to_string())
    }

    fn describe_token(token: &Token) -> String {
        match token {
            Token::Ident(name) => format!("identifier '{}'", name),
            Token::Number(value) => format!("number '{}'", value),
            Token::LParen => "'('".to_string(),
            Token::RParen => "')'".to_string(),
            Token::LBracket => "'['".to_string(),
            Token::RBracket => "']'".to_string(),
            Token::Colon => "':'".to_string(),
            Token::Comma => "','".to_string(),
            Token::Equal => "'='".to_string(),
            Token::Plus => "'+'".to_string(),
            Token::Minus => "'-'".to_string(),
            Token::Star => "'*'".to_string(),
            Token::Slash => "'/'".to_string(),
            Token::At => "'@'".to_string(),
            Token::Arrow => "'->'".to_string(),
        }
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn peek_n(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.index + offset)
    }
}

fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch == '-' && chars.get(index + 1) == Some(&'>') {
            tokens.push(Token::Arrow);
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
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '@' => Token::At,
            _ => panic!("unsupported token: {ch}"),
        };
        tokens.push(token);
        index += 1;
    }

    tokens
}

fn parse_type(source: &str) -> Result<Type, Error> {
    parse_type_with_custom_types(source, &HashMap::new())
}

fn parse_type_with_custom_types(
    source: &str,
    custom_types: &HashMap<String, AlgebraicCategory>,
) -> Result<Type, Error> {
    let source = source.trim();
    if source.starts_with("func(") && source.ends_with(')') {
        let inner = &source[5..source.len() - 1];
        let (input, output) = split_arrow_legacy(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types(input, custom_types)?,
            parse_type_with_custom_types(output, custom_types)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "Func") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types(input, custom_types)?,
            parse_type_with_custom_types(output, custom_types)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "Hom") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types(input, custom_types)?,
            parse_type_with_custom_types(output, custom_types)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "End") {
        let ty = parse_type_with_custom_types(inner, custom_types)?;
        return Ok(Type::func(ty.clone(), ty));
    }
    if let Some(inner) = strip_type_head(source, "Array") {
        return Ok(Type::Array(Box::new(parse_type_with_custom_types(
            inner,
            custom_types,
        )?)));
    }
    if let Some(parts) = split_top_level_product(source) {
        let mut parsed = Vec::new();
        for part in parts {
            parsed.push(parse_type_with_custom_types(part, custom_types)?);
        }
        return Ok(Type::Product(parsed));
    }
    if category_by_name(source).is_some() {
        return Err(Error::new(format!(
            "category '{}' cannot be used as a type",
            source
        )));
    }
    if let Some(category) = custom_types.get(source) {
        return Ok(custom_type(source, *category));
    }
    match parse_builtin_type_name(source) {
        Some(ty) => Ok(ty),
        None => Err(Error::new(format!("unsupported type '{}'", source))),
    }
}

fn parse_type_with_custom_types_for_ambient(
    source: &str,
    custom_types: &HashMap<String, AlgebraicCategory>,
    ambient_dimension: ShapeDimension,
) -> Result<Type, Error> {
    let source = source.trim();
    if source == "Object" && ambient_dimension == ShapeDimension::D2 {
        return Ok(Type::Object2D);
    }
    if source.starts_with("func(") && source.ends_with(')') {
        let inner = &source[5..source.len() - 1];
        let (input, output) = split_arrow_legacy(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types_for_ambient(input, custom_types, ambient_dimension)?,
            parse_type_with_custom_types_for_ambient(output, custom_types, ambient_dimension)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "Func") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types_for_ambient(input, custom_types, ambient_dimension)?,
            parse_type_with_custom_types_for_ambient(output, custom_types, ambient_dimension)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "Hom") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(
            parse_type_with_custom_types_for_ambient(input, custom_types, ambient_dimension)?,
            parse_type_with_custom_types_for_ambient(output, custom_types, ambient_dimension)?,
        ));
    }
    if let Some(inner) = strip_type_head(source, "End") {
        let ty = parse_type_with_custom_types_for_ambient(inner, custom_types, ambient_dimension)?;
        return Ok(Type::func(ty.clone(), ty));
    }
    if let Some(inner) = strip_type_head(source, "Array") {
        return Ok(Type::Array(Box::new(
            parse_type_with_custom_types_for_ambient(inner, custom_types, ambient_dimension)?,
        )));
    }
    if let Some(parts) = split_top_level_product(source) {
        let mut parsed = Vec::new();
        for part in parts {
            parsed.push(parse_type_with_custom_types_for_ambient(
                part,
                custom_types,
                ambient_dimension,
            )?);
        }
        return Ok(Type::Product(parsed));
    }
    parse_type_with_custom_types(source, custom_types)
}

fn split_arrow_legacy(source: &str) -> Result<(&str, &str), Error> {
    source
        .split_once("->")
        .map(|(left, right)| (left.trim(), right.trim()))
        .ok_or_else(|| Error::new("expected '->' in function type"))
}

fn strip_type_head<'a>(source: &'a str, head: &str) -> Option<&'a str> {
    source
        .strip_prefix(head)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
}

fn split_top_level_comma(source: &str) -> Result<(&str, &str), Error> {
    let mut depth = 0;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                return Ok((source[..index].trim(), source[index + 1..].trim()));
            }
            _ => {}
        }
    }
    Err(Error::new("expected ',' in function type"))
}

fn split_top_level_product(source: &str) -> Option<Vec<&str>> {
    let mut depth = 0;
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = source[index..].chars().next().unwrap();
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '×' if depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + ch.len_utf8();
            }
            'x' if depth == 0 => {
                let prev_space = index > 0 && bytes[index - 1].is_ascii_whitespace();
                let next_index = index + 1;
                let next_space =
                    next_index < bytes.len() && bytes[next_index].is_ascii_whitespace();
                if prev_space && next_space {
                    parts.push(source[start..index - 1].trim());
                    start = next_index + 1;
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(source[start..].trim());
    Some(parts)
}

fn split_type_name(source: &str) -> Result<(&str, &str), Error> {
    let index = source
        .rfind(' ')
        .ok_or_else(|| Error::new("expected '<type> <name>'"))?;
    Ok((&source[..index], source[index + 1..].trim()))
}

fn split_once_required(source: &str, ch: char) -> Result<(&str, &str), Error> {
    source
        .split_once(ch)
        .map(|(left, right)| (left, right))
        .ok_or_else(|| Error::new(format!("expected '{}'", ch)))
}
