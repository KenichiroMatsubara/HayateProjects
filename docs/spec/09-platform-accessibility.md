# §9 Platform Adapter & Accessibility

Platform Adapter の責務範囲と、アクセシビリティ（AccessKit）の所有・展開順序。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### PLAT-01 — Platform Adapter の責務は三つ
**規範文:** Platform Adapter の責務は IME 入力・クリップボード・raw 入力イベント変換の三つに限定する。Core は Platform Adapter を知らない。サーフェス生成とフレームタイミングは wgpu が担い、アクセシビリティ報告は AccessKit が担うため、いずれも Adapter の責務に含めない。
**出典:** ADR-0014
**状況:** ✅ — `platform/web` は raw 入力変換（`on_pointer_*` / `on_wheel` / `on_key_down`）、IME（composition handlers）、clipboard（`element_paste`）に限定。Core は adapter 非依存。
**備考:** surface/present/resize は Render Host（§4 REND-14）、a11y は PLAT-03/04。

### PLAT-02 — クリップボードは element_paste
**規範文:** クリップボード貼り付けは `element_paste(id, text)` で `text-input` の `text_content` に反映する（active preedit があれば確定してから追記）。OS クリップボードの読み書きは host 側が担う。
**出典:** ADR-0014
**状況:** ✅ — `tree.rs:428` `element_paste()`、adapter `element_renderer.rs:563`、テスト `element_layer.rs:1118-1172`（空/追記/preedit確定/イベント発火/非text-input no-op）。
**備考:** —

### PLAT-03 — AccessKit は Core が所有
**規範文:** Core はアクセシビリティツリー（`accesskit::TreeUpdate`）の生成責務を持ち、`role` / `aria_label` 等の a11y データを Element に保持する。Platform Adapter は AccessKit のプラットフォーム実装を呼び OS の AT（UIA / NSAccessibility / AT-SPI / Web ARIA）に報告する。
**出典:** ADR-0041, `CONTEXT.md`「AccessKit」
**状況:** ✅ — `accesskit` 依存（`hayate-core`）；`ElementTree::accessibility_update()` が `layout_cache` 境界矩形 + `aria_label` / `role` から `TreeUpdate` を生成（`element/accessibility.rs`）；Canvas adapter `poll_accessibility()` が JSON 返却（`element_renderer.rs`）。
**備考:** Parley `LayoutAccessibility::build_nodes` による text run 詳細は将来。ネイティブ Platform Adapter への報告は PLAT-04。

### PLAT-04 — AccessKit 展開順序：ネイティブ優先
**規範文:** AccessKit 対応はネイティブ（UIA / NSAccessibility / AT-SPI）を優先し、Web Canvas Mode は Safari が EditContext API を正式サポートした時点で `accesskit-web`（不可視 ARIA DOM）を最優先で対応する。Web HTML Mode は実 DOM に ARIA 属性付与で対応する。
**出典:** ADR-0041
**状況:** 🟡 — 設計確定。Core `TreeUpdate` 生成は完了（PLAT-03）。ネイティブ AT 報告（UIA/NSAccessibility/AT-SPI）と Web Canvas `accesskit-web` は未着手。ネイティブ Platform Adapter は Android（PLAT-06）に加え iOS グラウンドワーク（PLAT-08）が存在するが、いずれも AccessKit 報告は未着手。
**備考:** ネイティブ Platform Adapter crate が前提。iOS は `UIAccessibility`、macOS は `NSAccessibility`。

### PLAT-05 — surface / frame timing / a11y はアダプタ責務外（設計境界）
**規範文:** サーフェス生成・フレームタイミングは wgpu、アクセシビリティ報告は AccessKit が担い、Platform Adapter の責務に含めない。
**出典:** ADR-0014, `CONTEXT.md`「Platform Adapter」
**状況:** ✅ — 「やらないこと」の境界規範。surface は Render Host、a11y は Core+AccessKit に分離（PLAT-01/03 と整合）。
**備考:** —

