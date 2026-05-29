# Hayate 残課題リスト

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
設計確定済み（2026-05-30）。実装順序は ADR-0041 参照。

**ネイティブ対応（優先）:**
- WIT に `poll_accessibility()` を追加（オンデマンド、フレームコストゼロ）
- WIT に role enum 14種を定義（`generic-container` / `button` / `label` / `text-input` / `scroll-view` / `image` / `list` / `list-item` / `heading` / `link` / `navigation` / `main` / `dialog` / `alert-dialog`）
- Element Tree を walk し、`layout_cache` から境界矩形を補完して `accesskit::TreeUpdate` を生成
- `text` / `text-input` のテキスト詳細は Parley の `LayoutAccessibility::build_nodes` を呼ぶ
- `ElementId` の `u64` キャストで AccessKit `NodeId` に直接変換（マッピングテーブル不要）
- 返り値は WIT record のフラットリスト
- Platform Adapter が `TreeUpdate` を受け取り各プラットフォームの AT（UIA / NSAccessibility / AT-SPI）に報告

**Web HTML Mode 対応（ネイティブ後）:**
- 実 DOM に ARIA 属性を付与する形で対応

**Web Canvas Mode 対応（Safari が EditContext API を正式サポートした時点で最優先）:**
- `accesskit-web` クレートを使い `<canvas>` 隣に不可視 ARIA DOM を生成

---

## P3 — アーキテクチャ改善

### 10. `on_pointer_move` のヒットテスト負荷
- 毎フレーム呼ばれる可能性があるため、layout_cache が空のときは skip する guard が必要
- `if self.tree.layout_cache.is_empty() { return; }` を各 on_pointer_* の先頭に追加

### 11. `flush_remove` の adapter 層回帰テスト未整備
- **ファイル**: `crates/adapters/web/src/element_renderer.rs`
- Core 側 (`element_remove`) の focused_element クリアは `remove_clears_focused_element` でカバー済み
- Canvas Mode の `remove_subtree`（`is_in_subtree` 走査）および HTML Mode の `flush_remove`（`!nodes.contains_key` チェック）は wasm-bindgen-test または E2E なしにテストできない
- 回帰テストは wasm-pack test --headless もしくは Playwright E2E が必要

---

## Tsubame 実装準備（ブロッカー順）

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

テスト: 34件すべて通過（`cargo test --package hayate-core`）
