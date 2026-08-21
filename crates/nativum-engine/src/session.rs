//! The live app: expand → layout → paint → hit-test → update.

use std::collections::BTreeMap;

use nativum_core::{
    tokens_for, Appearance, Command, Message, Point, Result, Size, TokenSet, Value,
};
use nativum_markup::Scope;
use nativum_markup::{parse_document, Document};

use crate::core_json::JsonCore;
use crate::expand::expand;
use crate::hit::{find_handler, first_field, hit_test};
use crate::layout::layout;
use crate::manifest::{LoadedApp, WindowSpec};
use crate::paint::{paint, Surface};
use crate::png::encode_png;
use crate::snapshot::SnapshotNode;
use crate::widget::Widget;

/// One painted frame plus the tree that produced it.
pub struct Frame {
    /// Root widget with frames assigned.
    pub root: Widget,
    /// Pixels.
    pub surface: Surface,
}

/// A journal step for `nativum replay` / tests.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum Step {
    /// Dispatch a named message, optionally with payload.
    Press {
        /// Message kind.
        press: String,
        /// Optional payload.
        #[serde(default)]
        payload: Option<serde_json::Value>,
    },
    /// Click at view coordinates.
    Click {
        /// `[x, y]`
        click: [f32; 2],
    },
    /// Type into the focused (or first) text field, then fire on-input.
    Type {
        /// Literal text to set (replacement, not append).
        r#type: String,
    },
    /// Append a character / handle Backspace / Enter.
    Key {
        /// Key name (`Enter`, `Backspace`, `a`, `+`, ...).
        key: String,
    },
}

/// Running application.
pub struct Session {
    /// Manifest window.
    pub window: WindowSpec,
    /// Tokens.
    pub tokens: TokenSet,
    appearance: Appearance,
    document: Document,
    core: JsonCore,
    model: Value,
    focused: Option<u64>,
    /// Last built tree. Empty until `rebuild`.
    pub root: Option<Widget>,
    pending: Vec<Command>,
}

impl Session {
    /// From a loaded app directory.
    pub fn from_loaded(app: LoadedApp) -> Result<Self> {
        let appearance = app.manifest.appearance();
        let document = parse_document(&app.view_source)?;
        let model = app.core.initial.clone();
        let mut session = Self {
            window: app.manifest.window,
            tokens: tokens_for(appearance),
            appearance,
            document,
            core: app.core,
            model,
            focused: None,
            root: None,
            pending: Vec::new(),
        };
        session.rebuild()?;
        Ok(session)
    }

    /// From in-memory sources (tests, `nativum init` dry-run).
    pub fn from_sources(
        view: &str,
        core: JsonCore,
        window: WindowSpec,
        appearance: Appearance,
    ) -> Result<Self> {
        let document = parse_document(view)?;
        let model = core.initial.clone();
        let mut session = Self {
            window,
            tokens: tokens_for(appearance),
            appearance,
            document,
            core,
            model,
            focused: None,
            root: None,
            pending: Vec::new(),
        };
        session.rebuild()?;
        Ok(session)
    }

    /// Replace markup (hot reload). Model is kept.
    pub fn reload_view(&mut self, source: &str) -> Result<()> {
        self.document = parse_document(source)?;
        self.rebuild()
    }

    /// Current model.
    pub fn model(&self) -> &Value {
        &self.model
    }

    /// Appearance.
    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<()> {
        self.appearance = appearance;
        self.tokens = tokens_for(appearance);
        self.rebuild()
    }

    /// Rebuild the widget tree from the model.
    pub fn rebuild(&mut self) -> Result<()> {
        let derived = self.core.derive(&self.model)?;
        let mut scope = Scope::new(self.model.clone()).with_derived(derived);
        let mut root = expand(&self.document, &mut scope, &self.tokens)?;
        if root.width.is_none() {
            root.width = Some(self.window.width);
        }
        if root.height.is_none() {
            root.height = Some(self.window.height);
        }
        if root.background.is_none() {
            root.background = Some(self.tokens.background);
        }
        layout(&mut root, Size::new(self.window.width, self.window.height));
        self.root = Some(root);
        Ok(())
    }

    /// Paint the current tree.
    pub fn frame(&self) -> Option<Frame> {
        let root = self.root.as_ref()?;
        let surface = paint(root, &self.tokens, self.focused);
        Some(Frame {
            root: root.clone(),
            surface,
        })
    }

    /// PNG bytes of the current frame.
    pub fn png(&self) -> Result<Vec<u8>> {
        let frame = self.frame().ok_or_else(|| {
            nativum_core::Error::Type("session has no frame; rebuild first".into())
        })?;
        Ok(encode_png(&frame.surface))
    }

    /// Accessibility snapshot.
    pub fn snapshot(&self) -> Option<SnapshotNode> {
        self.root.as_ref().map(SnapshotNode::from_widget)
    }

    /// Dispatch a message and rebuild.
    pub fn dispatch(&mut self, msg: Message) -> Result<()> {
        let cmds = self.core.apply(&mut self.model, &msg)?;
        self.pending.extend(cmds);
        self.rebuild()
    }

