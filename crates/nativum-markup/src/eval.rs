//! Evaluate expressions against a stacked scope of objects.

use std::collections::BTreeMap;

use nativum_core::{Error, Result, Value};

use crate::expr::{BinOp, Expr, UnaryOp};

/// Lookup chain: later entries shadow earlier ones. The model sits at index 0.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    frames: Vec<Value>,
}

impl Scope {
    /// Scope whose only frame is `model`.
    pub fn new(model: Value) -> Self {
        Self {
            frames: vec![model],
        }
    }

    /// The bottom frame (the model).
    pub fn model(&self) -> &Value {
        self.frames.first().unwrap_or(&Value::Null)
    }

    /// Mutable model.
    pub fn model_mut(&mut self) -> &mut Value {
        if self.frames.is_empty() {
            self.frames.push(Value::object());
        }
        &mut self.frames[0]
    }

    /// Push a loop/template binding frame (must be an object).
    pub fn push(&mut self, frame: Value) {
        self.frames.push(frame);
    }

    /// Pop a frame.
    pub fn pop(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    /// Resolve an identifier from the innermost frame that has it.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for frame in self.frames.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Clone every frame above the model into one object (loop/template bindings).
    pub fn extras(&self) -> Value {
        let mut merged = Value::object();
        for frame in self.frames.iter().skip(1) {
            let _ = merged.merge_object(frame);
        }
        merged
    }

    /// Merge derived bindings into a temporary top frame.
    pub fn with_derived(&self, derived: BTreeMap<String, Value>) -> Self {
        let mut clone = self.clone();
        clone.frames.push(Value::Object(derived));
        clone
    }
}

/// Evaluate `expr` in `scope`.
pub fn eval(expr: &Expr, scope: &Scope) -> Result<Value> {
    match expr {
        Expr::Null(_) => Ok(Value::Null),
        Expr::Bool(b, _) => Ok(Value::Bool(*b)),
        Expr::Number(n, _) => Ok(Value::Number(*n)),
        Expr::String(s, _) => Ok(Value::String(s.clone())),
        Expr::Ident(name, span) => scope.lookup(name).cloned().ok_or_else(|| {
            Error::at(
                span.line,
                span.column,
                format!(
                    "unknown binding '{name}' — not a model field, derived name, or loop variable"
                ),
            )
        }),
        Expr::Field { base, name, span } => {
            let v = eval(base, scope)?;
            v.get(name).cloned().ok_or_else(|| {
                Error::at(
                    span.line,
                    span.column,
                    format!("no field '{name}' on {}", v.type_name()),
                )
            })
        }
        Expr::Index { base, index, span } => {
            let v = eval(base, scope)?;
            let i = eval(index, scope)?;
            match (&v, &i) {
                (Value::Array(a), Value::Number(n)) => {
                    let idx = *n as i64;
                    if idx < 0 {
                        return Err(Error::at(span.line, span.column, "negative index"));
                    }
                    a.get(idx as usize).cloned().ok_or_else(|| {
                        Error::at(span.line, span.column, format!("index {idx} out of range"))
                    })
                }
                (Value::Object(o), Value::String(k)) => o
                    .get(k)
                    .cloned()
                    .ok_or_else(|| Error::at(span.line, span.column, format!("no field '{k}'"))),
                _ => Err(Error::at(
                    span.line,
                    span.column,
                    format!("cannot index {} with {}", v.type_name(), i.type_name()),
                )),
            }
        }
        Expr::Call { name, args, span } => eval_call(name, args, scope, *span),
        Expr::Object { fields, .. } => {
            let mut map = BTreeMap::new();
            for (k, e) in fields {
                map.insert(k.clone(), eval(e, scope)?);
            }
            Ok(Value::Object(map))
        }
        Expr::Unary { op, expr, span } => {
            let v = eval(expr, scope)?;
            match op {
                UnaryOp::Neg => {
                    let n = v.as_number().ok_or_else(|| {
                        Error::at(
                            span.line,
                            span.column,
                            format!("unary '-' needs a number, got {}", v.type_name()),
                        )
                    })?;
                    Ok(Value::Number(-n))
                }
                UnaryOp::Not => {
                    if let Value::Bool(b) = v {
                        Ok(Value::Bool(!b))
                    } else {
                        Err(Error::at(
                            span.line,
                            span.column,
                            format!("'not' needs a boolean, got {} — write `count > 0`, not `not count`", v.type_name()),
                        ))
                    }
                }
            }
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => eval_bin(*op, left, right, scope, *span),
    }
}

fn eval_bin(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    scope: &Scope,
    span: crate::span::Span,
) -> Result<Value> {
    let l = eval(left, scope)?;
    let r = eval(right, scope)?;
    match op {
        BinOp::Concat => Ok(Value::String(format!("{}{}", l.display(), r.display()))),
        BinOp::Eq => Ok(Value::Bool(l.eq_value(&r))),
        BinOp::Ne => Ok(Value::Bool(!l.eq_value(&r))),
        BinOp::And => match (&l, &r) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            _ => Err(Error::at(
                span.line,
                span.column,
                "'and' needs booleans on both sides",
            )),
        },
        BinOp::Or => match (&l, &r) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
            _ => Err(Error::at(
                span.line,
                span.column,
                "'or' needs booleans on both sides",
            )),
        },
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let a = l.as_number().ok_or_else(|| {
                Error::at(
                    span.line,
                    span.column,
                    "ordering comparison takes numbers only",
                )
            })?;
            let b = r.as_number().ok_or_else(|| {
                Error::at(
                    span.line,
                    span.column,
                    "ordering comparison takes numbers only",
                )
            })?;
            let bit = match op {
                BinOp::Lt => a < b,
                BinOp::Le => a <= b,
                BinOp::Gt => a > b,
                BinOp::Ge => a >= b,
                _ => unreachable!(),
            };
            Ok(Value::Bool(bit))
        }
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
            let a = l.as_number().ok_or_else(|| {
                Error::at(
                    span.line,
                    span.column,
                    format!("arithmetic needs numbers, left is {}", l.type_name()),
                )
            })?;
            let b = r.as_number().ok_or_else(|| {
                Error::at(
                    span.line,
                    span.column,
                    format!("arithmetic needs numbers, right is {}", r.type_name()),
                )
            })?;
            if op == BinOp::Div && b == 0.0 {
                return Err(Error::at(
                    span.line,
                    span.column,
                    "division by zero — nativum never returns NaN or a silent zero",
                ));
            }
            let n = match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => a / b,
                _ => unreachable!(),
            };
            Ok(Value::Number(n))
        }
    }
}

