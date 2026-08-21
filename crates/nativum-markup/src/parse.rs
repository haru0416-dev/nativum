//! Recursive-descent parser for `.native` files.

use nativum_core::{Error, Result};

use crate::ast::{AttrValue, Attribute, Document, Node, Template, TextPart};
use crate::expr::parse_expr;
use crate::span::{line_col, Span};

/// Parse a complete document. Templates and imports must precede the single view root.
pub fn parse_document(source: &str) -> Result<Document> {
    let mut p = Parser { src: source, i: 0 };
    p.skip_ws_and_comments();
    let mut imports = Vec::new();
    let mut templates = Vec::new();
    while p.starts_with("<import") {
        imports.push(p.parse_import()?);
        p.skip_ws_and_comments();
    }
    while p.starts_with("<template") {
        templates.push(p.parse_template()?);
        p.skip_ws_and_comments();
    }
    if p.eof() {
        return Err(p.err("document is empty — expected a view root element"));
    }
    if !p.starts_with("<") {
        return Err(p.err("expected a view root element"));
    }
    let root = p.parse_node()?;
    if root.tag().is_none() {
        return Err(p.err("view root must be an element, not text"));
    }
    p.skip_ws_and_comments();
    if !p.eof() {
        return Err(p.err("unexpected content after the view root — a document has one root"));
    }
    Ok(Document {
        imports,
        templates,
        root,
    })
}

struct Parser<'a> {
    src: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn eof(&self) -> bool {
        self.i >= self.src.len()
    }

    fn rest(&self) -> &'a str {
        &self.src[self.i..]
    }

    fn starts_with(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn span_here(&self) -> Span {
        Span::at(self.src, self.i)
    }

    fn err(&self, message: &str) -> Error {
        let (line, column) = line_col(self.src, self.i);
        Error::at(line, column, message)
    }

    fn err_at(&self, i: usize, message: impl AsRef<str>) -> Error {
        let (line, column) = line_col(self.src, i);
        Error::at(line, column, message)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c.is_whitespace() {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            self.skip_ws();
            if self.starts_with("<!--") {
                self.i += 4;
                if let Some(rel) = self.rest().find("-->") {
                    self.i += rel + 3;
                } else {
                    // unterminated comment: consume the rest so the next error is useful
                    self.i = self.src.len();
                    return;
                }
            } else {
                break;
            }
        }
    }

    fn parse_import(&mut self) -> Result<(String, Span)> {
        let span = self.span_here();
        self.i += "<import".len();
        let attrs = self.parse_attrs()?;
        self.finish_open_tag()?;
        let src = attrs
            .iter()
            .find(|a| a.name == "src")
            .ok_or_else(|| self.err_at(span.start, "import needs src=\"path.native\""))?;
        Ok((src.raw.clone(), span))
    }

    fn finish_open_tag(&mut self) -> Result<bool> {
        self.skip_ws();
        if self.starts_with("/>") {
            self.i += 2;
            return Ok(true);
        }
        if self.starts_with(">") {
            self.i += 1;
            return Ok(false);
        }
        Err(self.err("expected '>' or '/>' after tag"))
    }

    fn parse_template(&mut self) -> Result<Template> {
        let span = self.span_here();
        self.i += "<template".len();
        let attrs = self.parse_attrs()?;
        let self_closing = self.finish_open_tag()?;
        let name = attrs
            .iter()
            .find(|a| a.name == "name")
            .ok_or_else(|| self.err_at(span.start, "template needs name=\"...\""))?
            .raw
            .clone();
        let args = parse_args_decl(
            attrs
                .iter()
                .find(|a| a.name == "args")
                .map(|a| a.raw.as_str())
                .unwrap_or(""),
        );
        if self_closing {
            return Ok(Template {
                name,
                args,
                body: Vec::new(),
                span,
            });
        }
        let body = self.parse_children("template")?;
        Ok(Template {
            name,
            args,
            body,
            span,
        })
    }

    fn parse_node(&mut self) -> Result<Node> {
        self.skip_ws_and_comments();
        if self.starts_with("<") && !self.starts_with("</") && !self.starts_with("<!--") {
            self.parse_element()
        } else {
            self.parse_text_until_tag()
        }
    }

    fn parse_element(&mut self) -> Result<Node> {
        let span = self.span_here();
        debug_assert!(self.starts_with("<"));
        self.i += 1;
        let name = self.parse_tag_name()?;
        let attrs = self.parse_attrs()?;
        let self_closing = self.finish_open_tag()?;
        if self_closing {
            return Ok(Node::Element {
                name,
                attrs,
                children: Vec::new(),
                span,
            });
        }
        let children = self.parse_children(&name)?;
        Ok(Node::Element {
            name,
            attrs,
            children,
            span,
        })
    }

    fn parse_children(&mut self, parent: &str) -> Result<Vec<Node>> {
        let mut children = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.eof() {
                return Err(self.err(&format!("unclosed <{parent}>")));
            }
            if self.starts_with("</") {
                self.i += 2;
                let close = self.parse_tag_name()?;
                self.skip_ws();
                if !self.starts_with(">") {
                    return Err(self.err("expected '>' after closing tag"));
                }
                self.i += 1;
                if close != parent {
                    return Err(self.err(&format!(
                        "mismatched closing tag </{close}> — open tag was <{parent}>"
                    )));
                }
                break;
            }
            children.push(self.parse_node()?);
        }
        Ok(children)
    }

    fn parse_tag_name(&mut self) -> Result<String> {
        let start = self.i;
        while let Some(c) = self.rest().chars().next() {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        if self.i == start {
            return Err(self.err("expected tag name"));
        }
        Ok(self.src[start..self.i].to_string())
    }

    fn parse_attrs(&mut self) -> Result<Vec<Attribute>> {
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.rest().chars().next() {
                Some('/') | Some('>') | None => break,
                Some(c) if is_attr_name_start(c) => attrs.push(self.parse_attr()?),
                Some(c) => {
                    return Err(self.err(&format!(
                        "unexpected '{c}' in tag — attribute names start with a letter"
                    )))
                }
            }
        }
        Ok(attrs)
    }

    fn parse_attr(&mut self) -> Result<Attribute> {
        let span = self.span_here();
        let name_start = self.i;
        while let Some(c) = self.rest().chars().next() {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        let name = self.src[name_start..self.i].to_string();
        self.skip_ws();
        if !self.starts_with("=") {
            // boolean attribute
            return Ok(Attribute {
                name,
                raw: "true".to_string(),
                value: AttrValue::Literal("true".to_string()),
                span,
            });
        }
        self.i += 1;
        self.skip_ws();
        let (raw, value_span) = self.parse_quoted()?;
        let value = parse_attr_value(&raw, value_span)?;
        Ok(Attribute {
            name,
            raw,
            value,
            span: value_span,
        })
    }

    fn parse_quoted(&mut self) -> Result<(String, Span)> {
        let quote = match self.rest().chars().next() {
            Some(q @ ('"' | '\'')) => q,
            _ => return Err(self.err("attribute value must be quoted")),
        };
        self.i += 1;
        let start = self.i;
        let span = self.span_here();
        while let Some(c) = self.rest().chars().next() {
            if c == quote {
                let raw = decode_entities(&self.src[start..self.i]);
                self.i += 1;
                return Ok((
                    raw,
                    Span {
                        start: span.start,
                        end: self.i,
                        line: span.line,
                        column: span.column,
                    },
                ));
            }
            self.i += c.len_utf8();
        }
        Err(self.err("unterminated attribute value"))
    }

    fn parse_text_until_tag(&mut self) -> Result<Node> {
        let span = self.span_here();
        let start = self.i;
        while !self.eof() {
            if self.starts_with("<!--") || self.starts_with("<") {
                break;
            }
            let ch = self.rest().chars().next().unwrap();
            self.i += ch.len_utf8();
        }
        let raw = decode_entities(&self.src[start..self.i]);
        let parts = split_interpolations(&raw, span)?;
        Ok(Node::Text { parts, span })
    }
}

