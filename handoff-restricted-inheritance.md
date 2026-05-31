## タスク: Hayate コアに制限スタイル継承を実装する

ADR-0047（`Hayate/docs/adr/0047-restricted-style-inheritance.md`）の設計に従い実装してください。ブランチ: `claude/css-restricted-inheritance-SY0A5`

### 設計概要

テキスト系プロパティ（`color` / `font-size` / `font-family`）に限定した継承を Hayate コアの `scene_build` レイヤーで実装する。`scene_build` の `walk()` に `InheritedStyle` 構造体をトップダウンで渡し、各 element は `Option<T>` の明示値フィールドを持ち、`None` の場合は親から引き継いだ値を使う。全 element kind（view / scroll-view 等）を通過・上書きできる。

### 変更ファイルと内容

#### `Hayate/crates/core/src/element/style.rs`

`StylePropKind` enum を新設する:

```rust
pub enum StylePropKind {
    Color,
    FontSize,
    FontFamily,
}
```

#### `Hayate/crates/core/src/element/tree.rs`

`Visual` 構造体の変更:
- `text_color: Color` → `text_color: Option<Color>`
- `font_size: f32` → `font_size: Option<f32>`

`Visual::default()` の変更:
- `text_color: None`、`font_size: None` に変更（デフォルト値は `InheritedStyle::default()` へ移す）

`ResolvedElement` 構造体の変更:
- `text_color: Color` → `text_color: Option<Color>`
- `font_size: f32` → `font_size: Option<f32>`

`apply_visual` 関数の変更:
- `StyleProp::Color(c)` → `visual.text_color = Some(*c)`
- `StyleProp::FontSize(v)` → `visual.font_size = Some(*v)`

`element_unset_style` メソッドを新設:

```rust
pub fn element_unset_style(&mut self, id: ElementId, kinds: &[StylePropKind]) {
    if let Some(el) = self.elements.get_mut(&id) {
        for kind in kinds {
            match kind {
                StylePropKind::Color => el.visual.text_color = None,
                StylePropKind::FontSize => {
                    el.visual.font_size = None;
                    el.text_layout = None;
                    let _ = self.taffy.mark_dirty(el.taffy_node);
                }
                StylePropKind::FontFamily => {
                    el.visual.font_family = None;
                    el.text_layout = None;
                    let _ = self.taffy.mark_dirty(el.taffy_node);
                }
            }
        }
    }
}
```

#### `Hayate/crates/core/src/element/scene_build.rs`

`InheritedStyle` 構造体を新設:

```rust
#[derive(Clone)]
struct InheritedStyle {
    color: Color,
    font_size: f32,
    font_family: Option<String>,
}

impl Default for InheritedStyle {
    fn default() -> Self {
        Self { color: Color::BLACK, font_size: 16.0, font_family: None }
    }
}
```

`build()` 関数の変更:

```rust
pub fn build(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    if let Some(root) = tree.root() {
        walk(tree, root, 0.0, 0.0, &mut sg, None, InheritedStyle::default());
    }
    sg
}
```

`walk()` 関数のシグネチャ変更と継承解決ロジック追加:

```rust
fn walk(
    tree: &ElementTree,
    id: ElementId,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    inherited: InheritedStyle,
) {
    // ...既存の el, layout 取得処理...

    // 継承を解決して確定値を求める
    let color = el.visual.text_color.unwrap_or(inherited.color);
    let font_size = el.visual.font_size.unwrap_or(inherited.font_size);
    let font_family = el.visual.font_family.clone().or_else(|| inherited.font_family.clone());

    // 子への継承コンテキストを構築
    let next_inherited = InheritedStyle {
        color,
        font_size,
        font_family: font_family.clone(),
    };

    // ...既存の SceneGraph ノード構築処理（color / font_size / font_family を使う）...

    // 子を走査
    for child_id in &el.children {
        walk(tree, *child_id, ..., next_inherited.clone());
    }
}
```

`walk()` 内で `el.visual.text_color` / `el.visual.font_size` を直接参照している箇所をすべて `color` / `font_size` に置き換える。

#### `Hayate/crates/adapters/web/src/element_renderer.rs`（HTML Mode）

`ResolvedElement.text_color` が `Option<Color>` になるため:
- `Some(color)` のときのみ CSS プロパティを設定する
- `None` のときは CSS プロパティを設定しない（ブラウザの CSS 継承に委ねる）

`font_size` も同様に `Option<f32>` を処理する。

#### WIT ファイル（`Hayate/` 以下の `*.wit`）

`style-prop-kind` 型と `element-unset-style` 関数を追加する。

### 完了条件

- `cargo build` が通ること
- `cargo test` が通ること
- 変更をコミットして `claude/css-restricted-inheritance-SY0A5` にプッシュすること
