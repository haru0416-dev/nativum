//! Static checks: unknown elements, unknown tokens, missing bindings, missing message tags.

use std::collections::BTreeSet;

use nativum_core::{Error, RadiusToken};

use crate::ast::{AttrValue, Document, Node, Template};
use crate::expr::Expr;

/// Known markup elements for this subset. Unknown tags are teaching errors.
pub const ELEMENTS: &[&str] = &[
    "row",
    "column",
    "stack",
    "panel",
    "card",
    "scroll",
    "list",
    "grid",
    "spacer",
    "separator",
    "text",
    "span",
    "badge",
    "status-bar",
    "button",
    "toggle-button",
    "checkbox",
    "radio",
    "radio-group",
    "toggle",
    "switch",
    "text-field",
    "input",
    "search-field",
    "textarea",
    "slider",
    "progress",
    "alert",
    "dialog",
    "tabs",
    "list-item",
    "for",
    "if",
    "else",
    "template",
    "use",
    "slot",
    "import",
];

/// Color-token attribute names.
const COLOR_ATTRS: &[&str] = &[
    "background",
    "foreground",
    "accent",
    "accent-foreground",
    "border-color",
    "focus-ring",
];

/// A check diagnostic. `errors` fail `nativum check`; `warnings` do not.
#[derive(Clone, Debug, Default)]
pub struct CheckReport {
    /// Hard errors.
    pub errors: Vec<Error>,
    /// Teaching warnings.
    pub warnings: Vec<String>,
}

impl CheckReport {
    /// True when no errors were recorded.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Check `doc` against model field names, derived names, and message tags.
pub fn check_document(
    doc: &Document,
    model_keys: &BTreeSet<String>,
    message_tags: &BTreeSet<String>,
) -> CheckReport {
    let mut report = CheckReport::default();
    for t in &doc.templates {
        walk(
            &t.body,
            model_keys,
            message_tags,
            &t.args.iter().map(|(n, _)| n.clone()).collect(),
            &doc.templates,
            &mut report,
        );
    }
    walk(
        std::slice::from_ref(&doc.root),
        model_keys,
        message_tags,
        &BTreeSet::new(),
        &doc.templates,
        &mut report,
    );
    report
}

fn walk(
    nodes: &[Node],
    model_keys: &BTreeSet<String>,
    message_tags: &BTreeSet<String>,
    extra: &BTreeSet<String>,
    templates: &[Template],
    report: &mut CheckReport,
) {
    let mut i = 0;
    while i < nodes.len() {
        match &nodes[i] {
            Node::Text { parts, span } => {
                for part in parts {
                    if let crate::ast::TextPart::Expr(e) = part {
                        check_expr(e, model_keys, extra, report, span.line, span.column);
                    }
                }
            }
            Node::Element {
                name,
                attrs,
                children,
                span,
            } => {
                if !ELEMENTS.contains(&name.as_str()) {
                    report.errors.push(Error::at(
                        span.line,
                        span.column,
                        format!(
                            "unknown element <{name}> — see docs/DESIGN.md for the closed catalog"
                        ),
                    ));
                }
                let mut next_extra = extra.clone();
                if name == "for" {
                    let as_name = literal_attr(attrs, "as")
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "item".to_string());
                    next_extra.insert(as_name);
                    if let Some(each) = literal_attr(attrs, "each") {
                        if !each.is_empty() && !model_keys.contains(&each) && !extra.contains(&each)
                        {
                            report.errors.push(Error::at(
                                span.line,
                                span.column,
                                format!("for each=\"{each}\" is not a model field or derived list"),
                            ));
                        }
                    }
                }
                if name == "use" {
                    if let Some(tname) = literal_attr(attrs, "template") {
                        if !templates.iter().any(|t| t.name == tname) {
                            report.errors.push(Error::at(
                                span.line,
                                span.column,
                                format!("unknown template '{tname}'"),
                            ));
                        }
                    }
                }
                for attr in attrs {
                    check_attr(name, attr, model_keys, message_tags, extra, report);
                }
                walk(
                    children,
                    model_keys,
                    message_tags,
                    &next_extra,
                    templates,
                    report,
                );
            }
        }
        i += 1;
    }
}

