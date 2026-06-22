//! 純粋式の DSL パーサ（ADR-0004）。
//!
//! `.hybs` の `<template>` 内の束縛・`:if` 条件・`:each` の key 式はすべて **純粋式**
//! （[`Expr`]）である。このモジュールはその式テキストを字句解析し、[`Expr`] AST に
//! 落とす。閉じた値モデル（number / string / bool / list / record）の上だけで閉じ、
//! 関数呼び出しや副作用は持たない（ADR-0004：binding は純粋式・script クロージャに
//! しない）。
//!
//! 文法（優先順位は低い順）：
//!
//! ```text
//! or      = and  ( "||" and )*
//! and     = eq   ( "&&" eq )*
//! eq      = cmp  ( ("==" | "!=") cmp )*
//! cmp     = add  ( ("<" | "<=" | ">" | ">=") add )*
//! add     = mul  ( ("+" | "-") mul )*
//! mul     = unary ( ("*" | "/") unary )*
//! unary   = ("!" | "-") unary | postfix
//! postfix = primary ( "." ident | "[" expr "]" )*
//! primary = number | string | "true" | "false" | ident | "(" expr ")"
//! ```

use crate::expr::{BinOp, Expr};
use crate::value::Value;
use std::fmt;

/// 式テキストを [`Expr`] に解析する。
pub fn parse_expr(input: &str) -> Result<Expr, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_or()?;
    if parser.pos != parser.tokens.len() {
        return Err(ParseError::new(format!(
            "unexpected trailing input near token {}",
            parser.pos
        )));
    }
    Ok(expr)
}