### PLAT-06 — Android を最初のネイティブ Platform Adapter とする（winit 不採用）
**規範文:** Android を最初のネイティブ Platform Adapter ターゲットとする（iOS は後続。グラウンドワークは PLAT-08）。段階スコープは (A) 描画スモークテスト（`hayate-adapter-android` crate + example、wgpu/Vulkan surface、入力/IME/AccessKit なし）→ (B) タッチ入力を Element Document Runtime に接続 → (C) `hayate-adapter-web` 同等のフルパリティ（IME ブリッジ・AccessKit・クリップボード）。Platform Adapter は `winit` 等の汎用ウィンドウ抽象を使わず、各プラットフォームのネイティブ API（Android は `android-activity`）に直接バインドする。stage C のビルド基盤は GameActivity（`android-activity` の `game-activity` バックエンド）+ Gradle とし、IME はソフトキーボードの `InputConnection` を GameTextInput 経由で native に上げる（ADR-0094、cargo-apk/native-activity から移行）。`hayate-core` はどのプラットフォーム依存も持たない。ADR-0051（Tsubame 優先）と並行トラックであり supersede ではない。
**出典:** ADR-0087, ADR-0094
**状況:** 🟡 — `crates/platform/mobile/android`（`lib.rs` / `surface_lifecycle.rs` / `touch_input.rs` / `scene_demo.rs` / `app.rs`、`tests/apk_packaging.rs`）が存在。(A) 描画スモーク完了。(B) タッチ入力に加え、ループが `tree.render()` で `ElementTree`→`SceneGraph` を lowering して毎フレーム present するようになり（`viewport_for_surface` で viewport を surface px に追従、`scene_demo` の `:active` ボタンでタップが画面に反映）、タッチが描画されないツリーを駆動していた穴を解消。(C) フルパリティ（IME / AccessKit / clipboard）は着手段階: パッケージング基盤を GameActivity + Gradle へ移行（`android-app/` の Gradle プロジェクト + `MainActivity : GameActivity` + Manifest、`Cargo.toml` を `game-activity` feature へ、cargo-apk metadata 撤去、ADR-0094）し、IME ブリッジを開始（`ime_input` が GameTextInput の絶対状態＝全文+composing region を core の「committed text_content + 末尾 preedit」モデルへ差分変換、`app.rs` がフォーカス時にソフトキーボード表示し focused TextInput へ適用）。AccessKit / clipboard と、CompositionStart/Update/End イベント発火・selection 対応は未着手。NDK/SDK/Gradle 不在環境では host テスト可能な純粋 seam（`surface_lifecycle` / `touch_input` / `scene_demo` / `ime_input`）と packaging 契約テスト（`tests/apk_packaging.rs` が Gradle/Manifest/Kotlin を読む）のみ検証可能で、`app.rs` の NDK glue・Gradle ビルドは実機/エミュレータ検証（#195）が必要。
**備考:** アダプタ間でウィンドウ/イベントループの共有コードは持たない（各アダプタが lifecycle/surface を再実装）。PLAT-04 のネイティブ AccessKit 報告は本アダプタを前提とする。

### PLAT-07 — AccessKit inbound action は Core が意味論写像（ポインタ非合成）
**規範文:** AT → Core の inbound アクションは Core が単独で既存 runtime intent へ写像する。`Click`/`Default` は既存 `Click` イベントを対象ノードへ直接 emit（合成ポインタ・`:active`・multi-click を経由しない、Flutter の semantic action と同型）。写像は Core の `on_accessibility_action` が所有し、Platform Adapter は OS の AT 配管として要求を橋渡しするだけ。アクション語彙は proto wire に載せず Core 内 `AccessibilityAction` enum（未対応は `Ignored`）。`SetTextSelection` は統一 Selection（ADR-0097）に `(ElementId, byte)` 一本で着地し、AccessKit `NodeId` は host `ElementId` から切り離した専用 dense `AccessIndex`（run は `(AccessIndex<<k)|local` でパック）で構成する。
**出典:** ADR-0098, ADR-0097, ADR-0041
**状況:** ⬜ — 設計確定（ADR-0098）。v1 アクション集合は {Focus, Click/Default, ScrollIntoView, SetValue}。`SetTextSelection` と outbound `set_text_selection` 反映は text-run a11y（Parley `LayoutAccessibility`）導入と同一作業単位で defer。実装はネイティブ Platform Adapter（PLAT-04/06）が前提で未着手。
**備考:** Web Canvas Mode の inbound は Safari EditContext 対応後に別 wire 拡張として設計（ADR-0041）。

