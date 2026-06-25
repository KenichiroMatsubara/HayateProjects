# 最初の desktop leaf を winit 単一 crate で着手し、Surface を vello/wgpu、入力を winit 抽象経由で全配線する（native IME・per-OS leaf は後続）

status: accepted

## Context

ADR-0117 で adapter 層を Core / Family Adapter / leaf の三層に再編し、desktop は **枠（ディレクトリ + grouping doctrine）だけ**を前払いした。だが desktop は **leaf 0** のまま — windowing も Platform Front（native binding）も Surface も無く、native でレンダリング結果を実際に画面へ出す経路が一本も無い。

ADR-0012 は **native を本体（primary target）** と定め、web が先行したのは開発速度の事情に過ぎないと明記する。App Host（ADR-0117）・`Surface` trait・vello/tiny-skia scene-renderer・Core の interaction/IME 増分 seam は揃っており、欠けているのは「OS の窓を開き、`App Host::tick` を回し、scene を present し、native 入力を Core へ流す」desktop 側の駆動・glue だけである。

ここで ADR-0117 のモデルと実装機構が一点擦れる。ADR-0117 は desktop を **3 つの per-OS leaf（macos / windows / linux）** としてモデル化するが、**winit + wgpu は 3 OS を windowing / event-loop / GPU surface の層で 1 crate に畳む**。per-OS に割れる必然が生じるのは native capability（audio 等）と native IME（TSF / TSM / IBus）に踏み込んだときであり、本決定はそこへは踏み込まない。

## Decision

desktop の最初の leaf を、**Platform Front（native binding）＋ Platform Adapter leaf glue** として、winit 単一 crate **`hayate-platform-desktop`** に置く。

- **粒度（winit が windowing 層で per-OS leaf を畳む）**: winit を desktop family の windowing 機構と位置づけ、macos / windows / linux を windowing / event-loop / `Surface` 層で 1 crate に統合する。これは ADR-0117 の per-OS leaf 像を **windowing 層では当面 collapse** させる前払い判断。per-OS leaf 分割は native capability / native IME 着手まで遅延する（その時点で割る）。Family Adapter / Capability（audio 等）の facade には**一切触れない** — desktop 枠の「空 facade を作らない」規律（ADR-0117）は維持される。
- **Surface = vello / wgpu（GPU）**: `Backend = wgpu 唯一`（CONTEXT.md）に一致する native primary の本番経路。vello crate の `render_to_texture` + `TextureBlitter` を winit の wgpu surface に blit する `Surface` 実装を leaf が持つ。tiny-skia（CPU）は確認用の位置づけで、desktop の本番 Surface には据えない。
- **フレーム駆動**: winit event loop が App Host を構築し、`request_redraw` クロージャを `window.request_redraw()` に配線、`RedrawRequested` で `tick(timestamp_ms)` を呼ぶ。継続フレーム判定（進行中 transition・`visual_dirty`・caret blink）は App Host が所有し、Platform Front はスケジューリングのみ（CONTEXT.md の Platform Front 定義）。
- **入力フル配線・ただし既存 Core seam への glue に徹する**:
  - **pointer**: winit `CursorMoved` / `MouseInput` → `on_pointer_down/move/up`（座標ベース・内部 hit-test）。`PointerKind = Mouse`。
  - **keyboard**: winit `KeyboardInput` → desktop keymap → `apply_edit_intent`。keymap は web の `edit_keymap.rs`（Cmd/Ctrl 分岐済み）を雛形とする。これは **web に次ぐ 2 実装目** で、将来 keymap を Core / common へ昇格させる prior art になる（今は昇格せず leaf に置く）。
  - **IME**: winit の `Ime::{Enabled, Preedit, Commit, Disabled}` → `ImeCommand` → `apply_command` → `apply_ime_action`（ADR-0117 の Core 所有増分モデル）。candidate window 位置は `set_ime_cursor_area` にフォーカス input の caret rect を渡す。**native IME（TSF / TSM / IBus）は後続フェーズ**に切る。
  - **resize / HiDPI**: winit の物理サイズ・`scale_factor` → `ViewportMetrics::from_physical_size` → `set_viewport` + wgpu surface 再 configure。
- **表示 = 共有 demo fixture**: 現状 tiny-skia test 内に埋まっている "Tasks" モックツリー構築を再利用可能な場所へ抽出し、desktop bin と tiny-skia test の双方から参照する（二重管理を避ける）。consumer（Hayabusa / Tsubame Canvas Renderer）は mount しない（`DeliverySink` 無し）。初手スコープは「表示 ＋ Core が直接解決する hover / active / focus / text-input 編集 / IME」までで、button の app レベルロジックは動かない（hover/active 等の擬似状態は Render Layer が解決するので視覚反応はする）。

CONTEXT.md の **Platform Front（native binding）** と **Platform Adapter（将来の macos 等）** を実体化するもので、**新規用語の追加は無い**。

## Considered Options

- **per-OS leaf（macos / windows / linux）を最初から 3 本（ADR-0117 に最も忠実）**: windowing は winit が畳めるので 3 重実装は冗長。native capability / IME が要求したときに割る方が安い。却下。
- **native windowing（AppKit / Win32 / X11・Wayland）直叩き**: leaf の「完全に platform 固有な glue」原則に最も忠実だが、風景を見るには過大で 3 実装ぶんの windowing を自前実装することになる。却下。
- **Surface を tiny-skia / softbuffer（CPU）で初手**: wgpu surface 設定が要らず配線は短いが、本番描画ではない確認用経路。native primary の風景としては偽物。却下。
- **native IME を最初から**: per-OS leaf 3 本 + `ImeBridge` 実装で過大。winit の IME 抽象で初手を 1 crate に畳み、native IME は後続へ。採用（winit IME 経由）。
- **winit + vello 単一 crate・入力は winit 抽象経由（採用）**: 1 crate で pointer / keyboard / IME を全配線でき、Core の既存 seam に glue として乗る。windowing 層の per-OS 分割と native IME を意識的に後続へ遅延する。

## Consequences

- 新 crate `hayate-platform-desktop`（workspace member 追加）。**winit が workspace dependency に入る**（wgpu / vello は既存）。
- `crates/platform/desktop/README.md` と `crates/platform/README.md` を「leaf 0 → **初手 windowing / 入力 leaf 着手**。capability facade（audio 等）は依然未着手で空 facade を作らない規律は維持」へ更新する。
- "Tasks" demo fixture を共有可能な場所へ抽出（tiny-skia test との二重管理を解消）。
- desktop keymap が web に次ぐ 2 実装目となり、将来の keymap 昇格（Core / common）の前払い prior art になる。
- ADR-0117 の per-OS leaf 像は **windowing 層で当面 collapse**。native capability（audio 等）/ native IME（TSF / TSM / IBus）着手時に per-OS leaf へ割る（その時点で taxonomy 調整）。
- 受容するリスク: winit を desktop windowing 機構として前払い採用するため、後で native windowing が必要になった場合に Platform Front を書き換える可能性がある。winit は Vello / Xilem エコシステムの標準で wgpu surface との実績が厚く、リスクは小さいと判断する。
- ADR-0012（native primary）・ADR-0117（三層モデル・desktop 枠）・ADR-0068（共有 seam の前払い）を継続。ADR-0087/0114（アダプタ間で windowing を共有しない）とは矛盾しない — winit が共有するのは **desktop family 内** の windowing であり、web / mobile / desktop を跨ぐ共有ではない。