fn eval_call(name: &str, args: &[Expr], scope: &Scope, span: crate::span::Span) -> Result<Value> {
    let err = |m: &str| Error::at(span.line, span.column, m);
    let evaled: Vec<Value> = args.iter().map(|a| eval(a, scope)).collect::<Result<_>>()?;

    match name {
        "if" => {
            need(name, &evaled, 3, span)?;
            Ok(if evaled[0].is_truthy() {
                evaled[1].clone()
            } else {
                evaled[2].clone()
            })
        }
        "abs" => Ok(Value::Number(num(name, &evaled, 0, span)?.abs())),
        "min" => {
            if evaled.len() < 2 {
                return Err(err("min needs at least two numbers"));
            }
            let mut m = num(name, &evaled, 0, span)?;
            for i in 1..evaled.len() {
                m = m.min(num(name, &evaled, i, span)?);
            }
            Ok(Value::Number(m))
        }
        "max" => {
            if evaled.len() < 2 {
                return Err(err("max needs at least two numbers"));
            }
            let mut m = num(name, &evaled, 0, span)?;
            for i in 1..evaled.len() {
                m = m.max(num(name, &evaled, i, span)?);
            }
            Ok(Value::Number(m))
        }
        "round" => Ok(Value::Number(num(name, &evaled, 0, span)?.round())),
        "floor" => Ok(Value::Number(num(name, &evaled, 0, span)?.floor())),
        "ceil" => Ok(Value::Number(num(name, &evaled, 0, span)?.ceil())),
        "fixed" => {
            need(name, &evaled, 2, span)?;
            let x = num(name, &evaled, 0, span)?;
            let d = num(name, &evaled, 1, span)? as usize;
            Ok(Value::String(format!("{x:.d$}")))
        }
        "thousands" => {
            let n = num(name, &evaled, 0, span)? as i64;
            Ok(Value::String(group_thousands(n)))
        }
        "percent" => {
            let frac = num(name, &evaled, 0, span)?;
            let digits = if evaled.len() > 1 {
                num(name, &evaled, 1, span)? as usize
            } else {
                0
            };
            Ok(Value::String(format!("{:.digits$}%", frac * 100.0)))
        }
        "upper" => Ok(Value::String(text(name, &evaled, 0, span)?.to_ascii_uppercase())),
        "lower" => Ok(Value::String(text(name, &evaled, 0, span)?.to_ascii_lowercase())),
        "trim" => Ok(Value::String(text(name, &evaled, 0, span)?.trim().to_string())),
        "plural" => {
            need(name, &evaled, 3, span)?;
            let n = num(name, &evaled, 0, span)?;
            let one = text(name, &evaled, 1, span)?;
            let many = text(name, &evaled, 2, span)?;
            Ok(Value::String(if n == 1.0 { one } else { many }))
        }
        "pad" => {
            need(name, &evaled, 2, span)?;
            let x = num(name, &evaled, 0, span)? as i64;
            let width = num(name, &evaled, 1, span)? as usize;
            let sign = if x < 0 { "-" } else { "" };
            let body = format!("{}", x.abs());
            Ok(Value::String(format!("{sign}{body:0>width$}")))
        }
        "len" => {
            need(name, &evaled, 1, span)?;
            let n = match &evaled[0] {
                Value::Array(a) => a.len(),
                Value::String(s) => s.chars().count(),
                Value::Object(o) => o.len(),
                other => {
                    return Err(err(&format!(
                        "len() takes array, string, or object, got {}",
                        other.type_name()
                    )))
                }
            };
            Ok(Value::Number(n as f64))
        }
        "num" => {
            need(name, &evaled, 1, span)?;
            match &evaled[0] {
                Value::Number(n) => Ok(Value::Number(*n)),
                Value::String(s) => s
                    .parse::<f64>()
                    .map(Value::Number)
                    .map_err(|_| err(&format!("cannot parse '{s}' as a number"))),
                other => Err(err(&format!(
                    "num() takes string or number, got {}",
                    other.type_name()
                ))),
            }
        }
        "fmt" => {
            need(name, &evaled, 1, span)?;
            Ok(Value::String(evaled[0].display()))
        }
        "append" => {
            need(name, &evaled, 2, span)?;
            match &evaled[0] {
                Value::Array(a) => {
                    let mut next = a.clone();
                    next.push(evaled[1].clone());
                    Ok(Value::Array(next))
                }
                other => Err(err(&format!(
                    "append() needs an array, got {}",
                    other.type_name()
                ))),
            }
        }
        "sum" => {
            need(name, &evaled, 2, span)?;
            let field = text(name, &evaled, 1, span)?;
            let arr = evaled[0].as_array().ok_or_else(|| {
                err(&format!("sum() needs an array, got {}", evaled[0].type_name()))
            })?;
            let mut total = 0.0;
            for item in arr {
                total += item.get(&field).and_then(Value::as_number).unwrap_or(0.0);
            }
            Ok(Value::Number(total))
        }
        "contains" => {
            need(name, &evaled, 2, span)?;
            let hay = evaled[0].display().to_ascii_lowercase();
            let needle = evaled[1].display().to_ascii_lowercase();
            Ok(Value::Bool(hay.contains(&needle)))
        }
        "filter_where" => {
            need(name, &evaled, 4, span)?;
            filter_where(&evaled, span)
        }
        "set_where" => {
            need(name, &evaled, 4, span)?;
            set_where(&evaled, span)
        }
        "get_where" => {
            need(name, &evaled, 4, span)?;
            get_where(&evaled, span)
        }
        "remove_where" => {
            need(name, &evaled, 3, span)?;
            remove_where(&evaled, span)
        }
        "apply" => {
            need(name, &evaled, 3, span)?;
            apply_op(&evaled, span)
        }
        other => Err(err(&format!(
            "unknown function '{other}' — the expression library is closed; put reused logic in the core"
        ))),
    }
}