fn literal_attr(attrs: &[crate::ast::Attribute], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name == name)
        .map(|a| match &a.value {
            AttrValue::Literal(s) => s.clone(),
            AttrValue::Expr(_) => String::new(),
        })
}

fn check_attr(
    tag: &str,
    attr: &crate::ast::Attribute,
    model_keys: &BTreeSet<String>,
    message_tags: &BTreeSet<String>,
    extra: &BTreeSet<String>,
    report: &mut CheckReport,
) {
    match &attr.value {
        AttrValue::Expr(e) => {
            check_expr(
                e,
                model_keys,
                extra,
                report,
                attr.span.line,
                attr.span.column,
            );
        }
        AttrValue::Literal(raw) => {
            if COLOR_ATTRS.contains(&attr.name.as_str())
                && nativum_core::tokens_for(nativum_core::Appearance::Light)
                    .color(raw)
                    .is_none()
                && nativum_core::Color::parse(raw).is_none()
            {
                report.errors.push(Error::at(
                    attr.span.line,
                    attr.span.column,
                    format!("unknown color token '{raw}'"),
                ));
            }
            if attr.name == "radius" && RadiusToken::parse(raw).is_none() {
                report.errors.push(Error::at(
                    attr.span.line,
                    attr.span.column,
                    format!("unknown radius token '{raw}' — use sm|md|lg|xl|none"),
                ));
            }
            if attr.name.starts_with("on-") {
                let (tag_name, payload) = split_message_spec(raw);
                if !message_tags.is_empty() && !message_tags.contains(tag_name) {
                    report.errors.push(Error::at(
                        attr.span.line,
                        attr.span.column,
                        format!(
                            "<{tag} {}=\"{raw}\"> names message '{tag_name}', which is not in the core",
                            attr.name
                        ),
                    ));
                }
                if let Some(p) = payload {
                    if let Ok(expr) = crate::expr::parse_expr(p, attr.span) {
                        check_expr(
                            &expr,
                            model_keys,
                            extra,
                            report,
                            attr.span.line,
                            attr.span.column,
                        );
                    }
                }
            }
        }
    }
}

fn check_expr(
    expr: &Expr,
    model_keys: &BTreeSet<String>,
    extra: &BTreeSet<String>,
    report: &mut CheckReport,
    line: usize,
    column: usize,
) {
    let mut roots = Vec::new();
    expr.root_idents(&mut roots);
    for name in roots {
        if name == "payload" {
            continue;
        }
        if !model_keys.contains(&name) && !extra.contains(&name) {
            report
                .errors
                .push(Error::at(line, column, format!("unknown binding '{name}'")));
        }
    }
}

/// Split `done:{h.id}` into (`done`, Some(`h.id`)).
pub fn split_message_spec(raw: &str) -> (&str, Option<&str>) {
    if let Some((tag, rest)) = raw.split_once(':') {
        let payload = rest
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(rest);
        (tag, Some(payload))
    } else {
        (raw, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_document;

    #[test]
    fn unknown_binding_is_an_error() {
        let doc = parse_document("<text>{nope}</text>").unwrap();
        let report = check_document(&doc, &BTreeSet::new(), &BTreeSet::new());
        assert!(!report.ok());
    }

    #[test]
    fn known_binding_passes() {
        let doc = parse_document("<text>{count}</text>").unwrap();
        let mut keys = BTreeSet::new();
        keys.insert("count".to_string());
        let report = check_document(&doc, &keys, &BTreeSet::new());
        assert!(report.ok(), "{:?}", report.errors);
    }
}
