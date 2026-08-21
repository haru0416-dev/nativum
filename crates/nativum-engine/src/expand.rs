//! Expand markup (`for`/`if`/`use`) into a widget tree with resolved bindings.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use nativum_core::{Color, Edges, Error, RadiusToken, Result, TokenSet, Value};
use nativum_markup::{
    eval, parse_expr, split_message_spec, AttrValue, Attribute, Document, Node, Scope, TextPart,
};

use crate::widget::{Align, Handler, Widget, WidgetKind};

/// Expand `doc` against `scope` and `tokens`.
pub fn expand(doc: &Document, scope: &mut Scope, tokens: &TokenSet) -> Result<Widget> {
    expand_node(&doc.root, doc, scope, tokens, 0)
}

fn expand_node(
    node: &Node,
    doc: &Document,
    scope: &mut Scope,
    tokens: &TokenSet,
    id_seed: u64,
) -> Result<Widget> {
    match node {
        Node::Text { parts, span } => {
            let text = interpolate(parts, scope)?;
            let mut w = Widget::new(WidgetKind::Text, mix_bytes(id_seed, text.as_bytes()));
            w.text = text;
            let _ = span;
            Ok(w)
        }
        Node::Element {
            name,
            attrs,
            children,
            span,
        } => match name.as_str() {
            "for" => expand_for(attrs, children, doc, scope, tokens, id_seed, *span),
            "if" => expand_if(node, attrs, children, doc, scope, tokens, id_seed),
            "else" => Ok(Widget::new(WidgetKind::Spacer, id_seed)),
            "use" => expand_use(attrs, children, doc, scope, tokens, id_seed, *span),
            "slot" => {
                // Slots are filled at the use site; a leftover slot is empty.
                Ok(Widget::new(WidgetKind::Spacer, id_seed))
            }
            _ => expand_element(name, attrs, children, doc, scope, tokens, id_seed, *span),
        },
    }
}

fn expand_for(
    attrs: &[Attribute],
    children: &[Node],
    doc: &Document,
    scope: &mut Scope,
    tokens: &TokenSet,
    id_seed: u64,
    span: nativum_markup::Span,
) -> Result<Widget> {
    let each = attr_literal(attrs, "each")
        .ok_or_else(|| Error::at(span.line, span.column, "<for> needs each=\"binding\""))?;
    let as_name = attr_literal(attrs, "as").unwrap_or_else(|| "item".to_string());
    let key_field = attr_literal(attrs, "key");
    let iterable = scope.lookup(&each).cloned().ok_or_else(|| {
        Error::at(
            span.line,
            span.column,
            format!("for each=\"{each}\" did not resolve"),
        )
    })?;
    let items = iterable.as_array().ok_or_else(|| {
        Error::at(
            span.line,
            span.column,
            format!(
                "for each=\"{each}\" must be an array, got {}",
                iterable.type_name()
            ),
        )
    })?;
    let mut wrapper = Widget::new(WidgetKind::Fragment, mix_bytes(id_seed, b"for"));
    wrapper.gap = 0.0;
    for (i, item) in items.iter().enumerate() {
        let mut frame = nativum_core::Value::object();
        frame.set_field(&as_name, item.clone())?;
        scope.push(frame);
        let key = key_field
            .as_ref()
            .and_then(|k| item.get(k))
            .map(|v| v.display())
            .unwrap_or_else(|| i.to_string());
        let child_seed = mix_bytes(id_seed, key.as_bytes());
        for (ci, child) in children.iter().enumerate() {
            if child.tag() == Some("else") {
                continue;
            }
            wrapper.children.push(expand_node(
                child,
                doc,
                scope,
                tokens,
                mix_u64(child_seed, ci as u64),
            )?);
        }
        scope.pop();
    }
    Ok(wrapper)
}

fn expand_if(
    node: &Node,
    attrs: &[Attribute],
    children: &[Node],
    doc: &Document,
    scope: &mut Scope,
    tokens: &TokenSet,
    id_seed: u64,
) -> Result<Widget> {
    let test = match node.attr("test") {
        Some(a) => resolve_attr(a, scope)?,
        None => {
            return Err(Error::Type("<if> needs test=\"{expr}\"".into()));
        }
    };
    let _ = attrs;
    let mut wrapper = Widget::new(WidgetKind::Fragment, mix_bytes(id_seed, b"if"));
    if test.is_truthy() {
        for (i, child) in children.iter().enumerate() {
            wrapper.children.push(expand_node(
                child,
                doc,
                scope,
                tokens,
                mix_u64(id_seed, i as u64 + 1),
            )?);
        }
    }
    Ok(wrapper)
}