/// 解析エラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        ParseError {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Str(String),
    Ident(String),
    True,
    False,
    Dot,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::EqEq);
                    i += 2;
                } else {
                    return Err(ParseError::new("expected `==` (single `=` is not valid)"));
                }
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::NotEq);
                    i += 2;
                } else {
                    tokens.push(Token::Bang);
                    i += 1;
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    tokens.push(Token::AndAnd);
                    i += 2;
                } else {
                    return Err(ParseError::new("expected `&&`"));
                }
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    tokens.push(Token::OrOr);
                    i += 2;
                } else {
                    return Err(ParseError::new("expected `||`"));
                }
            }
            '"' | '\'' => {
                let quote = c;
                let mut s = String::new();
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let ch = chars[i];
                    if ch == '\\' {
                        // 最小のエスケープ：\" \' \\ \n \t。
                        match chars.get(i + 1) {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('\\') => s.push('\\'),
                            Some('"') => s.push('"'),
                            Some('\'') => s.push('\''),
                            Some(other) => s.push(*other),
                            None => return Err(ParseError::new("unterminated escape")),
                        }
                        i += 2;
                    } else if ch == quote {
                        closed = true;
                        i += 1;
                        break;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                if !closed {
                    return Err(ParseError::new("unterminated string literal"));
                }
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    // `1.2.3` のような多重ドットは数値として弾く（後段で member と区別）。
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                match text.parse::<f64>() {
                    Ok(n) => tokens.push(Token::Number(n)),
                    Err(_) => return Err(ParseError::new(format!("invalid number `{}`", text))),
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                match text.as_str() {
                    "true" => tokens.push(Token::True),
                    "false" => tokens.push(Token::False),
                    _ => tokens.push(Token::Ident(text)),
                }
            }
            other => {
                return Err(ParseError::new(format!("unexpected character `{}`", other)));
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, expected: &Token) -> Result<(), ParseError> {
        match self.peek() {
            Some(t) if t == expected => {
                self.pos += 1;
                Ok(())
            }
            other => Err(ParseError::new(format!(
                "expected {:?}, found {:?}",
                expected, other
            ))),
        }
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), Some(Token::OrOr)) {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::binary(BinOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_eq()?;
        while matches!(self.peek(), Some(Token::AndAnd)) {
            self.advance();
            let rhs = self.parse_eq()?;
            lhs = Expr::binary(BinOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_eq(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cmp()?;
        loop {
            let op = match self.peek() {
                Some(Token::EqEq) => BinOp::Eq,
                Some(Token::NotEq) => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_cmp()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Some(Token::Lt) => BinOp::Lt,
                Some(Token::Le) => BinOp::Le,
                Some(Token::Gt) => BinOp::Gt,
                Some(Token::Ge) => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_add()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Bang) => {
                self.advance();
                let inner = self.parse_unary()?;
                Ok(Expr::Not(Box::new(inner)))
            }
            Some(Token::Minus) => {
                self.advance();
                let inner = self.parse_unary()?;
                // 単項マイナスは `0 - x` に展開する（閉じた値モデルの算術に乗せる）。
                Ok(Expr::binary(BinOp::Sub, Expr::lit(Value::number(0)), inner))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::Dot) => {
                    self.advance();
                    match self.advance() {
                        Some(Token::Ident(name)) => expr = expr.member(name),
                        other => {
                            return Err(ParseError::new(format!(
                                "expected field name after `.`, found {:?}",
                                other
                            )))
                        }
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_or()?;
                    self.eat(&Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::lit(Value::number(n))),
            Some(Token::Str(s)) => Ok(Expr::lit(Value::string(s))),
            Some(Token::True) => Ok(Expr::lit(Value::Bool(true))),
            Some(Token::False) => Ok(Expr::lit(Value::Bool(false))),
            Some(Token::Ident(name)) => Ok(Expr::var(name)),
            Some(Token::LParen) => {
                let inner = self.parse_or()?;
                self.eat(&Token::RParen)?;
                Ok(inner)
            }
            other => Err(ParseError::new(format!(
                "expected an expression, found {:?}",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{Binding, Scope};

    /// 定数畳み込みのない式は、空 scope で評価して値を確かめるのが手軽。
    fn eval_str(input: &str) -> Value {
        parse_expr(input).unwrap().eval(&Scope::new())
    }

    #[test]
    fn numbers_strings_bools() {
        assert_eq!(eval_str("42"), Value::number(42));
        assert_eq!(eval_str("3.5"), Value::number(3.5));
        assert_eq!(eval_str("\"hi\""), Value::string("hi"));
        assert_eq!(eval_str("'yo'"), Value::string("yo"));
        assert_eq!(eval_str("true"), Value::Bool(true));
        assert_eq!(eval_str("false"), Value::Bool(false));
    }

    #[test]
    fn arithmetic_precedence() {
        assert_eq!(eval_str("1 + 2 * 3"), Value::number(7));
        assert_eq!(eval_str("(1 + 2) * 3"), Value::number(9));
        assert_eq!(eval_str("10 - 2 - 3"), Value::number(5)); // 左結合
        assert_eq!(eval_str("-5 + 8"), Value::number(3)); // 単項マイナス
    }

    #[test]
    fn comparison_and_logic() {
        assert_eq!(eval_str("3 > 2"), Value::Bool(true));
        assert_eq!(eval_str("3 == 3"), Value::Bool(true));
        assert_eq!(eval_str("3 != 3"), Value::Bool(false));
        assert_eq!(eval_str("true && false"), Value::Bool(false));
        assert_eq!(eval_str("true || false"), Value::Bool(true));
        assert_eq!(eval_str("!false"), Value::Bool(true));
        // && は || より強く結合する。
        assert_eq!(eval_str("false && false || true"), Value::Bool(true));
    }

    #[test]
    fn string_concat() {
        assert_eq!(eval_str("\"a\" + \"b\""), Value::string("ab"));
        assert_eq!(eval_str("\"n=\" + 5"), Value::string("n=5"));
    }

    #[test]
    fn var_member_index() {
        let item = Value::record([("name".into(), Value::string("Sun"))]);
        let list = Value::list([Value::number(10), Value::number(20)]);
        let scope = Scope::new()
            .with("item", Binding::Const(item))
            .with("xs", Binding::Const(list));

        assert_eq!(
            parse_expr("item.name").unwrap().eval(&scope),
            Value::string("Sun")
        );
        assert_eq!(parse_expr("xs[1]").unwrap().eval(&scope), Value::number(20));
    }

    #[test]
    fn member_chains_and_calls_in_keys() {
        // `:each` の典型: item.id
        let item = Value::record([("id".into(), Value::number(7))]);
        let scope = Scope::new().with("item", Binding::Const(item));
        assert_eq!(
            parse_expr("item.id").unwrap().eval(&scope),
            Value::number(7)
        );
    }

    #[test]
    fn errors_are_reported() {
        assert!(parse_expr("1 +").is_err());
        assert!(parse_expr("(1 + 2").is_err());
        assert!(parse_expr("1 = 2").is_err());
        assert!(parse_expr("@bad").is_err());
        assert!(parse_expr("1 2").is_err()); // 余分な入力
    }
}
