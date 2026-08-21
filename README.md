# nativum

[![ci](https://github.com/haru0416-dev/nativum/actions/workflows/ci.yml/badge.svg)](https://github.com/haru0416-dev/nativum/actions/workflows/ci.yml)
[![license: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![status: experimental](https://img.shields.io/badge/status-experimental-orange.svg)

**English** | [日本語](README.ja.md)

A toolkit for building **native desktop UIs in Rust**. Views are declarative markup in `.native` files. Logic is a Model / Msg / `update` loop. The engine draws every pixel itself.

No browser. No WebView. No JavaScript runtime in the binary.

Inspired by [Vercel Labs' Native SDK](https://github.com/vercel-labs/native) (Zig). nativum is an independent original implementation — not affiliated with Vercel, and not a port of that source tree. See [NOTICE](NOTICE).

## Why

Native SDK's bet is the one worth copying: **keep the expressive authoring model, replace the heavy runtime.** Markup + a pure update function is a shape humans and agents can both write. A software renderer is a shape tests can pin.

Where Native compiles TypeScript cores to Zig, nativum **is** Rust. App cores are either a tiny JSON TEA file (`src/core.json`) so a directory is enough, or a Rust `Session` embedding the same engine.

## Quick start

```sh
cargo install --path crates/nativum-cli
nativum init my_app
cd my_app
nativum check
nativum render --out preview.png
nativum test
nativum preview          # http://127.0.0.1:8787 — click the frame
```

A window-sized PNG is the whole UI. `preview` is an HTTP shell around that PNG so cloud agents and headless boxes can drive the app without a GPU or a display server.

The scaffolded counter is three files of truth:

```html
<!-- src/app.native -->
<row gap="8" main="center">
  <button variant="secondary" on-press="decrement">-</button>
  <text>{count}</text>
  <button variant="primary" on-press="increment">+</button>
</row>
```

```json
{
  "initial": { "count": 0 },
  "update": {
    "increment": { "count": "count + 1" },
    "decrement": { "count": "count - 1" },
    "reset": { "count": "0" }
  }
}
```

Markup can bind and dispatch. It cannot mutate. Every state change goes through `update`.

## What you get (v0.1)

- **Closed markup dialect** close to Native SDK views: `row` / `column` / `for` / `if` / `{bindings}` / `on-press="done:{h.id}"`, design tokens, templates.
- **Predictable state.** Events produce messages; messages update a JSON model; the view is derived.
- **Deterministic software renderer.** Rounded rects + a public-domain 8×8 font, PNG-encoded with no compression crate. Same inputs, same bytes.
- **Headless by default.** `check`, `render`, `snapshot`, `replay`, `test` never open a window.
- **Agent-friendly snapshot.** Every frame dumps an accessibility tree with roles, frames, and bound actions.
- **Showcase apps** in `apps/`: counter, calculator, notes, habits.

Not in v0.1 (on purpose): OS windows, GPU presentation, a TypeScript core transpiler, WebView, packaging. The loop and the pixels come first.

## Layout

| Path | What |
|------|------|
| `crates/nativum-core` | Values, tokens, geometry, messages |
| `crates/nativum-markup` | `.native` parser, expression language, checker |
| `crates/nativum-engine` | Expand, layout, paint, JSON cores, session |
| `crates/nativum-cli` | `nativum` binary |
| `crates/nativum` | Facade crate |
| `apps/` | Zero-config showcase apps |
| `docs/DESIGN.md` | Architecture and non-goals |

## License

Apache-2.0. See [LICENSE](LICENSE).
