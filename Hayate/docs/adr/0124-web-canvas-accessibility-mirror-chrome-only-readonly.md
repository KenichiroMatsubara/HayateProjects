# Web Canvas a11y を Chrome 限定・読み取り専用の TS ARIA ミラーで前倒し実装する

Canvas モードの `<canvas>` はアクセシビリティツリーから黒箱で、`poll_accessibility()` が完全な `accesskit::TreeUpdate`（JSON）を出しているのに公開先が無く、Playwright の `getByRole` 等で中身を一切検証できなかった。第一目的は AT ユーザーではなく **AI（クラウド実行の Claude）が Canvas アプリを `getByRole`/aria-snapshot で照会・アサートできるテスト面を作ること**。これを最短で得るため、`@hayate/host`（Hayate 自身の web ホスト TS glue、`createHayateWebHost` が canvas と `RawHayate` を握る）に `attachAccessibilityMirror(raw, canvas, …)` を新設し、`poll_accessibility()` の JSON を `<canvas>` 兄弟の**不可視 ARIA DOM** に投影する。ADR-0041 が定めた「Web Canvas a11y は Safari の EditContext 正式対応を待つ」ゲートを、**Chrome 限定**に割り切ることで本決定の範囲に限り前倒し（front-run）する。

## Status

accepted（ADR-0041 の Web Canvas 部分を Chrome スコープで前倒し。Safari トリガーや `accesskit-web` 全面対応は ADR-0041 のまま）

## Considered Options

- **Rust `platform/web`（web-sys）で ARIA DOM を構築** — ドクトリン（PLAT-03/04「Adapter が AT に報告」）に最も素直で `pseudo_style_dom.rs` の前例もある。しかし `poll_accessibility()` は**わざわざ JSON 化して JS に渡す**設計で、消費は TS が自然。web-sys の DOM 差分は冗長で、投影ルールを変えるたび wasm 再ビルドが要る（クラウドの反復が遅い）。却下。
- **Tsubame 側（renderer-hayate 等）に置く** — ARIA ブリッジはレンダラの責務でなく、ADR-0001 の Hayate↔Tsubame 境界を汚す。却下。`@hayate/host` なら Hayate 側に閉じつつ canvas+raw を既に握っている。
- **interactive ミラー（`getByRole().click()` を `on_accessibility_action` に往復）** — DOM アプリと同じ idiom で AI に最も自然。だが受信 `on_accessibility_action` は wasm/JS 未露出で binding 新設（Core/wasm 再ビルド）が要り、pointer-events 干渉・focusable 管理も伴う。v2 に defer。
- **Core に dirty/version signal を足して push 同期** — 最も効率的だが Core＋proto＋wasm を触る。v1 は poll＋JSON 文字列比較で間引く方式とし、プロファイルで必要が出たら入れる。

## Consequences

- v1 は**読み取り専用**。ミラーノードは各要素の on-canvas 矩形に絶対配置し `opacity:0` ＋ `pointer-events:none`（`display:none`/`visibility:hidden` は a11y ツリーから消えるので不可）。Playwright は `getByRole`/`toMatchAriaSnapshot` で**観測**し、対象を意味的に特定 → その `boundingBox()` から座標を得て `page.mouse.click` で**駆動**する。`getByRole().click()` の直接駆動は `pointer-events:none` のため不可で、v2（interactive）まで座標ホップが残る。
- 同期は `attachAccessibilityMirror` 内の自前 rAF ループで `poll_accessibility()` を毎フレーム呼び、**返り JSON が前回と同一なら DOM を触らない**。Core 変更ゼロ。reload teardown 用に detach 関数を返す。
- `@hayate/host` に置くため、**全 Canvas アプリが host boot 経由で自動的に**テスト可能な ARIA 面を得る（host-boot 毎の配線が不要）。
- 受容するリスク: Chrome 限定の前払い。Safari 対応や `accesskit-web` 全面化が必要になった時点で投影器を拡張する（ADR-0041 の本来計画に合流）。