### PLAT-08 — iOS を 2 つ目のネイティブ Platform Adapter とする（UIKit/Metal、薄い Swift ホスト、winit 不採用）
**規範文:** iOS を 2 つ目のネイティブ Platform Adapter ターゲットとし、Android（PLAT-06）の段階スコープ（A 描画スモーク → B タッチ → C IME/AccessKit/clipboard フルパリティ）と de-risk パターン（ホストでテスト可能な純粋シーム + `#[cfg(target_os="ios")]` ネイティブグルー + パッケージング契約テスト）を踏襲する。`winit` 等の汎用ウィンドウ抽象は使わない。UIKit / `UITextInput` / `CAMetalLayer` / `CADisplayLink` は薄い Swift ホスト（`AppDelegate`/`SceneDelegate`/`HayateView`）が所有し、`hayate_ios_*` C FFI 経由で Rust staticlib にライフサイクル・タッチ・IME を渡す（Android の「薄い Kotlin ホスト + Rust」の iOS 版、shape 1）。Rust は ObjC-free で、Swift が渡す `CAMetalLayer` ポインタから `wgpu::SurfaceTargetUnsafe::CoreAnimationLayer` で Metal サーフェスを張る。IME は Android（GameTextInput の絶対状態を差分）と異なり UITextInput の増分コールバック（`insertText`/`setMarkedText`/`unmarkText`/`deleteBackward`）をコマンド駆動でコアの「確定 text_content + 末尾 preedit」へ畳む（出力半分は Android と共有）。content scale は Android の 1.0 固定と異なり実 `UIScreen.scale`（Retina）を `ViewportMetrics::from_physical_size` に通す。`hayate-core` はどのプラットフォーム依存も持たない。ADR-0051（Tsubame 優先）と並行トラック。
**出典:** ADR-0114, ADR-0115, ADR-0116（Tsubame JS は方針のみ）
**状況:** 🟡 — グラウンドワーク。`crates/platform/mobile/ios`（純粋シーム `surface_lifecycle` / `touch_input` / `ime_input` / `scene_demo` をホストで全テスト、`#[cfg(target_os="ios")]` グルー `app.rs` / `ime_bridge.rs`、契約テスト `tests/ios_packaging.rs` / `tests/ime_api_encapsulation.rs`、Xcode 雛形 `ios-app/`）。ホスト検証可能なのは純粋シーム（状態機械・タッチ・IME コマンド→ImeAction の日本語変換込み・`apply_ime_action` を実 `ElementTree` に適用・`ViewportMetrics` 再利用）と両契約テストのみ。`app.rs` の Metal/FFI グルー・Swift ホスト・Xcode ビルド・実機 IME は Mac/シミュレータ/実機検証に残る（ADR-0087/0094 と同じ検証ギャップ、`aarch64-apple-ios` は本サンドボックス未インストールで Apple SDK も無いためターゲット compile-check も Mac 必須）。Tsubame JS 経路は ADR-0116 で Hermes パリティ方針のみ確定（コードなし）。
**備考:** AccessKit（`UIAccessibility`）報告は PLAT-04 同様未着手。スクロール物理（ADR-0046）・clipboard は defer。winit 不採用は本 leaf（iOS native）に掛かる規範であり、desktop family の windowing 機構としての winit 採用（PLAT-10）とは別レイヤー（ADR-0118 が ADR-0087/0114 と非矛盾と明記）。

### PLAT-09 — アダプター層は Core / Family Adapter / leaf の三層（capability を責務クラスに追加）
**規範文:** アダプター層を Core / Family Adapter / leaf の三層に分け、責務を性質で振り分ける。**Core** は platform-free な共通 seam を所有する（surface 状態機械＝`InitWindow`/`TerminateWindow`/`WindowResized`/`Destroy` の 4 論理イベント、touch 変換＝native enum→`TouchAction`→座標 pointer dispatch、IME 増分＝`ImeCommand`/`ImeBuffer`/`apply_command`（Android=絶対状態 diff / iOS=増分 command の両入力モデルを Core が所有））。**Family Adapter**（`platform/{mobile,desktop}/`）は family 内で統一できる platform-bound capability（audio 等）を `cfg(target_os)` ビルド時 dispatch の単一 facade で供給する（ランタイム dispatch ではない）。**leaf**（`platform/{web, mobile/<os>, desktop/<os>}`）は完全に platform 固有な glue（surface 生成・raw event 配線・`ImeBridge` 実装・capability の platform 実装）だけを持ち、アダプタ間で windowing/event-loop は共有しない（ADR-0087/0114 維持）。capability の契約は常に Core 所有（`ImeBridge`/`Surface`/`FontFetcher` と同型）、共通 API への昇格は原則2実装から・trait は先置きしない。これは ADR-0014「Platform Adapter の責務は IME/clipboard/raw 入力の三つ」の閉じたリストを reopen し、capability を leaf/Family Adapter の責務クラスとして追加する。
**出典:** ADR-0117（adapter-core-seam）、ADR-0068/0069/0113（Core 所有パターン）、ADR-0014（reopen）
**状況:** ✅ — `crates/adapters/{web,android,ios}` フラット構成 → `crates/platform/{web, mobile/{android,ios}, desktop}` へ再編済み（ADR-0117、`crates/platform/{common,mobile,desktop}`）。`hayate-adapter-mobile`（cfg facade）新設、`desktop` は枠（leaf 0・PLAT-10）。surface_lifecycle/touch_input/IME 増分の Core hoist は ADR-0117 の方針に沿って進行。今 trait を切る capability は mobile audio（android+ios 2 実装確定）＋ ADR-0119 の wave-1（PLAT-11）。
**備考:** 個々の capability trait は実装時に Core へ足す（空 trait を先置きしない）。desktop の枠（ディレクトリ + grouping doctrine）のみ前払い（ADR-0012 で desktop 確定ターゲット・ADR-0068 前払い条件）。

