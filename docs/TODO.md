# Hayate 残課題リスト

Phase 1〜5 完了後の未実装項目。優先度順。

---

## ✅ 完了済み（P0・P1）

### ✅ 1. `on_pointer_up` / `on_pointer_move` 実装済み
- `on_pointer_up` → `Event::PointerUp` を emit
- `on_pointer_move` → `hovered_element` を追跡し `Event::PointerEnter` / `Event::PointerLeave` を emit
- `encode_events` で pointer_up(9) / pointer_enter(10) / pointer_leave(11) を JS に送出

### ✅ 2. ScrollView スクロール上限クランプ実装済み
- `on_wheel()` で `element_content_size()` を取得し `(ox + delta).clamp(0.0, max)` を適用

### ✅ 3. TextInput カーソル描画（Canvas モード）実装済み
- `crates/core/src/element/scene_build.rs` にカーソル描画ロジック実装済み
- `element_set_cursor_visible(id, bool)` で on/off 制御
- Parley `Cursor::from_byte_index` から `geometry()` でピクセル位置を取得して Rect ノードを emit

### ✅ 4. キーボードイベント実装済み
- `on_key_down(key: &str, modifiers: u32)` を両レンダラーに追加
- `Backspace`: 最後の文字を削除
- `Enter`: TextInput に `\n` を挿入
- `modifiers` bitmask: `modifier_shift()/ctrl()/alt()/meta()` 定数を JS に公開
- `Event::KeyDown { target, key, modifiers }` に `modifiers` フィールドを追加
- `encode_events` で `[12, target_ffi, modifiers]` に更新

### ✅ 5. PNG 以外の画像フォーマット対応済み
- `fetch_image()` が `image` クレートで PNG / JPEG / WebP を自動判別してデコード

### ✅ カーソル点滅
- `HayateElementRenderer::tick_cursor(timestamp_ms: f64)` を追加
- JS の `requestAnimationFrame` から呼ぶことで 500ms 周期で点滅

---

## P2 — 品質向上

---

## P2 — 品質向上

### 6. クリップボード未実装
- `on_paste(text: &str)` → TextInput の `text_content` に追記
- `element_get_text_content(id)` で取得した値を JS 側が clipboard に書く
- WIT に `paste-event` を追加するか JS 側で完結させるかを選択

### 7. フォントカスタム読み込み未実装
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
- `load_font(data: &[u8])` を追加し、`tree.font_cx` に `FontContext::collection_mut().add_font_bytes()` で登録
- Parley の FontContext は `ElementTree` が保持しているため、adapter からアクセスするヘルパーが必要

### 8. アクセシビリティツリー（accesskit）未実装
- Parley は accesskit feature フラグあり（`crates/vendor/parley/`）
- `NodeKind::TextRun` に対応する accesskit Node を SceneGraph から生成する設計が必要
- 優先度は低いが、本番利用には必要

---

## P3 — アーキテクチャ改善

### 10. `on_pointer_move` のヒットテスト負荷
- 毎フレーム呼ばれる可能性があるため、layout_cache が空のときは skip する guard が必要
- `if self.tree.layout_cache.is_empty() { return; }` を各 on_pointer_* の先頭に追加

### 11. `flush_remove` が子孫の `hovered_element` / `active_element` をクリアしない
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
  - Canvas Mode: `flush_pending` の `Command::Remove` 分岐（L498-507 付近）
  - HTML Mode: `flush_remove`（L1190-1198 付近）
- 削除対象の id 自身しか比較していない。`element_remove` はサブツリー全体を削除するので、子孫を hover/active 中の状態でその祖先を削除すると、`hovered_element` / `active_element` が dangling な `ElementId` を保持し続ける。
- `focused_element` は `ElementTree::element_remove` 側で全子孫を walk して clear しているので問題なし。アダプタ側でも同じ走査が必要。
- 影響: 次の `on_pointer_up` が存在しない要素に `ActiveEnd` を emit する、`on_pointer_move` の hover 遷移ロジックが過去フレームの dangling id と比較し続ける、など。
- 検出: 現状のテストはネイティブ側 ElementTree のみで wasm アダプタを覆っていない。回帰テストは wasm-bindgen-test もしくは E2E が必要。

---

---

## Tsubame 実装準備（ブロッカー順）

### ✅ T1. `apply_mutations` バッチエンコーディング仕様の確定【設計】
- ADR-0039 に仕様を記録。`apply_mutations(ops: Float64Array, styles: Float32Array)` の 2 引数形式、固定長レコード、不明 op_kind は Err 返却。

### T2. `apply_mutations` の実装【Hayate 側】
- **ファイル**: `crates/adapters/web/src/element_renderer.rs` L446–450
- T1 の仕様確定後に実装する
- batch 配列をパースして既存の `element_create` / `element_set_style` 等を呼び出す

### T3. `flush_remove` の dangling `hovered_element` / `active_element` バグ修正【Hayate 側】
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
  - Canvas Mode: `flush_pending` の `Command::Remove` 分岐（L498 付近）
  - HTML Mode: `flush_remove`（L1190 付近）
- Tsubame が element を動的削除すると即座に問題になる（hover/active 中の子孫を削除したとき dangling ElementId が残る）
- 修正: 削除 Element のサブツリーを走査して `hovered_element` / `active_element` をクリアする（`focused_element` 側の処理と同様）

### T4. `on_pointer_move` の空 layout_cache ガード追加【Hayate 側】
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
- `on_pointer_move` 先頭に `if self.tree.layout_cache.is_empty() { return; }` を追加
- Tsubame が毎フレーム apply_mutations → render → poll_events を回す前に入れる

### T5. WASM バインディング動作確認【検証】
- `wasm-pack build` 後に生成される JS バインディングで `apply_mutations` の引数型が JS から自然に扱えるか確認
- 必要であれば `.d.ts` を手動補完

### T6. 最小デモスケルトン整備【任意】
- `examples/web-demo/` に Tsubame Canvas Mode から `apply_mutations` を呼ぶ Hello World を追加
- 実装中の動作確認サイクルを短縮するため

---

## 実装済みフェーズ（参考）

| Phase | 内容 | コミット |
|-------|------|---------|
| 1 | Event System（Click/Focus/Blur/Scroll/Resize）| feat(event): Phase 1 |
| 2a | ZIndex | feat(style): Phase 2a |
| 2b | Transform / Group / Clip ノード | feat(render): Phase 2b |
| 3 | ScrollView クリッピング＋オフセット | feat(scroll): Phase 3 |
| 4 | Image（PNG fetch + Vello描画）| feat(image): Phase 4 |
| 5 | TextInput + IME composition | feat(text-input): Phase 5 |

テスト: 19件すべて通過（`cargo test --package hayate-core`）
