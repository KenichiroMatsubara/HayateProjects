# Hayate 残課題リスト

Phase 1〜5 完了後の未実装項目。優先度順。

---

## P0 — 即修正（バグ・空実装）

### 1. `on_pointer_up` / `on_pointer_move` が空実装
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
- `on_pointer_up` → `active-end` イベントを emit する
- `on_pointer_move` → `hovered_element` を追跡し `hover-enter` / `hover-leave` を emit する
- WIT に `hover-enter(element-id)` / `hover-leave(element-id)` / `active-start(element-id)` / `active-end(element-id)` を追加（Phase 1b 相当）

### 2. ScrollView スクロール上限クランプなし
- **ファイル**: `crates/adapters/web/src/element_renderer.rs` `on_wheel()`
- コンテンツ高さ（子要素の合計）を超えてスクロールできてしまう
- `element_set_scroll_offset` 側でもクランプが必要
- コンテンツサイズを `Element.scroll_content_size: (f32, f32)` に記録し、layout 後に更新する

---

## P1 — 機能完成に必須

### 3. TextInput カーソル描画（Canvas モード）
- **ファイル**: `crates/core/src/element/scene_build.rs`
- TextInput がフォーカスされているとき、カーソル位置に細い Rect ノードを追加描画する
- カーソル位置は Parley `PlainEditor` から取得可能（`selection.focus().index()`）
- フォーカス状態は adapter から core に `element_set_focused(id, bool)` で通知する設計が必要

### 4. キーボードイベント未実装
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
- `on_key_down(key: &str, modifiers: u32)` を両レンダラーに追加
- 最低限: `Backspace`（TextInput の最後の文字削除）、`Enter`
- WIT に `key-down` イベントを追加するか、JS 側で on_text_input に変換するかを決定する

### 5. PNG 以外の画像フォーマット未対応
- **ファイル**: `crates/adapters/web/src/element_renderer.rs` `fetch_png()`
- 関数名を `fetch_image()` に変更し、Content-Type ヘッダを見て分岐する
- JPEG: `jpeg-decoder` クレートを追加（または `image` クレートで統一）
- WebP: `image` クレート（`image = { features = ["webp"] }`）

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

### 9. イベント f64 エンコードに文字列を持てない
- **現状**: TextInput/Composition イベントは `[tag, target_ffi]` のみ（テキストなし）
- JS は `element_get_text_content(id)` で取得する運用
- **改善案**: `poll_text_events() -> String`（JSON 配列）を追加し、文字列ペイロードを返す

### 10. `on_pointer_move` のヒットテスト負荷
- 毎フレーム呼ばれる可能性があるため、layout_cache が空のときは skip する guard が必要
- `if self.tree.layout_cache.is_empty() { return; }` を各 on_pointer_* の先頭に追加

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