fn expand_use(
    attrs: &[Attribute],
    children: &[Node],
    doc: &Document,
    scope: &mut Scope,
    tokens: &TokenSet,
    id_seed: u64,
    span: nativum_markup::Span,
) -> Result<Widget> {
    let name = attr_literal(attrs, "template")
        .ok_or_else(|| Error::at(span.line, span.column, "<use> needs template=\"name\""))?;
    let tpl = doc
        .templates
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| Error::at(span.line, span.column, format!("unknown template '{name}'")))?;
    let mut frame = nativum_core::Value::object();
    for (arg, default) in &tpl.args {
        if let Some(attr) = attrs.iter().find(|a| a.name == *arg) {
            frame.set_field(arg, resolve_attr(attr, scope)?)?;
        } else if let Some(d) = default {
            frame.set_field(arg, Value::String(d.clone()))?;
        } else {
            return Err(Error::at(
                span.line,
                span.column,
                format!("template '{name}' missing arg '{arg}'"),
            ));
        }
    }
    scope.push(frame);
    let mut wrapper = Widget::new(WidgetKind::Column, mix_bytes(id_seed, name.as_bytes()));
    for (i, child) in tpl.body.iter().enumerate() {
        if child.tag() == Some("slot") {
            for (j, passed) in children.iter().enumerate() {
                wrapper.children.push(expand_node(
                    passed,
                    doc,
                    scope,
                    tokens,
                    mix_u64(id_seed, (i as u64) << 8 | j as u64),
                )?);
            }
        } else {
            wrapper.children.push(expand_node(
                child,
                doc,
                scope,
                tokens,
                mix_u64(id_seed, i as u64),
            )?);
        }
    }
    scope.pop();
    Ok(wrapper)
}

#[allow(clippy::too_many_arguments)]
fn expand_element(
    name: &str,
    attrs: &[Attribute],
    children: &[Node],
    doc: &Document,
    scope: &mut Scope,
    tokens: &TokenSet,
    id_seed: u64,
    span: nativum_markup::Span,
) -> Result<Widget> {
    let kind = kind_of(name);
    let key = attr_resolved(attrs, "key", scope)
        .ok()
        .flatten()
        .map(|v| v.display())
        .or_else(|| {
            attr_resolved(attrs, "global-key", scope)
                .ok()
                .flatten()
                .map(|v| v.display())
        });
    let id = mix_bytes(id_seed, key.as_deref().unwrap_or(name).as_bytes());
    let mut w = Widget::new(kind, id);
    apply_common(&mut w, attrs, scope, tokens, span)?;

    // Text content from element children that are text, plus nested elements.
    let mut text = String::new();
    for (i, child) in children.iter().enumerate() {
        match child {
            Node::Text { parts, .. } => {
                text.push_str(&interpolate(parts, scope)?);
            }
            Node::Element { name, .. } if name == "else" => {
                // handled by parent if
            }
            Node::Element { name, .. } if name == "if" => {
                // Expand if; if false, look at following else sibling — handled below via flatten
                let expanded = expand_node(child, doc, scope, tokens, mix_u64(id, i as u64))?;
                if !test_is_false_if(child, scope)? {
                    if expanded.kind == WidgetKind::Fragment {
                        w.children.extend(expanded.children);
                    } else {
                        w.children.push(expanded);
                    }
                } else if let Some(Node::Element {
                    name,
                    children: else_children,
                    ..
                }) = children.get(i + 1)
                {
                    if name == "else" {
                        for (j, c) in else_children.iter().enumerate() {
                            w.children.push(expand_node(
                                c,
                                doc,
                                scope,
                                tokens,
                                mix_u64(id, ((i as u64) << 16) | j as u64),
                            )?);
                        }
                    }
                }
            }
            Node::Element { name, .. } if name == "else" => {
                // consumed by preceding if
                let prev_was_if =
                    matches!(children.get(i.wrapping_sub(1)), Some(n) if n.tag() == Some("if"));
                if !prev_was_if {
                    w.children.push(expand_node(
                        child,
                        doc,
                        scope,
                        tokens,
                        mix_u64(id, i as u64),
                    )?);
                }
            }
            other => {
                let expanded = expand_node(other, doc, scope, tokens, mix_u64(id, i as u64))?;
                if expanded.kind == WidgetKind::Fragment {
                    w.children.extend(expanded.children);
                } else {
                    w.children.push(expanded);
                }
            }
        }
    }
    if (!text.trim().is_empty()
        || matches!(
            kind,
            WidgetKind::Text
                | WidgetKind::Button
                | WidgetKind::Badge
                | WidgetKind::StatusBar
                | WidgetKind::Alert
        ))
        && w.text.is_empty()
    {
        w.text = collapse_ws(&text);
    }
    if w.label.is_empty() && !w.text.is_empty() {
        w.label = w.text.clone();
    }
    Ok(w)
}