### PLAT-10 — Desktop を winit 単一 crate で着手する（windowing 層で per-OS leaf を collapse）
**規範文:** Desktop family の最初の Platform Adapter leaf を、windowing / event-loop / GPU surface を `winit` 単一 crate `hayate-platform-desktop` に畳んで置く（macos/windows/linux を windowing 層で 1 crate に統合）。Surface は vello/wgpu（`Backend = wgpu 唯一`に一致する native primary の本番経路。tiny-skia/CPU は確認用で本番 Surface には据えない）。フレーム駆動は winit event loop が App Host を構築し `request_redraw`→`RedrawRequested`→`tick(timestamp_ms)`（REND-13）。入力は winit 抽象→既存 Core seam への glue に徹する — pointer（`CursorMoved`/`MouseInput`→`on_pointer_*`、`PointerKind=Mouse`）、keyboard（`KeyboardInput`→desktop keymap→`apply_edit_intent`、web `edit_keymap` を雛形とする2実装目）、IME（`Ime::{Enabled,Preedit,Commit,Disabled}`→`ImeCommand`→`apply_ime_action`）、resize/HiDPI（物理サイズ・`scale_factor`→`ViewportMetrics::from_physical_size`）。per-OS leaf 分割と native IME（TSF/TSM/IBus）は後続フェーズに遅延する。winit が共有するのは **desktop family 内** の windowing であり web/mobile/desktop を跨ぐ共有ではないため、ADR-0087/0114（windowing 非共有）と矛盾しない。
**出典:** ADR-0118（ADR-0117 の per-OS leaf 像を windowing 層で当面 collapse）、ADR-0012（native primary）
**状況:** ⬜（設計確定・実装は未着手） — ADR-0118 で winit 単一 crate の初手 leaf（windowing/入力フル配線 + vello surface + 共有 "Tasks" demo fixture）着手が決定。`crates/platform/desktop` は枠（README）の段階で、`hayate-platform-desktop` crate・winit 依存は未追加（コードなし）。
**備考:** desktop keymap が web に次ぐ2実装目となり、将来の keymap 昇格（Core/common）の前払い prior art になる。受容するリスク: winit を desktop windowing 機構として前払い採用するため、後で native windowing が必要になれば Platform Front を書き換える可能性（ADR-0118）。

### PLAT-11 — モバイル capability は breadth-first scaffold で型として先に存在させる
**規範文:** モバイル共通 API は Flutter federated plugin の `*_platform_interface` モデルで breadth-first に scaffold する。各 capability を「Core trait ＋ android/ios 両 leaf stub ＋ mobile facade ＋ typed エラー」で先に型として存在させ、未実装は `Result<T, CapabilityError>` の `Err(Unimplemented)` を返す（panic 禁止）。これは PLAT-09 の grouping doctrine「昇格は2実装から」を「android+ios の両 leaf stub を同時に置き、契約形を Flutter `platform_interface`（複数 platform の variation を織り込んだ prior art）から取る」ことでゲートの意図（1 platform 決め打ちで契約形を誤るリスクの回避）を満たす形に読み替える。RN（TurboModule）の core module 一覧はカタログ網羅性のクロスチェックにのみ使い、bridge/channel のランタイム機構は借りない。
**出典:** ADR-0119（ADR-0117 grouping doctrine 下）
**状況:** 🟡 — wave-1 の 9 capability（biometric / haptics / file_picker / share / url_launcher / secure_storage / key_value_store / local_notification / device_info）を `crates/core/src/{biometric,haptics,file_picker,share,url_launcher,secure_storage,key_value_store,local_notification,device_info,capability}.rs` の Core trait + `platform/mobile/{android,ios}/src/capability_stubs.rs` の両 leaf stub + `platform/mobile/src/lib.rs` の facade + `Err(Unimplemented)` で scaffold 済み（`platform/mobile/tests/capability_scaffold_facade.rs`）。実機実装は未着手で、契約の最終形は実装で確定する（明示的に受容するリスク）。
**備考:** 「概形を完璧に設計」ではなく「網羅的・型付き・呼べば typed エラーを返す」が目標（ADR-0119）。鉄則1（契約は Core）・鉄則3（機構は借りない）は不変。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 5 | PLAT-01, PLAT-02, PLAT-03, PLAT-05, PLAT-09 |
| 🟡部分 | 4 | PLAT-04, PLAT-06, PLAT-08, PLAT-11 |
| ⬜未実装 | 2 | PLAT-07, PLAT-10 |
