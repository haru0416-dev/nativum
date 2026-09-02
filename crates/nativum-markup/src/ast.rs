//! Markup AST. Structure tags (`for`, `if`, `template`) stay in the tree until expand.

use crate::expr::Expr;
use crate::span::Span;

/// A parsed document: optional templates then a single view root.
#[derive(Clone, Debug)]
pub struct Document {
    /// Imported component paths, in order (`<import src="..."/>`).
    pub imports: Vec<(String, Span)>,
    /// Templates defined in this file (and later spliced imports).
    pub templates: Vec<Template>,
    /// The view root element.
    pub root: Node,
}

/// A reusable subtree. Args bind like `for` variables at the use site.
#[derive(Clone, Debug)]
pub struct Template {
    /// Template name.
    pub name: String,
    /// Declared args: `(name, optional literal default)`.
    pub args: Vec<(String, Option<String>)>,
    /// Body nodes (may include a single `<slot/>`).
    pub body: Vec<Node>,
    /// Source span of the opening tag.
    pub span: Span,
}

/// One markup node.
#[derive(Clone, Debug)]
pub enum Node {
    /// Element or structure tag.
    Element {
        /// Tag name (`row`, `button`, `for`, ...).
        name: String,
        /// Attributes in source order.
        attrs: Vec<Attribute>,
        /// Children.
        children: Vec<Node>,
        /// Opening tag span.
        span: Span,
    },
    /// Text run, possibly with interpolations.
    Text {
        /// Alternating literal / expression pieces. Literals may be empty.
        parts: Vec<TextPart>,
        /// Span of the whole run.
        span: Span,
    },
}

/// A slice of element text content.
#[derive(Clone, Debug)]
pub enum TextPart {
    /// Literal characters (HTML entities already decoded).
    Literal(String),
    /// `{expression}`.
    Expr(Expr),
}

/// An attribute. Values are either a literal or a single expression.
#[derive(Clone, Debug)]
pub struct Attribute {
    /// Attribute name (`on-press`, `gap`, `checked`, ...).
    pub name: String,
    /// Raw source string inside the quotes, without the surrounding quotes.
    pub raw: String,
    /// Parsed form.
    pub value: AttrValue,
    /// Span of the value.
    pub span: Span,
}

/// Literal vs `{expr}`.
#[derive(Clone, Debug)]
pub enum AttrValue {
    /// Plain string. May still contain a message spec (`done:{h.id}`) for `on-*`.
    Literal(String),
    /// Entire value is one `{expression}`.
    Expr(Expr),
}

impl Node {
    /// Element tag name, if this is an element.
    pub fn tag(&self) -> Option<&str> {
        match self {
            Self::Element { name, .. } => Some(name.as_str()),
            Self::Text { .. } => None,
        }
    }

    /// Attribute by name.
    pub fn attr(&self, name: &str) -> Option<&Attribute> {
        match self {
            Self::Element { attrs, .. } => attrs.iter().find(|a| a.name == name),
            Self::Text { .. } => None,
        }
    }
}