fn test_is_false_if(node: &Node, scope: &Scope) -> Result<bool> {
    if node.tag() != Some("if") {
        return Ok(false);
    }
    if let Some(a) = node.attr("test") {
        return Ok(!resolve_attr(a, scope)?.is_truthy());
    }
    Ok(false)
}

fn kind_of(name: &str) -> WidgetKind {
    match name {
        "row" | "tabs" | "radio-group" | "button-group" | "toggle-group" => WidgetKind::Row,
        "column" | "list" => WidgetKind::Column,
        "stack" => WidgetKind::Stack,
        "panel" | "card" | "dialog" => WidgetKind::Surface,
        "scroll" => WidgetKind::Scroll,
        "spacer" => WidgetKind::Spacer,
        "separator" => WidgetKind::Separator,
        "text" | "span" => WidgetKind::Text,
        "badge" => WidgetKind::Badge,
        "status-bar" => WidgetKind::StatusBar,
        "button" | "toggle-button" | "list-item" => WidgetKind::Button,
        "checkbox" | "toggle" | "switch" => WidgetKind::Checkbox,
        "radio" => WidgetKind::Radio,
        "text-field" | "input" | "search-field" | "textarea" => WidgetKind::TextField,
        "slider" => WidgetKind::Slider,
        "progress" => WidgetKind::Progress,
        "alert" => WidgetKind::Alert,
        _ => WidgetKind::Column,
    }
}

fn apply_common(
    w: &mut Widget,
    attrs: &[Attribute],
    scope: &Scope,
    tokens: &TokenSet,
    span: nativum_markup::Span,
) -> Result<()> {
    for attr in attrs {
        match attr.name.as_str() {
            "gap" => w.gap = number_attr(attr, scope)?,
            "padding" => w.padding = padding_attr(attr, scope)?,
            "grow" => w.grow = number_attr(attr, scope)?,
            "width" => w.width = Some(number_attr(attr, scope)?),
            "height" => w.height = Some(number_attr(attr, scope)?),
            "min-width" => w.min_width = Some(number_attr(attr, scope)?),
            "main" => w.main = Align::parse(&raw_or_eval(attr, scope)?),
            "cross" => w.cross = Align::parse(&raw_or_eval(attr, scope)?),
            "text-alignment" => w.text_align = Align::parse(&raw_or_eval(attr, scope)?),
            "variant" => w.variant = raw_or_eval(attr, scope)?,
            "size" => {
                w.size = raw_or_eval(attr, scope)?;
                w.font_scale = match w.size.as_str() {
                    "sm" => 2,
                    "lg" | "heading" => 3,
                    "display" => 4,
                    _ => 2,
                };
            }
            "wrap" => w.wrap = truthy_attr(attr, scope)?,
            "disabled" => w.disabled = truthy_attr(attr, scope)?,
            "checked" | "selected" => w.selected = truthy_attr(attr, scope)?,
            "text" => w.text = resolve_attr(attr, scope)?.display(),
            "placeholder" => w.placeholder = resolve_attr(attr, scope)?.display(),
            "label" => w.label = resolve_attr(attr, scope)?.display(),
            "value" => w.value = number_attr(attr, scope)?,
            "background" => w.background = color_attr(attr, scope, tokens)?,
            "foreground" => w.foreground = color_attr(attr, scope, tokens)?,
            "border-color" => w.border = color_attr(attr, scope, tokens)?,
            "radius" => {
                w.radius = RadiusToken::parse(&raw_or_eval(attr, scope)?)
                    .map(|r| r.px())
                    .unwrap_or(0.0);
            }
            "on-press" => w.on_press = Some(handler_attr(attr, scope, span)?),
            "on-toggle" => w.on_toggle = Some(handler_attr(attr, scope, span)?),
            "on-input" => w.on_input = Some(handler_attr(attr, scope, span)?),
            "on-submit" => w.on_submit = Some(handler_attr(attr, scope, span)?),
            _ => {}
        }
    }
    Ok(())
}

fn handler_attr(attr: &Attribute, scope: &Scope, span: nativum_markup::Span) -> Result<Handler> {
    let spec = raw_or_eval(attr, scope)?;
    // If the attribute was an expression, `spec` is its display — handlers should be literals.
    let raw = match &attr.value {
        AttrValue::Literal(s) => s.as_str(),
        AttrValue::Expr(_) => spec.as_str(),
    };
    let (kind, payload_src) = split_message_spec(raw);
    let payload = match payload_src {
        Some(src) if !src.is_empty() => Some(parse_expr(src, span)?),
        _ => None,
    };
    Ok(Handler {
        kind: kind.to_string(),
        payload,
        scope: scope.extras(),
    })
}