fn is_attr_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn parse_args_decl(raw: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for tok in raw.split_whitespace() {
        if let Some((name, default)) = tok.split_once('=') {
            out.push((name.to_string(), Some(default.to_string())));
        } else if !tok.is_empty() {
            out.push((tok.to_string(), None));
        }
    }
    out
}

/// If the whole value is `{expr}`, parse it as an expression. Otherwise keep the literal
/// (message specs like `done:{h.id}` stay literal and are split later).
fn parse_attr_value(raw: &str, span: Span) -> Result<AttrValue> {
    let trimmed = raw.trim();
    if let Some(inner) = wrapped_expr(trimmed) {
        Ok(AttrValue::Expr(parse_expr(inner, span)?))
    } else {
        Ok(AttrValue::Literal(raw.to_string()))
    }
}

fn wrapped_expr(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with('{') && s.ends_with('}') && s.len() >= 2 {
        // Not an object literal in attributes — `{count}` is a binding, `{a == b}` too.
        // Object literals in attributes are not used.
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}

/// Split text into literal / `{expr}` parts.
pub fn split_interpolations(raw: &str, span: Span) -> Result<Vec<TextPart>> {
    let mut parts = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find('{') {
        let (lit, after) = rest.split_at(start);
        if !lit.is_empty() {
            parts.push(TextPart::Literal(lit.to_string()));
        }
        let inner_src = &after[1..];
        match find_matching_brace(inner_src) {
            Some(end) => {
                let inner = &inner_src[..end];
                parts.push(TextPart::Expr(parse_expr(inner, span)?));
                rest = &inner_src[end + 1..];
            }
            None => {
                return Err(Error::at(
                    span.line,
                    span.column,
                    "unterminated '{expression}' in text",
                ));
            }
        }
    }
    if !rest.is_empty() {
        parts.push(TextPart::Literal(rest.to_string()));
    }
    if parts.is_empty() {
        parts.push(TextPart::Literal(String::new()));
    }
    Ok(parts)
}

fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    let mut in_str = false;
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '\'' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_counter_fragment() {
        let src = r#"
            <row gap="8" main="center">
              <button variant="secondary" on-press="decrement">-</button>
              <text>{count}</text>
              <button variant="primary" on-press="increment">+</button>
            </row>
        "#;
        let doc = parse_document(src).unwrap();
        assert_eq!(doc.root.tag(), Some("row"));
    }

    #[test]
    fn parses_for_and_if() {
        let src = r#"
            <column>
              <if test="{habit_count}">
                <for each="visible" as="h" key="id">
                  <text>{h.name}</text>
                </for>
              </if>
              <else>
                <text>empty</text>
              </else>
            </column>
        "#;
        let doc = parse_document(src).unwrap();
        assert_eq!(doc.root.tag(), Some("column"));
    }

    #[test]
    fn reports_mismatched_tag() {
        let err = parse_document("<row></column>").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("mismatched"), "{msg}");
    }
}
