//! `.native` markup: parser, expression language, and static checks.

#![forbid(unsafe_code)]

mod ast;
mod check;
mod eval;
mod expr;
mod parse;
mod span;

pub use ast::{AttrValue, Attribute, Document, Node, Template, TextPart};
pub use check::{check_document, split_message_spec, CheckReport};
pub use eval::{eval, Scope};
pub use expr::{parse_expr, Expr};
pub use parse::parse_document;
pub use span::Span;