fn interpolate(parts: &[TextPart], scope: &Scope) -> Result<String> {
    let mut out = String::new();
    for part in parts {
        match part {
            TextPart::Literal(s) => out.push_str(s),
            TextPart::Expr(e) => out.push_str(&eval(e, scope)?.display()),
        }
    }
    Ok(out)
}

fn collapse_ws(s: &str) -> String {
    let trimmed = s.trim();
    let mut out = String::new();
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            prev_space = false;
            out.push(ch);
        }
    }
    out
}

fn attr_literal(attrs: &[Attribute], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name == name)
        .map(|a| match &a.value {
            AttrValue::Literal(s) => s.clone(),
            AttrValue::Expr(_) => String::new(),
        })
}

fn attr_resolved(attrs: &[Attribute], name: &str, scope: &Scope) -> Result<Option<Value>> {
    match attrs.iter().find(|a| a.name == name) {
        Some(a) => Ok(Some(resolve_attr(a, scope)?)),
        None => Ok(None),
    }
}

fn resolve_attr(attr: &Attribute, scope: &Scope) -> Result<Value> {
    match &attr.value {
        AttrValue::Expr(e) => eval(e, scope),
        AttrValue::Literal(s) => {
            if let Ok(n) = s.parse::<f64>() {
                Ok(Value::Number(n))
            } else if s == "true" {
                Ok(Value::Bool(true))
            } else if s == "false" {
                Ok(Value::Bool(false))
            } else {
                Ok(Value::String(s.clone()))
            }
        }
    }
}

fn raw_or_eval(attr: &Attribute, scope: &Scope) -> Result<String> {
    Ok(resolve_attr(attr, scope)?.display())
}

fn number_attr(attr: &Attribute, scope: &Scope) -> Result<f32> {
    let v = resolve_attr(attr, scope)?;
    match v {
        Value::Number(n) => Ok(n as f32),
        Value::String(s) => s.parse::<f32>().map_err(|_| {
            Error::at(
                attr.span.line,
                attr.span.column,
                format!("expected a number, got '{s}'"),
            )
        }),
        other => Err(Error::at(
            attr.span.line,
            attr.span.column,
            format!("expected a number, got {}", other.type_name()),
        )),
    }
}

fn truthy_attr(attr: &Attribute, scope: &Scope) -> Result<bool> {
    Ok(resolve_attr(attr, scope)?.is_truthy())
}

fn color_attr(attr: &Attribute, scope: &Scope, tokens: &TokenSet) -> Result<Option<Color>> {
    let name = raw_or_eval(attr, scope)?;
    if let Some(c) = tokens.color(&name) {
        return Ok(Some(c));
    }
    if let Some(c) = Color::parse(&name) {
        return Ok(Some(c));
    }
    Err(Error::at(
        attr.span.line,
        attr.span.column,
        format!("unknown color '{name}'"),
    ))
}

fn padding_attr(attr: &Attribute, scope: &Scope) -> Result<Edges> {
    let s = raw_or_eval(attr, scope)?;
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        [a] => Ok(Edges::all(a.parse::<f32>().unwrap_or(0.0))),
        [v, h] => Ok(Edges::vh(
            v.parse::<f32>().unwrap_or(0.0),
            h.parse::<f32>().unwrap_or(0.0),
        )),
        [t, h, b] => Ok(Edges {
            top: t.parse().unwrap_or(0.0),
            right: h.parse().unwrap_or(0.0),
            bottom: b.parse().unwrap_or(0.0),
            left: h.parse().unwrap_or(0.0),
        }),
        [t, r, b, l] => Ok(Edges {
            top: t.parse().unwrap_or(0.0),
            right: r.parse().unwrap_or(0.0),
            bottom: b.parse().unwrap_or(0.0),
            left: l.parse().unwrap_or(0.0),
        }),
        _ => Ok(Edges::all(0.0)),
    }
}

fn mix_bytes(seed: u64, extra: impl AsRef<[u8]>) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    extra.as_ref().hash(&mut h);
    h.finish()
}

fn mix_u64(seed: u64, extra: u64) -> u64 {
    seed ^ extra.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nativum_core::{tokens_for, Appearance};
    use nativum_markup::parse_document;

    #[test]
    fn expands_binding() {
        let doc = parse_document("<text>{count}</text>").unwrap();
        let mut scope = Scope::new(Value::from_json(serde_json::json!({"count": 4})));
        let tokens = tokens_for(Appearance::Light);
        let w = expand(&doc, &mut scope, &tokens).unwrap();
        assert_eq!(w.text, "4");
    }
}
