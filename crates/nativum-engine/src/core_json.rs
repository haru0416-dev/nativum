//! Data-driven TEA core: `src/core.json`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use nativum_core::{Command, Error, Message, Result, Value};
use nativum_markup::{eval, parse_expr, Expr, Scope, Span};

/// A JSON core: initial model, derived bindings, per-kind field updates, key map.
#[derive(Clone, Debug)]
pub struct JsonCore {
    /// Boot model.
    pub initial: Value,
    /// `name -> expression` evaluated on every view build, not stored.
    pub derived: BTreeMap<String, Expr>,
    /// `kind -> field -> expression`. Each field is assigned the eval result.
    pub update: BTreeMap<String, BTreeMap<String, Expr>>,
    /// Keyboard map: key name → message kind.
    pub keys: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct CoreFile {
    initial: serde_json::Value,
    #[serde(default)]
    derived: BTreeMap<String, String>,
    #[serde(default)]
    update: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

impl JsonCore {
    /// Parse from JSON text.
    pub fn parse(src: &str) -> Result<Self> {
        let file: CoreFile = serde_json::from_str(src)?;
        let dummy = Span::dummy();
        let mut derived = BTreeMap::new();
        for (k, v) in file.derived {
            derived.insert(k, parse_expr(&v, dummy)?);
        }
        let mut update = BTreeMap::new();
        for (kind, fields) in file.update {
            let mut map = BTreeMap::new();
            for (field, expr) in fields {
                map.insert(field, parse_expr(&expr, dummy)?);
            }
            update.insert(kind, map);
        }
        Ok(Self {
            initial: Value::from_json(file.initial),
            derived,
            update,
            keys: file.keys,
        })
    }

    /// Load from a file.
    pub fn load(path: &Path) -> Result<Self> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("cannot read {}: {e}", path.display())))?;
        Self::parse(&src).map_err(|e| match e {
            Error::Located { message, .. } => {
                Error::Config(format!("{}: {message}", path.display()))
            }
            other => other,
        })
    }

    /// Message tags this core handles.
    pub fn message_tags(&self) -> Vec<String> {
        self.update.keys().cloned().collect()
    }

    /// Model field names plus derived names.
    pub fn binding_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(obj) = self.initial.as_object() {
            names.extend(obj.keys().cloned());
        }
        names.extend(self.derived.keys().cloned());
        names
    }

    /// Evaluate derived bindings against `model`.
    pub fn derive(&self, model: &Value) -> Result<BTreeMap<String, Value>> {
        let mut out = BTreeMap::new();
        // Derived may depend on each other in declaration order.
        let mut scope = Scope::new(model.clone());
        for (name, expr) in &self.derived {
            let v = eval(expr, &scope)?;
            if let Value::Object(_) = scope.model() {
                // push as we go so later derived can see earlier ones
            }
            let mut frame = Value::object();
            frame.set_field(name, v.clone())?;
            scope.push(frame);
            out.insert(name.clone(), v);
        }
        Ok(out)
    }

    /// Apply `msg` to `model`.
    pub fn apply(&self, model: &mut Value, msg: &Message) -> Result<Vec<Command>> {
        let fields = self
            .update
            .get(&msg.kind)
            .ok_or_else(|| Error::Type(format!("no update handler for message '{}'", msg.kind)))?;
        let mut scope_model = model.clone();
        if let Some(payload) = &msg.payload {
            scope_model.set_field("payload", payload.clone())?;
        }
        let derived = self.derive(model).unwrap_or_default();
        let scope = Scope::new(scope_model).with_derived(derived);
        let mut next = model.clone();
        for (field, expr) in fields {
            let v = eval(expr, &scope)?;
            next.set_field(field, v)?;
        }
        *model = next;
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_update() {
        let core = JsonCore::parse(
            r#"{
                "initial": {"count": 0},
                "update": {
                    "increment": {"count": "count + 1"},
                    "decrement": {"count": "count - 1"},
                    "reset": {"count": "0"}
                }
            }"#,
        )
        .unwrap();
        let mut model = core.initial.clone();
        core.apply(&mut model, &Message::plain("increment"))
            .unwrap();
        core.apply(&mut model, &Message::plain("increment"))
            .unwrap();
        assert_eq!(model.get("count").unwrap().display(), "2");
        core.apply(&mut model, &Message::plain("reset")).unwrap();
        assert_eq!(model.get("count").unwrap().display(), "0");
    }
}