fn need(name: &str, args: &[Value], n: usize, span: crate::span::Span) -> Result<()> {
    if args.len() < n {
        Err(Error::at(
            span.line,
            span.column,
            format!("{name}() expects {n} argument(s), got {}", args.len()),
        ))
    } else {
        Ok(())
    }
}

fn num(name: &str, args: &[Value], i: usize, span: crate::span::Span) -> Result<f64> {
    args.get(i).and_then(Value::as_number).ok_or_else(|| {
        Error::at(
            span.line,
            span.column,
            format!("{name}() argument {} must be a number", i + 1),
        )
    })
}

fn text(name: &str, args: &[Value], i: usize, span: crate::span::Span) -> Result<String> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v) => Ok(v.display()),
        None => Err(Error::at(
            span.line,
            span.column,
            format!("{name}() missing argument {}", i + 1),
        )),
    }
}

fn filter_where(args: &[Value], span: crate::span::Span) -> Result<Value> {
    let arr = args[0]
        .as_array()
        .ok_or_else(|| Error::at(span.line, span.column, "filter_where() needs an array"))?;
    let field = args[1].display();
    let op = args[2].display();
    let rhs = &args[3];
    let mut out = Vec::new();
    for item in arr {
        let lhs = item.get(&field).cloned().unwrap_or(Value::Null);
        if compare_op(&lhs, &op, rhs) {
            out.push(item.clone());
        }
    }
    Ok(Value::Array(out))
}

