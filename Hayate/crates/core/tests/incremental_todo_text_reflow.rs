//! 回帰: todo を 1 件追加すると、タイトル文字が min-content（1 単語/行）でシェイプされ、
//! `maxLines:1` クランプで先頭単語＋… に切り詰められる不具合の再現。
//!
//! SolidJS の `visible().map(...)`（キーなし）は、追加のたびにリスト配下を**全破棄＋再構築**
//! する。実測した RecordingRenderer のop列（renderer-protocol）はまさにこの形:
//!   - 新規行を全件 createElement + setText + appendChild
//!   - 旧行を element_remove
//! を 1 バッチ（`apply_mutations`）で適用 → 1 回 settle/render。
//!
//! タイトルボタンは `overflow:hidden + maxLines:1 + textOverflow:ellipsis`（ui/styles.ts の
//! `titleStyle`）。box幅は十分なのにグリフが min-content で折れると、クランプで先頭単語＋…
//! に化け、ユーザーには「文字描画がバグる／追加が反映されない」と見える。DOM レンダラーは
//! ブラウザ flex で正しく描くので Canvas（Hayate）も一致しなければならない。

use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementId, ElementKind, ElementTree,
    FlexDirectionValue, OverflowValue, StyleProp, TextOverflowValue,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

struct Studio {
    tree: ElementTree,
    next: u64,
    list: ElementId,
}

impl Studio {
    fn mk(&mut self, kind: ElementKind, style: &[StyleProp]) -> ElementId {
        let id = self.tree.element_create(self.next, kind);
        self.next += 1;
        self.tree.element_set_style(id, style);
        id
    }

    /// todo 1 行（App の TodoRow を実測 op 列どおりに再現）:
    /// row[align:center] > col[flexGrow:1] > button[titleStyle] > text。
    /// タイトル text 要素の id を返す。`list` 末尾へ append する（実測の add op 列と同じ）。
    fn add_row(&mut self, text: &str) -> ElementId {
        let row = self.mk(
            ElementKind::View,
            &[
                StyleProp::Display(DisplayValue::Flex),
                StyleProp::FlexDirection(FlexDirectionValue::Row),
                StyleProp::AlignItems(AlignValue::Center),
                StyleProp::Gap(Dimension::px(12.0)),
                StyleProp::Padding(Dimension::px(12.0)),
            ],
        );
        let col = self.mk(
            ElementKind::View,
            &[
                StyleProp::FlexGrow(1.0),
                StyleProp::Display(DisplayValue::Flex),
                StyleProp::FlexDirection(FlexDirectionValue::Column),
            ],
        );
        self.tree.element_append_child(row, col);
        // titleStyle: overflow:hidden + maxLines:1 + textOverflow:ellipsis。
        let title_btn = self.mk(
            ElementKind::Button,
            &[
                StyleProp::Display(DisplayValue::Flex),
                StyleProp::AlignItems(AlignValue::Center),
                StyleProp::DefaultFontSize(15.0),
                StyleProp::Overflow(OverflowValue::Hidden),
                StyleProp::MaxLines(1),
                StyleProp::TextOverflow(TextOverflowValue::Ellipsis),
            ],
        );
        self.tree.element_append_child(col, title_btn);
        let title = self.mk(ElementKind::Text, &[StyleProp::DefaultFontSize(15.0)]);
        self.tree.element_set_text(title, text);
        self.tree.element_append_child(title_btn, title);
        self.tree.element_append_child(self.list, row);
        title
    }

    /// その行（タイトル text を起点に row まで遡って）を破棄する。
    fn remove_row_of(&mut self, title: ElementId) {
        // title -> button -> col -> row。row は list の直接の子。
        let row = self
            .tree
            .ordered_children(self.list)
            .into_iter()
            .find(|&r| {
                // row > col > button > title
                self.tree.ordered_children(r).iter().any(|&col| {
                    self.tree
                        .ordered_children(col)
                        .iter()
                        .any(|&btn| self.tree.ordered_children(btn).contains(&title))
                })
            })
            .expect("row containing title");
        self.tree.element_remove(row);
    }
}

