# nativum

[![ci](https://github.com/haru0416-dev/nativum/actions/workflows/ci.yml/badge.svg)](https://github.com/haru0416-dev/nativum/actions/workflows/ci.yml)
[![license: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![status: experimental](https://img.shields.io/badge/status-experimental-orange.svg)

[English](README.md) | **日本語**

**ネイティブデスクトップ UI を Rust で書くためのツールキット。** ビューは `.native` の宣言的マークアップ。ロジックは Model / Msg / `update`。ピクセルはエンジンが自分で描く。

ブラウザなし。WebView なし。成果物のバイナリに JS ランタイムは乗らない。

[Vercel Labs の Native SDK](https://github.com/vercel-labs/native)（Zig 製）の作者モデルにインスピレーションを受けた、独立したオリジナル実装。Vercel とは無関係で、ソースの移植でもない。[NOTICE](NOTICE) を参照。

## なぜ

Native SDK の賭けは写す価値がある。**表現力のある作者モデルは残し、重いランタイムだけを取り替える。** マークアップと純粋な `update` は人間にもエージェントにも書ける。ソフトウェアレンダラはテストがピン留めできる。

Native が TypeScript コアを Zig にコンパイルするのに対し、nativum のエンジンは **最初から Rust**。アプリコアはディレクトリだけで動く JSON TEA（`src/core.json`）か、同じエンジンを埋め込んだ Rust `Session`。

## クイックスタート

```sh
cargo install --path crates/nativum-cli
nativum init my_app
cd my_app
nativum check
nativum run              # 本物の OS ウィンドウ。クリックとキー入力
nativum render --out preview.png
nativum test
nativum preview          # http://127.0.0.1:8787 — フレームをクリック
```

`nativum run` はソフトウェアレンダラのフレームを OS ウィンドウに出す（winit + softbuffer）。ピクセルも `update` も同じ。`preview` はディスプレイがない環境向けの HTTP シェル。

## v0.1 でできること

- Native SDK のビューに近い **閉じたマークアップ**: `row` / `column` / `for` / `if` / `{bindings}` / `on-press="done:{h.id}"`、デザイントークン、テンプレート。
- **予測可能な状態。** イベントがメッセージを生み、メッセージが JSON モデルを更新し、ビューは導出される。
- **決定的なソフトウェアレンダラ。** 角丸矩形 + パブリックドメイン 8×8 フォント。圧縮クレートなしの PNG。入力が同じならバイトも同じ。
- **本物のウィンドウ。** `nativum run` が同じサーフェスを OS ウィンドウに出す。クリックとキーがメッセージになる。
- **ヘッドレスも残す。** `check` / `render` / `snapshot` / `replay` / `test` はウィンドウを開かない。`run --frames 1` は 1 フレーム出して終了（CI / xvfb）。
- **エージェント向けスナップショット。** 毎フレーム、ロール・フレーム・アクション付きのツリーを吐く。
- ショーケース: `apps/` の counter, calculator, notes, habits。

v0.1 に入れないもの（意図的）: GPU 提示、TypeScript コアのトランスパイラ、WebView、パッケージング。ループとピクセルが先。

## ライセンス

Apache-2.0。[LICENSE](LICENSE) を参照。
