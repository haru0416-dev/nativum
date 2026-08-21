# nativum design

Status: v0.1 experimental. This document is the spec the code is written against, not a retrospective.

## Bet

Expressive UI and native performance should not be competing goals. The web runtime is the usual way to buy authoring speed. nativum keeps a markup + TEA authoring model and draws pixels with its own engine.

Inspiration: Vercel Labs Native SDK. Independent implementation. Compatible-in-spirit `.native` dialect, not a grammar clone and not a Zig-to-Rust transliteration.

## Three files of truth

An app directory is:

| File | Role |
|------|------|
| `src/app.native` | View. Elements, flex, bindings, message dispatch. Never mutates. |
| `src/core.json` | `initial` model, `derived` bindings, `update` field assignments, optional `keys`. |
| `app.json` | Name, window size, paths, appearance. |

Rust library users skip `core.json` and drive `nativum_engine::Session` (or wrap `JsonCore`).

## Loop

1. Expand markup against the model (plus derived values, plus `for` frames).
2. Flex-layout the widget tree into the window size.
3. Paint a deterministic RGBA surface.
4. Input (click / key / named press) becomes a `Message`.
5. `update` assigns fields. Derived values are not stored.
6. Rebuild.

Errors degrade: a bad `update` expression is a diagnostic, not a panic in library paths (`Result` all the way down). The CLI exits non-zero.

## Markup

Closed catalog: see `nativum_markup::check::ELEMENTS`. Unknown tags are check errors.

Expressions inside `{...}` are a total, terminating language: paths, literals, arithmetic, `==` / `!=` / order, `and` / `or` / `not`, `++`, a closed function library. No user-defined functions, no effects, no computed message names.

Message attributes: `on-press="increment"` or `on-press="done:{h.id}"`. The payload is evaluated in the model plus the captured `for` frame.

Templates: `<template name args>` / `<use template>` / `<slot/>` / `<import src>`. Imports are recorded; v0.1 expands in-file templates. Cross-file import loading is a follow-up (the AST already carries the paths).

## JSON cores

`update` is a map of message kind → field → expression. Each expression sees the pre-update model and `payload`. Assignments are applied as a batch onto a clone, then swapped in.

`derived` runs on every rebuild in sorted name order; later names see earlier ones. Markup binds them like model fields.

Array helpers exist because list UIs are the point: `append`, `set_where`, `get_where`, `remove_where`, `filter_where`, `sum`, `len`.

## Renderer

- Logical pixels, top-left origin.
- Coverage-based rounded rects.
- 8×8 public-domain glyphs, integer scale (body = 2 → 16px cells).
- PNG via stored-block zlib, no `png`/`flate2` crate. Goldens can hash the file.

This will look like a well-kerned terminal, not SF Pro. That is acceptable for v0.1: determinism beats fashion. A TTF path (fontdue) is a later token swap, not an engine rewrite.

## Preview server

`nativum preview` binds a loopback HTTP server and serves the current PNG. Clicks map through `img.naturalWidth`. This is the development loop in environments without a compositor (CI, cloud agents).

## Non-goals for v0.1

- Real OS windows (winit / softbuffer is the obvious next host).
- Compiling markup out of the binary (proc-macro / `include_str` + parse at build is enough of a plan).
- A TypeScript core transpiler.
- WebView, bridge, mobile embed.
- Pixel-identical Native SDK goldens. Different font, different tokens, same *authoring* ideas.

## Testing contract

- Unit tests in crates cover parse, eval, layout, PNG signature, counter session.
- App journals in `apps/*/tests/*.json` cover update through the real markup.
- `nativum check` is the authoring gate: unknown bindings and unknown message tags fail.