    /// Drain pending commands.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.pending)
    }

    /// Click at view coordinates.
    pub fn click(&mut self, x: f32, y: f32) -> Result<bool> {
        let hit = {
            let Some(root) = self.root.as_ref() else {
                return Ok(false);
            };
            hit_test(root, Point::new(x, y)).cloned()
        };
        let Some(hit) = hit else {
            self.focused = None;
            return Ok(false);
        };
        self.focused = Some(hit.id);
        if let Some(h) = hit.on_toggle.clone().or(hit.on_press.clone()) {
            let payload = eval_payload(&h, &self.model)?;
            self.dispatch(Message {
                kind: h.kind,
                payload,
            })?;
            return Ok(true);
        }
        if hit.on_input.is_some() {
            return Ok(true);
        }
        Ok(false)
    }

    /// Dispatch `press` by message kind (first matching widget).
    pub fn press_kind(&mut self, kind: &str, payload: Option<Value>) -> Result<bool> {
        if self.core.update.contains_key(kind) {
            self.dispatch(Message {
                kind: kind.to_string(),
                payload,
            })?;
            return Ok(true);
        }
        let found = self
            .root
            .as_ref()
            .is_some_and(|root| find_handler(root, kind).is_some());
        if found {
            self.dispatch(Message {
                kind: kind.to_string(),
                payload,
            })?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Type into the focused or first text field.
    pub fn type_text(&mut self, text: &str) -> Result<bool> {
        let handler = {
            let Some(root) = self.root.as_ref() else {
                return Ok(false);
            };
            let field = self
                .focused
                .and_then(|id| find_id(root, id))
                .or_else(|| first_field(root));
            field.and_then(|f| f.on_input.clone())
        };
        let Some(h) = handler else {
            return Ok(false);
        };
        self.dispatch(Message {
            kind: h.kind,
            payload: Some(Value::String(text.to_string())),
        })?;
        Ok(true)
    }

    /// Handle a key. App-level `core.keys` runs when no field consumes it.
    pub fn key(&mut self, key: &str) -> Result<bool> {
        if let Some(kind) = self.core.keys.get(key).cloned() {
            return self.press_kind(&kind, None);
        }
        if key == "Enter" {
            let handler = self
                .root
                .as_ref()
                .and_then(|root| first_field(root).and_then(|field| field.on_submit.clone()));
            if let Some(h) = handler {
                self.dispatch(Message::plain(h.kind))?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Run a journal of steps.
    pub fn replay(&mut self, steps: &[Step]) -> Result<()> {
        for step in steps {
            match step {
                Step::Press { press, payload } => {
                    let p = payload.clone().map(Value::from_json);
                    if !self.press_kind(press, p)? {
                        return Err(nativum_core::Error::Type(format!(
                            "journal press '{press}' did not match a message or widget"
                        )));
                    }
                }
                Step::Click { click } => {
                    if !self.click(click[0], click[1])? {
                        return Err(nativum_core::Error::Type(format!(
                            "journal click at ({}, {}) hit nothing",
                            click[0], click[1]
                        )));
                    }
                }
                Step::Type { r#type } => {
                    if !self.type_text(r#type)? {
                        return Err(nativum_core::Error::Type(
                            "journal type found no text field".into(),
                        ));
                    }
                }
                Step::Key { key } => {
                    if !self.key(key)? {
                        return Err(nativum_core::Error::Type(format!(
                            "journal key '{key}' was not handled"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Binding names for `nativum check`.
    pub fn binding_names(&self) -> BTreeMap<String, ()> {
        self.core
            .binding_names()
            .into_iter()
            .map(|k| (k, ()))
            .collect()
    }

    /// Core (for the checker).
    pub fn core(&self) -> &JsonCore {
        &self.core
    }

    /// Parsed document.
    pub fn document(&self) -> &Document {
        &self.document
    }
}

fn eval_payload(h: &crate::widget::Handler, model: &Value) -> Result<Option<Value>> {
    let Some(expr) = &h.payload else {
        return Ok(None);
    };
    let mut scope = Scope::new(model.clone());
    if !matches!(h.scope, Value::Object(ref m) if m.is_empty()) {
        scope.push(h.scope.clone());
    }
    Ok(Some(nativum_markup::eval(expr, &scope)?))
}

fn find_id(root: &Widget, id: u64) -> Option<&Widget> {
    if root.id == id {
        return Some(root);
    }
    for c in &root.children {
        if let Some(w) = find_id(c, id) {
            return Some(w);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::WindowSpec;

    const VIEW: &str = r#"
        <column padding="16" gap="12" background="background">
          <text size="heading">{count}</text>
          <row gap="8" main="center">
            <button variant="secondary" on-press="decrement">-</button>
            <button variant="primary" on-press="increment">+</button>
          </row>
          <button on-press="reset">Reset</button>
        </column>
    "#;

    fn counter() -> Session {
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
        Session::from_sources(VIEW, core, WindowSpec::default(), Appearance::Light).unwrap()
    }

    #[test]
    fn increment_via_kind() {
        let mut s = counter();
        s.press_kind("increment", None).unwrap();
        s.press_kind("increment", None).unwrap();
        assert_eq!(s.model().get("count").unwrap().display(), "2");
    }

    #[test]
    fn png_is_non_empty() {
        let s = counter();
        let png = s.png().unwrap();
        assert!(png.len() > 100);
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