fn studio() -> Studio {
    let mut tree = ElementTree::new();
    tree.test_set_wasm_like_fonts(FONT.to_vec());
    let ink = Color::new(0.1, 0.1, 0.12, 1.0);

    let mut next = 1u64;
    let mut mk = |t: &mut ElementTree, k: ElementKind, s: &[StyleProp]| {
        let id = t.element_create(next, k);
        next += 1;
        t.element_set_style(id, s);
        id
    };

    let shell = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::DefaultColor(ink),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(shell);
    tree.set_viewport(900.0, 720.0);

    let card = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(620.0)),
            StyleProp::MaxWidth(Dimension::percent(100.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(16.0)),
            StyleProp::Padding(Dimension::px(22.0)),
        ],
    );
    tree.element_append_child(shell, card);

    let list = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(8.0)),
        ],
    );
    tree.element_append_child(card, list);

    Studio { tree, next, list }
}

/// box幅が十分なら、`maxLines:1` でもタイトルは丸ごと 1 行に収まる（切り詰めない）。
fn assert_title_intact(tree: &ElementTree, title: ElementId, full: &str, label: &str) {
    let lines = tree.test_text_line_count(title);
    let shaped = tree.test_shaped_text(title);
    assert_eq!(
        lines,
        Some(1),
        "{label}: タイトルは 1 行（line_count）。shaped={shaped:?}"
    );
    assert_eq!(
        shaped.as_deref(),
        Some(full),
        "{label}: タイトルが min-content 折返し→クランプで切り詰められた"
    );
}

/// フォント読み込み（`fonts_dirty`）後の全テキスト再シェイプと、続けての todo 追加が
/// 共存しても、タイトルが min-content で折れないこと。実環境では起動後に名前付きフォント
/// （Inter / Segoe UI）が非同期ロードされ `fonts_dirty` で全テキストを reshape する。
/// この全 reshape と差分追加が重なる経路を、フォント供給を差し替えて模す。
#[test]
fn font_reload_then_add_keeps_titles_intact() {
    const A: &str = "layout engine flex wrap";
    const B: &str = "verify box shadow draw";
    const C: &str = "added task xyz one";

    let mut s = studio();
    let a1 = s.add_row(A);
    let b1 = s.add_row(B);
    s.tree.render(0.0);
    assert_title_intact(&s.tree, a1, A, "初期 行A");

    // 名前付きフォントのロード相当: コレクション差し替え + fonts_dirty → 全テキスト reshape。
    s.tree.test_set_wasm_like_fonts(FONT.to_vec());
    s.tree.render(16.0);
    assert_title_intact(&s.tree, a1, A, "フォント reload 後 行A");
    assert_title_intact(&s.tree, b1, B, "フォント reload 後 行B");

    // reshape 直後に追加（全破棄＋再構築）。
    let c2 = s.add_row(C);
    let a2 = s.add_row(A);
    let b2 = s.add_row(B);
    s.remove_row_of(a1);
    s.remove_row_of(b1);
    s.tree.render(32.0);
    assert_title_intact(&s.tree, c2, C, "reload→追加 新規行C");
    assert_title_intact(&s.tree, a2, A, "reload→追加 行A");
    assert_title_intact(&s.tree, b2, B, "reload→追加 行B");
}

#[test]
fn adding_a_todo_keeps_existing_and_new_titles_intact() {
    const A: &str = "layout engine flex wrap";
    const B: &str = "verify box shadow draw";
    const C: &str = "added task xyz one";

    let mut s = studio();
    // 初期 2 行（seed 相当）。
    let a1 = s.add_row(A);
    let b1 = s.add_row(B);
    s.tree.render(0.0);
    assert_title_intact(&s.tree, a1, A, "初期 行A");
    assert_title_intact(&s.tree, b1, B, "初期 行B");

    // === 実測の add op 列（全破棄＋再構築）を 1 バッチで適用 ===
    // 新規 3 行を作成（append）。
    let c2 = s.add_row(C);
    let a2 = s.add_row(A);
    let b2 = s.add_row(B);
    // 旧 2 行を破棄。
    s.remove_row_of(a1);
    s.remove_row_of(b1);
    // 1 回 render（settle）。
    s.tree.render(16.0);

    assert_title_intact(&s.tree, c2, C, "追加後 新規行C");
    assert_title_intact(&s.tree, a2, A, "追加後 行A");
    assert_title_intact(&s.tree, b2, B, "追加後 行B");
}
