//! Expression AST and parser. Spreadsheet power, not a programming language.

use nativum_core::{Error, Result};

use crate::span::Span;

/// A pure expression.
#[derive(Clone, Debug)]
pub enum Expr {
    /// `null`.
    Null(Span),
    /// `true` / `false`.
    Bool(bool, Span),
    /// Numeric literal.
    Number(f64, Span),
    /// Single-quoted string.
    String(String, Span),
    /// Identifier or dotted path start (`count`, `h`).
    Ident(String, Span),
    /// `obj.field`.
    Field {
        /// Receiver.
        base: Box<Expr>,
        /// Field name.
        name: String,
        /// Span covering `base.name`.
        span: Span,
    },
    /// `arr[index]`.
    Index {
        /// Receiver.
        base: Box<Expr>,
        /// Index expression.
        index: Box<Expr>,
        /// Span.
        span: Span,
    },
    /// `name(args...)`.
    Call {
        /// Function name.
        name: String,
        /// Arguments.
        args: Vec<Expr>,
        /// Span of the call.
        span: Span,
    },
    /// `{k: v, ...}` object literal (JSON-core updates, not typical markup).
    Object {
        /// Fields.
        fields: Vec<(String, Expr)>,
        /// Span.
        span: Span,
    },
    /// Unary `-` or `not`.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        expr: Box<Expr>,
        /// Span.
        span: Span,
    },
    /// Binary operator.
    Binary {
        /// Operator.
        op: BinOp,
        /// Left.
        left: Box<Expr>,
        /// Right.
        right: Box<Expr>,
        /// Span.
        span: Span,
    },
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
    /// Boolean not.
    Not,
}

/// Binary operators. `And`/`Or` evaluate both sides (expressions are pure).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/` always produces a float.
    Div,
    /// `++` string join.
    Concat,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `and`
    And,
    /// `or`
    Or,
}

impl Expr {
    /// Source span.
    pub fn span(&self) -> Span {
        match self {
            Self::Null(s)
            | Self::Bool(_, s)
            | Self::Number(_, s)
            | Self::String(_, s)
            | Self::Ident(_, s) => *s,
            Self::Field { span, .. }
            | Self::Index { span, .. }
            | Self::Call { span, .. }
            | Self::Object { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. } => *span,
        }
    }

    /// Collect identifier roots (for the checker). `h.name` yields `h`.
    pub fn root_idents(&self, out: &mut Vec<String>) {
        match self {
            Self::Ident(name, _) => out.push(name.clone()),
            Self::Field { base, .. }
            | Self::Index { base, .. }
            | Self::Unary { expr: base, .. } => {
                base.root_idents(out);
            }
            Self::Call { args, .. } => {
                for a in args {
                    a.root_idents(out);
                }
            }
            Self::Object { fields, .. } => {
                for (_, e) in fields {
                    e.root_idents(out);
                }
            }
            Self::Binary { left, right, .. } => {
                left.root_idents(out);
                right.root_idents(out);
            }
            Self::Null(_) | Self::Bool(_, _) | Self::Number(_, _) | Self::String(_, _) => {}
        }
    }
}

/// Parse an expression from a complete string (the inside of `{...}`).
pub fn parse_expr(src: &str, origin: Span) -> Result<Expr> {
    let mut p = Parser {
        src,
        origin,
        i: skip_ws(src, 0),
    };
    let expr = p.parse_or()?;
    p.i = skip_ws(src, p.i);
    if p.i != src.len() {
        return Err(p.err(format!(
            "unexpected trailing input in expression: '{}'",
            &src[p.i..]
        )));
    }
    Ok(expr)
}

struct Parser<'a> {
    src: &'a str,
    origin: Span,
    i: usize,
}