fn compare_op(lhs: &Value, op: &str, rhs: &Value) -> bool {
    match op {
        "==" | "eq" => lhs.eq_value(rhs),
        "!=" | "ne" => !lhs.eq_value(rhs),
        ">" => cmp_num(lhs, rhs).is_some_and(|o| o.is_gt()),
        ">=" => cmp_num(lhs, rhs).is_some_and(|o| o.is_ge()),
        "<" => cmp_num(lhs, rhs).is_some_and(|o| o.is_lt()),
        "<=" => cmp_num(lhs, rhs).is_some_and(|o| o.is_le()),
        "contains" => lhs
            .display()
            .to_ascii_lowercase()
            .contains(&rhs.display().to_ascii_lowercase()),
        _ => false,
    }
}

fn cmp_num(lhs: &Value, rhs: &Value) -> Option<std::cmp::Ordering> {
    lhs.as_number()?.partial_cmp(&rhs.as_number()?)
}

fn set_where(args: &[Value], span: crate::span::Span) -> Result<Value> {
    let arr = args[0]
        .as_array()
        .ok_or_else(|| Error::at(span.line, span.column, "set_where() needs an array"))?;
    let key = args[1].display();
    let r#match = &args[2];
    let patch = args[3].as_object().ok_or_else(|| {
        Error::at(
            span.line,
            span.column,
            "set_where() patch must be an object",
        )
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        if item.get(&key).is_some_and(|v| v.eq_value(r#match)) {
            let mut obj = item.clone();
            if let Some(map) = obj.as_object_mut() {
                for (k, v) in patch {
                    map.insert(k.clone(), v.clone());
                }
            }
            out.push(obj);
        } else {
            out.push(item.clone());
        }
    }
    Ok(Value::Array(out))
}

fn get_where(args: &[Value], span: crate::span::Span) -> Result<Value> {
    let arr = args[0]
        .as_array()
        .ok_or_else(|| Error::at(span.line, span.column, "get_where() needs an array"))?;
    let key = args[1].display();
    let r#match = &args[2];
    let field = args[3].display();
    for item in arr {
        if item.get(&key).is_some_and(|v| v.eq_value(r#match)) {
            return Ok(item.get(&field).cloned().unwrap_or(Value::Null));
        }
    }
    Ok(Value::Null)
}

fn remove_where(args: &[Value], span: crate::span::Span) -> Result<Value> {
    let arr = args[0]
        .as_array()
        .ok_or_else(|| Error::at(span.line, span.column, "remove_where() needs an array"))?;
    let key = args[1].display();
    let r#match = &args[2];
    let out: Vec<Value> = arr
        .iter()
        .filter(|item| !item.get(&key).is_some_and(|v| v.eq_value(r#match)))
        .cloned()
        .collect();
    Ok(Value::Array(out))
}

fn apply_op(args: &[Value], span: crate::span::Span) -> Result<Value> {
    let acc = args[0].as_number().unwrap_or(0.0);
    let op = args[1].display();
    let n = args[2].as_number().unwrap_or(0.0);
    let result = match op.as_str() {
        "" | "none" => n,
        "add" | "+" => acc + n,
        "subtract" | "-" | "−" => acc - n,
        "multiply" | "*" | "×" => acc * n,
        "divide" | "/" | "÷" => {
            if n == 0.0 {
                return Err(Error::at(span.line, span.column, "division by zero"));
            }
            acc / n
        }
        other => {
            return Err(Error::at(
                span.line,
                span.column,
                format!("unknown operator '{other}'"),
            ))
        }
    };
    Ok(Value::Number(result))
}

fn group_thousands(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let grouped: String = out.chars().rev().collect();
    format!("{sign}{grouped}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::parse_expr;
    use crate::span::Span;

    fn run(src: &str, model: serde_json::Value) -> Value {
        let expr = parse_expr(src, Span::dummy()).unwrap();
        let scope = Scope::new(Value::from_json(model));
        eval(&expr, &scope).unwrap()
    }

    #[test]
    fn arithmetic_and_helpers() {
        assert_eq!(
            run("count + 1", serde_json::json!({"count": 3})),
            Value::Number(4.0)
        );
        assert_eq!(
            run(
                "plural(count, 'item', 'items')",
                serde_json::json!({"count": 2})
            )
            .display(),
            "items"
        );
        assert_eq!(nativum_core::format_number(3.0), "3");
    }

    #[test]
    fn set_where_patches_matching_row() {
        let v = run(
            "set_where(habits, 'id', 1, {streak: 10})",
            serde_json::json!({"habits": [{"id": 1, "streak": 2}, {"id": 2, "streak": 0}]}),
        );
        assert_eq!(v.get_path("0.streak").unwrap().display(), "10");
        assert_eq!(v.get_path("1.streak").unwrap().display(), "0");
    }
}
