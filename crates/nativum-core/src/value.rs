//! JSON-shaped values used as the model, as expression results, and as message payloads.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::error::{Error, Result};

/// A JSON-like value. Objects use `BTreeMap` so iteration (and goldens) stay sorted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// JSON null.
    Null,
    /// Boolean.
    Bool(bool),
    /// Finite number. Integers are stored as `f64` but display without a decimal.
    Number(f64),
    /// UTF-8 text.
    String(String),
    /// Ordered array.
    Array(Vec<Value>),
    /// Sorted object.
    Object(BTreeMap<String, Value>),
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

impl Value {
    /// Empty object.
    pub fn object() -> Self {
        Self::Object(BTreeMap::new())
    }

    /// JSON type name for teaching errors.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// Native-style display: integers without `.0`, bools as `true`/`false`.
    pub fn display(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => format_number(*n),
            Self::String(s) => s.clone(),
            Self::Array(items) => items
                .iter()
                .map(Value::display)
                .collect::<Vec<_>>()
                .join(", "),
            Self::Object(_) => "[object]".to_string(),
        }
    }

    /// Truthiness for `<if test=...>`. Empty string, `0`, `false`, `null`, and `[]` are false.
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => !s.is_empty(),
            Self::Array(a) => !a.is_empty(),
            Self::Object(o) => !o.is_empty(),
        }
    }

    /// Number accessor.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// String accessor.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Object accessor.
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }

    /// Mutable object accessor.
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }

    /// Array accessor.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Field lookup on an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|m| m.get(key))
    }

    /// Dotted path `a.b.0.c`. Numeric segments index arrays.
    pub fn get_path(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for seg in path.split('.') {
            if seg.is_empty() {
                return None;
            }
            cur = match cur {
                Self::Object(m) => m.get(seg)?,
                Self::Array(a) => {
                    let i: usize = seg.parse().ok()?;
                    a.get(i)?
                }
                _ => return None,
            };
        }
        Some(cur)
    }

    /// Set a top-level object field. Errors if `self` is not an object.
    pub fn set_field(&mut self, key: &str, value: Value) -> Result<()> {
        match self {
            Self::Object(m) => {
                m.insert(key.to_string(), value);
                Ok(())
            }
            other => Err(Error::Type(format!(
                "cannot set field '{key}' on {}",
                other.type_name()
            ))),
        }
    }

    /// Merge `other`'s object fields into `self` (other wins). Both must be objects.
    pub fn merge_object(&mut self, other: &Value) -> Result<()> {
        let src = other.as_object().ok_or_else(|| {
            Error::Type(format!("cannot merge {} into object", other.type_name()))
        })?;
        let type_name = self.type_name();
        let dst = self
            .as_object_mut()
            .ok_or_else(|| Error::Type(format!("cannot merge into {type_name}")))?;
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    /// Convert from `serde_json::Value`.
    pub fn from_json(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(a) => {
                Self::Array(a.into_iter().map(Self::from_json).collect())
            }
            serde_json::Value::Object(o) => Self::Object(
                o.into_iter()
                    .map(|(k, v)| (k, Self::from_json(v)))
                    .collect(),
            ),
        }
    }

    /// Convert to `serde_json::Value`.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::Number(n) => number_to_json(*n),
            Self::String(s) => serde_json::Value::String(s.clone()),
            Self::Array(a) => serde_json::Value::Array(a.iter().map(Self::to_json).collect()),
            Self::Object(o) => {
                serde_json::Value::Object(o.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
        }
    }

    /// Deep equality used by `==` in expressions. Numbers compare as f64.
    pub fn eq_value(&self, other: &Value) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_value(y))
            }
            (Self::Object(a), Self::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| v.eq_value(w)))
            }
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Number(v as f64)
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Self::Number(v as f64)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

fn number_to_json(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) && !n.is_nan() {
        return serde_json::Value::Number(Number::from(n as i64));
    }
    Number::from_f64(n)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
}

/// Format a number the way interpolation should: `3` not `3.0`, full float otherwise.
pub fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
        format!("{}", n as i64)
    } else {
        let s = format!("{n}");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_display() {
        let v = Value::from_json(serde_json::json!({"h": {"name": "Read", "streak": 9}}));
        assert_eq!(v.get_path("h.name").unwrap().display(), "Read");
        assert_eq!(v.get_path("h.streak").unwrap().display(), "9");
        assert_eq!(
            v.get_path("h.streak").unwrap().to_json(),
            serde_json::json!(9)
        );
    }
}