impl Parser<'_> {
    fn span_from(&self, start: usize) -> Span {
        Span {
            start: self.origin.start + start,
            end: self.origin.start + self.i,
            line: self.origin.line,
            column: self.origin.column,
        }
    }

    fn err(&self, message: impl AsRef<str>) -> Error {
        Error::at(self.origin.line, self.origin.column, message)
    }

    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.i += ch.len_utf8();
        Some(ch)
    }

    fn skip(&mut self) {
        self.i = skip_ws(self.src, self.i);
    }

    fn eat(&mut self, kw: &str) -> bool {
        self.skip();
        if self.src[self.i..].starts_with(kw) {
            let after = self.i + kw.len();
            let boundary_ok = after == self.src.len()
                || !self.src[after..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            let start_ok = self.i == 0
                || !self.src[..self.i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            // For symbolic ops, skip ident-boundary checks.
            let symbolic = !kw.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
            if symbolic || (boundary_ok && start_ok) {
                self.i = after;
                return true;
            }
        }
        false
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        while self.eat("or") {
            let right = self.parse_and()?;
            let span = left.span();
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_not()?;
        while self.eat("and") {
            let right = self.parse_not()?;
            let span = left.span();
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr> {
        self.skip();
        if self.eat("not") {
            let start = self.i;
            let expr = self.parse_not()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span: self.span_from(start),
            });
        }
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> Result<Expr> {
        let left = self.parse_concat()?;
        self.skip();
        let op = if self.eat("==") {
            BinOp::Eq
        } else if self.eat("!=") {
            BinOp::Ne
        } else if self.eat("<=") {
            BinOp::Le
        } else if self.eat(">=") {
            BinOp::Ge
        } else if self.eat("<") {
            BinOp::Lt
        } else if self.eat(">") {
            BinOp::Gt
        } else {
            return Ok(left);
        };
        let right = self.parse_concat()?;
        let span = left.span();
        Ok(Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        })
    }

    fn parse_concat(&mut self) -> Result<Expr> {
        let mut left = self.parse_add()?;
        while {
            self.skip();
            self.src[self.i..].starts_with("++")
        } {
            self.i += 2;
            let right = self.parse_add()?;
            let span = left.span();
            left = Expr::Binary {
                op: BinOp::Concat,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr> {
        let mut left = self.parse_mul()?;
        loop {
            self.skip();
            let op = if self.src[self.i..].starts_with("++") {
                break;
            } else if self.eat("+") {
                BinOp::Add
            } else if self.eat("-") {
                BinOp::Sub
            } else {
                break;
            };
            let right = self.parse_mul()?;
            let span = left.span();
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip();
            let op = if self.eat("*") {
                BinOp::Mul
            } else if self.eat("/") {
                BinOp::Div
            } else {
                break;
            };
            let right = self.parse_unary()?;
            let span = left.span();
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        self.skip();
        if self.eat("-") {
            let start = self.i;
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
                span: self.span_from(start),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            self.skip();
            if self.peek() == Some('.') {
                self.bump();
                self.skip();
                let name = self.parse_ident()?;
                let span = expr.span();
                expr = Expr::Field {
                    base: Box::new(expr),
                    name,
                    span,
                };
            } else if self.peek() == Some('[') {
                self.bump();
                let index = self.parse_or()?;
                self.skip();
                if self.peek() != Some(']') {
                    return Err(self.err("expected ']' after index"));
                }
                self.bump();
                let span = expr.span();
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        self.skip();
        let start = self.i;
        match self.peek() {
            Some('(') => {
                self.bump();
                let inner = self.parse_or()?;
                self.skip();
                if self.peek() != Some(')') {
                    return Err(self.err("expected ')'"));
                }
                self.bump();
                Ok(inner)
            }
            Some('{') => self.parse_object(),
            Some('\'') => {
                self.bump();
                let mut s = String::new();
                loop {
                    match self.bump() {
                        Some('\'') => break,
                        Some('\\') => match self.bump() {
                            Some(c) => s.push(c),
                            None => return Err(self.err("unterminated string")),
                        },
                        Some(c) => s.push(c),
                        None => return Err(self.err("unterminated string")),
                    }
                }
                Ok(Expr::String(s, self.span_from(start)))
            }
            Some(c) if c.is_ascii_digit() => {
                let mut end = self.i;
                while self.src[end..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_digit() || ch == '.')
                {
                    end += 1;
                }
                let n: f64 = self.src[self.i..end]
                    .parse()
                    .map_err(|_| self.err("invalid number"))?;
                self.i = end;
                Ok(Expr::Number(n, self.span_from(start)))
            }
            Some(c) if is_ident_start(c) => {
                let name = self.parse_ident()?;
                self.skip();
                if self.peek() == Some('(') {
                    self.bump();
                    let mut args = Vec::new();
                    self.skip();
                    if self.peek() != Some(')') {
                        loop {
                            args.push(self.parse_or()?);
                            self.skip();
                            if self.peek() == Some(',') {
                                self.bump();
                                self.skip();
                                continue;
                            }
                            break;
                        }
                    }
                    if self.peek() != Some(')') {
                        return Err(self.err("expected ')' after arguments"));
                    }
                    self.bump();
                    return Ok(Expr::Call {
                        name,
                        args,
                        span: self.span_from(start),
                    });
                }
                match name.as_str() {
                    "true" => Ok(Expr::Bool(true, self.span_from(start))),
                    "false" => Ok(Expr::Bool(false, self.span_from(start))),
                    "null" => Ok(Expr::Null(self.span_from(start))),
                    _ => Ok(Expr::Ident(name, self.span_from(start))),
                }
            }
            Some(c) => Err(self.err(format!("unexpected '{c}' in expression"))),
            None => Err(self.err("unexpected end of expression")),
        }
    }

    fn parse_object(&mut self) -> Result<Expr> {
        let start = self.i;
        self.bump(); // {
        let mut fields = Vec::new();
        self.skip();
        if self.peek() != Some('}') {
            loop {
                self.skip();
                let key = self.parse_ident()?;
                self.skip();
                if self.peek() != Some(':') {
                    return Err(self.err("expected ':' in object literal"));
                }
                self.bump();
                let value = self.parse_or()?;
                fields.push((key, value));
                self.skip();
                if self.peek() == Some(',') {
                    self.bump();
                    self.skip();
                    if self.peek() == Some('}') {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        if self.peek() != Some('}') {
            return Err(self.err("expected '}' at end of object literal"));
        }
        self.bump();
        Ok(Expr::Object {
            fields,
            span: self.span_from(start),
        })
    }

    fn parse_ident(&mut self) -> Result<String> {
        self.skip();
        let start = self.i;
        match self.peek() {
            Some(c) if is_ident_start(c) => {
                self.bump();
            }
            _ => return Err(self.err("expected identifier")),
        }
        while self.peek().is_some_and(is_ident_continue) {
            self.bump();
        }
        Ok(self.src[start..self.i].to_string())
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn skip_ws(src: &str, mut i: usize) -> usize {
    while let Some(c) = src[i..].chars().next() {
        if c.is_whitespace() {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn p(s: &str) -> Expr {
        parse_expr(s, Span::dummy()).unwrap()
    }

    #[test]
    fn parses_comparison_and_path() {
        match p("f == filter") {
            Expr::Binary { op: BinOp::Eq, .. } => {}
            other => panic!("{other:?}"),
        }
        match p("h.streak") {
            Expr::Field { name, .. } => assert_eq!(name, "streak"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_call_and_concat() {
        match p("plural(n, 'item', 'items')") {
            Expr::Call { name, args, .. } => {
                assert_eq!(name, "plural");
                assert_eq!(args.len(), 3);
            }
            other => panic!("{other:?}"),
        }
        match p("count ++ ' open'") {
            Expr::Binary {
                op: BinOp::Concat, ..
            } => {}
            other => panic!("{other:?}"),
        }
    }
}
