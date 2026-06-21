//! Tsubame hello-world の "Tasks" 画面を tiny-skia で描画し、Canvas Mode の出力を
//! DOM Mode（ブラウザ）描画と目視比較するためのテスト。
//!
//! `HAYATE_WRITE_SCREENSHOT=1` を付けたときだけ PNG を出力し、それ以外は no-op なので
//! CI をブロックしない。フィクスチャは `Tsubame/examples/hello-world/src/App.tsx`
//! （ライトテーマ、teal アクセント）を、レイアウト/テキスト/スタイルの差異が出る程度に再現する。

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, ElementId, ElementKind, ElementTree,
    FlexDirectionValue, JustifyValue, PseudoState, Shadow, StyleProp,
};
use hayate_scene_renderer_tiny_skia::{premultiplied_to_straight, TinySkiaSceneRenderer};
use tiny_skia::Pixmap;

static NOTO_SANS_JP_BYTES: &[u8] =
    include_bytes!("../../../core/assets/fonts/NotoSansJP.ttf");

const VW: f32 = 980.0;
const VH: f32 = 1060.0;

/// `#rgb`, `#rrggbb`, `#rrggbbaa` を core の `Color` にパースする。
fn hex(s: &str) -> Color {
    let h = s.trim_start_matches('#');
    let n = |a: usize, b: usize| u8::from_str_radix(&h[a..b], 16).unwrap() as f64 / 255.0;
    match h.len() {
        6 => Color::new(n(0, 2), n(2, 4), n(4, 6), 1.0),
        8 => Color::new(n(0, 2), n(2, 4), n(4, 6), n(6, 8)),
        _ => panic!("bad hex {s}"),
    }
}

/// `theme.ts` から写したライトテーマ + teal アクセントのパレット。
struct P;
impl P {
    fn bg(&self) -> Color { hex("#f1ede3") }
    fn rail(&self) -> Color { hex("#fbf8f1") }
    fn panel(&self) -> Color { hex("#fdfdfb") }
    fn panel2(&self) -> Color { hex("#ece6d8") }
    fn panel3(&self) -> Color { hex("#e0d8c7") }
    fn ink(&self) -> Color { hex("#262130") }
    fn text(&self) -> Color { hex("#322c3f") }
    fn muted(&self) -> Color { hex("#6f6878") }
    fn quiet(&self) -> Color { hex("#9a93a3") }
    fn line(&self) -> Color { hex("#d9d3c6") }
    fn accent(&self) -> Color { hex("#14b8a6") }
    fn danger(&self) -> Color { hex("#e5484d") }
    fn success(&self) -> Color { hex("#2fa86a") }
    fn blue(&self) -> Color { hex("#4b8ef0") }
    fn black(&self) -> Color { hex("#14101c") }
    fn shadow(&self) -> Color { hex("#2621301f") }
}

fn prio_tone(p: &P, prio: u8) -> Color {
    match prio {
        3 => p.danger(),
        2 => hex("#ef9d2e"), // accent2
        _ => p.blue(),
    }
}

const PRIO_LABEL: [&str; 4] = ["", "低", "中", "高"];

/// todo example の削除コントロール（各行ボタン + フッターヒント）用グリフ。
/// 同梱 Canvas フォント（NotoSansJP.ttf）にアウトラインが存在する必要がある。
/// U+2715 ✕ は欠落していて Canvas が `.notdef`（0 ink）にフォールバックするが、U+00D7 × は描ける。
/// `Tsubame/examples/todo` と揃える。
const DELETE_GLYPH: &str = "×";

struct B {
    tree: ElementTree,
    next: u64,
}

impl B {
    fn new() -> Self {
        let mut tree = ElementTree::new();
        tree.register_font("Noto Sans", NOTO_SANS_JP_BYTES.to_vec());
        // アプリは Inter/Segoe/system-ui を要求する。同梱フェースをそれらの名前でも
        // 登録し、フォント欠落フォールバックが本物の不具合を隠さないようにする。
        tree.register_font("Inter", NOTO_SANS_JP_BYTES.to_vec());
        Self { tree, next: 1 }
    }

    fn el(&mut self, kind: ElementKind, styles: &[StyleProp]) -> ElementId {
        let id = self.next;
        self.next += 1;
        let e = self.tree.element_create(id, kind);
        self.tree.element_set_style(e, styles);
        e
    }

    fn view(&mut self, styles: &[StyleProp]) -> ElementId {
        self.el(ElementKind::View, styles)
    }

    fn text(&mut self, s: &str, styles: &[StyleProp]) -> ElementId {
        let e = self.el(ElementKind::Text, styles);
        self.tree.element_set_text(e, s);
        e
    }

    /// tsubame-solid のボタン構築を再現する。Button コンテナに子の Text ノードを持つ（ADR-0058）。
    /// ラベルの色/サイズは Button の `DefaultColor` / `DefaultFontSize` として渡し、
    /// 子テキストが継承する（アプリの `defaultColor` 用法に一致）。
    fn button(&mut self, s: &str, styles: &[StyleProp]) -> ElementId {
        let mut container = Vec::new();
        for st in styles {
            match st {
                StyleProp::Color(c) => container.push(StyleProp::DefaultColor(*c)),
                StyleProp::FontSize(v) => container.push(StyleProp::DefaultFontSize(*v)),
                other => container.push(other.clone()),
            }
        }
        let btn = self.el(ElementKind::Button, &container);
        let label = self.el(ElementKind::Text, &[]);
        self.tree.element_set_text(label, s);
        self.tree.element_append_child(btn, label);
        btn
    }

    fn child(&mut self, parent: ElementId, child: ElementId) {
        self.tree.element_append_child(parent, child);
    }

    fn children(&mut self, parent: ElementId, kids: &[ElementId]) {
        for &k in kids {
            self.tree.element_append_child(parent, k);
        }
    }
}

fn row(gap: f32) -> Vec<StyleProp> {
    vec![
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::Gap(Dimension::px(gap)),
    ]
}

fn col(gap: f32) -> Vec<StyleProp> {
    vec![
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(gap)),
    ]
}

#[test]
fn render_tasks_screen() {
    if std::env::var_os("HAYATE_WRITE_SCREENSHOT").is_none() {
        return;
    }
    let p = P;
    let mut b = B::new();

    // ルート列
    let root = b.view(&[
        StyleProp::Width(Dimension::px(VW)),
        StyleProp::Height(Dimension::px(VH)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::BackgroundColor(p.bg()),
        StyleProp::DefaultColor(p.text()),
        StyleProp::DefaultFontSize(14.0),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(VW, VH);

    // ── AppBar ──────────────────────────────────────────────────────────────
    let appbar = b.view(&[
        StyleProp::Height(Dimension::px(64.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::SpaceBetween),
        StyleProp::BackgroundColor(p.rail()),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::PaddingLeft(Dimension::px(24.0)),
        StyleProp::PaddingRight(Dimension::px(24.0)),
    ]);
    // ブランド
    let brand = b.view(&row(12.0));
    let logo = b.view(&[
        StyleProp::Width(Dimension::px(38.0)),
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.accent()),
        StyleProp::BorderRadius(12.0),
    ]);
    let logo_t = b.text("TS", &[StyleProp::FontSize(18.0), StyleProp::Color(p.black())]);
    b.child(logo, logo_t);
    let titles = b.view(&col(2.0));
    let t1 = b.text("Tsubame Task Studio", &[StyleProp::FontSize(20.0), StyleProp::Color(p.ink())]);
    let t2 = b.text("POP TODO + Hayate CSS gallery", &[StyleProp::FontSize(12.0), StyleProp::Color(p.muted())]);
    b.children(titles, &[t1, t2]);
    b.children(brand, &[logo, titles]);

    // 右クラスタ
    let right = b.view(&row(10.0));
    let tab_tasks = b.button("Tasks", &[
        StyleProp::Height(Dimension::px(34.0)),
        StyleProp::PaddingLeft(Dimension::px(16.0)),
        StyleProp::PaddingRight(Dimension::px(16.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.accent()),
        StyleProp::Color(p.black()),
        StyleProp::BorderRadius(10.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.accent()),
        StyleProp::FontSize(13.0),
    ]);
    let tab_gallery = b.button("CSS Gallery", &[
        StyleProp::Height(Dimension::px(34.0)),
        StyleProp::PaddingLeft(Dimension::px(16.0)),
        StyleProp::PaddingRight(Dimension::px(16.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.panel()),
        StyleProp::Color(p.text()),
        StyleProp::BorderRadius(10.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(13.0),
    ]);
    // アクセント色スウォッチ
    let swatches = b.view(&row(6.0));
    for (i, c) in ["#14b8a6", "#e84d8a", "#ef8f3c", "#5ca80f", "#7c5cf0"].iter().enumerate() {
        let selected = i == 0;
        let sw = b.button(" ", &[
            StyleProp::Width(Dimension::px(22.0)),
            StyleProp::Height(Dimension::px(22.0)),
            StyleProp::BackgroundColor(hex(c)),
            StyleProp::BorderRadius(999.0),
            StyleProp::BorderWidth(if selected { 3.0 } else { 1.0 }),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if selected { p.ink() } else { p.line() }),
        ]);
        b.child(swatches, sw);
    }
    let theme_btn = b.button("🌙", &[
        StyleProp::Width(Dimension::px(34.0)),
        StyleProp::Height(Dimension::px(34.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.panel()),
        StyleProp::Color(p.text()),
        StyleProp::BorderRadius(10.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(15.0),
    ]);
    let rlabel = b.text("renderer", &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)]);
    let badge = b.view(&[
        StyleProp::Height(Dimension::px(28.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::Gap(Dimension::px(10.0)),
        StyleProp::BackgroundColor(p.panel()),
        StyleProp::BorderRadius(10.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
    ]);
    let badge_t = b.text("tiny-skia", &[StyleProp::Color(p.accent()), StyleProp::FontSize(13.0)]);
    let badge_t2 = b.text("tiny-skia", &[StyleProp::Color(p.muted()), StyleProp::FontSize(12.0)]);
    b.children(badge, &[badge_t, badge_t2]);
    b.children(right, &[tab_tasks, tab_gallery, swatches, theme_btn, rlabel, badge]);
    b.children(appbar, &[brand, right]);

    // ── コンテンツパネル ─────────────────────────────────────────────────────
    let stage = b.view(&[
        StyleProp::FlexGrow(1.0),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::PaddingTop(Dimension::px(28.0)),
        StyleProp::PaddingBottom(Dimension::px(28.0)),
        StyleProp::BackgroundColor(p.bg()),
    ]);
    let panel = b.view(&[
        StyleProp::Width(Dimension::px(620.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(16.0)),
        StyleProp::Padding(Dimension::px(22.0)),
        StyleProp::BackgroundColor(p.panel()),
        StyleProp::BorderRadius(18.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::BoxShadow(vec![Shadow {
            offset_x: 0.0, offset_y: 18.0, blur: 40.0, spread: -8.0,
            color: p.shadow(), inset: false,
        }]),
    ]);

    // ヘッダー
    let header = b.view(&col(12.0));
    let hrow = b.view(&[
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::SpaceBetween),
    ]);
    let htitle = b.text("きょうのタスク", &[StyleProp::Color(p.ink()), StyleProp::FontSize(24.0)]);
    let hsub = b.text("残り 3 件 / 全 5 件", &[StyleProp::Color(p.muted()), StyleProp::FontSize(13.0)]);
    b.children(hrow, &[htitle, hsub]);
    // 進捗バー（40%）
    let pbar = b.view(&[
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::px(12.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::BackgroundColor(p.black()),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
    ]);
    let pfill = b.view(&[
        StyleProp::Width(Dimension::percent(40.0)),
        StyleProp::Height(Dimension::px(8.0)),
        StyleProp::MarginLeft(Dimension::px(2.0)),
        StyleProp::BackgroundColor(p.success()),
        StyleProp::BorderRadius(6.0),
    ]);
    b.child(pbar, pfill);
    b.children(header, &[hrow, pbar]);

    // SelectableNote
    let note = b.view(&[
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(8.0)),
        StyleProp::Padding(Dimension::px(12.0)),
        StyleProp::BackgroundColor(p.panel2()),
        StyleProp::BorderRadius(12.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
    ]);
    for s in [
        "この段落は選択できます。ダブルクリックで単語、トリプルクリックで段落を選び、Shift+クリックや Shift+矢印で範囲を伸縮、Cmd/Ctrl+A で全選択できます。",
        "これは二つ目の段落です。一つ目の段落からここまでドラッグすると、block をまたいだ連続選択になります（issue #269）。",
        "Canvas Mode では core が選択ハイライトを描画し、DOM Mode ではブラウザのネイティブ選択に委ねます。",
    ] {
        let t = b.text(s, &[StyleProp::Color(p.muted()), StyleProp::FontSize(13.0)]);
        b.child(note, t);
    }

    // AddForm
    let addform = b.view(&row(8.0));
    let input = b.el(ElementKind::TextInput, &[
        StyleProp::FlexGrow(1.0),
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::BackgroundColor(p.panel2()),
        StyleProp::Color(p.text()),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(13.0),
    ]);
    b.tree.element_set_text(input, "新しいタスクを入力…");
    let segs = b.view(&row(4.0));
    for (prio, active) in [(3u8, false), (2u8, true), (1u8, false)] {
        let tone = prio_tone(&p, prio);
        let seg = b.button(PRIO_LABEL[prio as usize], &[
            StyleProp::Height(Dimension::px(38.0)),
            StyleProp::MinWidth(Dimension::px(40.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(if active { tone } else { p.panel2() }),
            StyleProp::Color(if active { p.black() } else { p.muted() }),
            StyleProp::BorderRadius(9.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if active { tone } else { p.line() }),
            StyleProp::FontSize(13.0),
        ]);
        b.child(segs, seg);
    }
    let addbtn = b.button("追加", &[
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(18.0)),
        StyleProp::PaddingRight(Dimension::px(18.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.accent()),
        StyleProp::Color(p.black()),
        StyleProp::BorderRadius(9.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.accent()),
        StyleProp::FontSize(13.0),
    ]);
    b.children(addform, &[input, segs, addbtn]);

    // Toolbar
    let toolbar = b.view(&[
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::Gap(Dimension::px(8.0)),
        StyleProp::PaddingTop(Dimension::px(10.0)),
        StyleProp::PaddingBottom(Dimension::px(10.0)),
    ]);
    let tl1 = b.text("表示", &[StyleProp::Color(p.quiet()), StyleProp::FontSize(12.0)]);
    b.child(toolbar, tl1);
    for (label, active) in [("すべて", true), ("未完了", false), ("完了済み", false)] {
        let chip = b.button(label, &[
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingRight(Dimension::px(12.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(if active { p.accent() } else { p.panel2() }),
            StyleProp::Color(if active { p.black() } else { p.text() }),
            StyleProp::BorderRadius(999.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if active { p.accent() } else { p.line() }),
            StyleProp::FontSize(12.0),
        ]);
        b.child(toolbar, chip);
    }
    let tl2 = b.text("並び", &[StyleProp::Color(p.quiet()), StyleProp::FontSize(12.0)]);
    b.child(toolbar, tl2);
    for (label, active) in [("手動", true), ("名前", false), ("優先度", false)] {
        let chip = b.button(label, &[
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingRight(Dimension::px(12.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(if active { p.accent() } else { p.panel2() }),
            StyleProp::Color(if active { p.black() } else { p.text() }),
            StyleProp::BorderRadius(999.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if active { p.accent() } else { p.line() }),
            StyleProp::FontSize(12.0),
        ]);
        b.child(toolbar, chip);
    }

    // Todo 行
    let list = b.view(&col(8.0));
    let seed: [(&str, u8, bool); 5] = [
        ("レイアウトエンジンに flex-wrap を実装", 3, false),
        ("box-shadow の描画を確認する", 2, true),
        ("ドラッグで並べ替えできるかテスト", 2, false),
        ("ダークモードの配色を調整", 1, false),
        ("sticky ヘッダーの挙動チェック", 3, true),
    ];
    for (txt, prio, done) in seed {
        let r = b.view(&[
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::Gap(Dimension::px(12.0)),
            StyleProp::Padding(Dimension::px(12.0)),
            StyleProp::BackgroundColor(p.panel2()),
            StyleProp::BorderRadius(12.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(p.line()),
            StyleProp::Opacity(if done { 0.62 } else { 1.0 }),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0, offset_y: 2.0, blur: 6.0, spread: -1.0,
                color: p.shadow(), inset: false,
            }]),
        ]);
        let check = b.button(if done { "✓" } else { " " }, &[
            StyleProp::Width(Dimension::px(24.0)),
            StyleProp::Height(Dimension::px(24.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(if done { p.success() } else { p.panel() }),
            StyleProp::Color(p.black()),
            StyleProp::BorderRadius(7.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if done { p.success() } else { p.line() }),
            StyleProp::FontSize(14.0),
        ]);
        let dot = b.view(&[
            StyleProp::Width(Dimension::px(10.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::BackgroundColor(prio_tone(&p, prio)),
            StyleProp::BorderRadius(999.0),
        ]);
        let labelwrap = b.view(&[
            StyleProp::FlexGrow(1.0),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ]);
        let label = b.button(txt, &[
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::BackgroundColor(Color::TRANSPARENT),
            StyleProp::Color(if done { p.quiet() } else { p.ink() }),
            StyleProp::FontSize(15.0),
            StyleProp::BorderWidth(0.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
        ]);
        b.child(labelwrap, label);
        let prio_t = b.text(&format!("優先度 {}", PRIO_LABEL[prio as usize]),
            &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)]);
        let del = b.button(DELETE_GLYPH, &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(p.panel()),
            StyleProp::Color(p.muted()),
            StyleProp::BorderRadius(8.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(p.line()),
            StyleProp::FontSize(14.0),
        ]);
        b.children(r, &[check, dot, labelwrap, prio_t, del]);
        b.child(list, r);
    }

    // 区切り線 + フッター
    let divider = b.view(&[
        StyleProp::Height(Dimension::px(1.0)),
        StyleProp::BackgroundColor(p.line()),
    ]);
    let footer = b.view(&[
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::SpaceBetween),
    ]);
    let f1 = b.text("40% 完了", &[StyleProp::Color(p.muted()), StyleProp::FontSize(13.0)]);
    let fright = b.view(&row(12.0));
    let f2 = b.text(&format!("クリックで完了 / {DELETE_GLYPH} で削除"), &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)]);
    let clearbtn = b.button("完了を消す", &[
        StyleProp::Height(Dimension::px(30.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.panel2()),
        StyleProp::Color(p.text()),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(12.0),
    ]);
    b.children(fright, &[f2, clearbtn]);
    b.children(footer, &[f1, fright]);

    b.children(panel, &[header, note, addform, toolbar, list, divider, footer]);
    b.child(stage, panel);
    b.children(root, &[appbar, stage]);

    // ── 描画 ──────────────────────────────────────────────────────────────
    let graph = b.tree.render(0.0).clone();
    let mut pixmap = Pixmap::new(VW as u32, VH as u32).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
    let mut data = pixmap.data().to_vec();
    premultiplied_to_straight(&mut data);

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/ui-comparison/tiny-skia-tasks.png");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    write_png(&out, &data, VW as u32, VH as u32);
    eprintln!("wrote {}", out.display());
}

#[test]
fn render_glyph_coverage() {
    if std::env::var_os("HAYATE_WRITE_SCREENSHOT").is_none() {
        return;
    }
    const W: f32 = 720.0;
    const H: f32 = 120.0;
    let mut b = B::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::px(W)),
        StyleProp::Height(Dimension::px(H)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::Gap(Dimension::px(8.0)),
        StyleProp::Padding(Dimension::px(16.0)),
        StyleProp::BackgroundColor(Color::WHITE),
        StyleProp::DefaultColor(Color::BLACK),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(W, H);
    for g in ["🌙", "☀", "✓", "✕", "↑", "↓", "あ", "A"] {
        let t = b.text(g, &[StyleProp::FontSize(40.0), StyleProp::Color(Color::BLACK)]);
        b.child(root, t);
    }
    let graph = b.tree.render(0.0).clone();
    let mut pixmap = Pixmap::new(W as u32, H as u32).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
    let mut data = pixmap.data().to_vec();
    premultiplied_to_straight(&mut data);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/ui-comparison/tiny-skia-glyphs.png");
    write_png(&out, &data, W as u32, H as u32);
    eprintln!("wrote {}", out.display());
}

/// 「空グリフ」差異を捉える決定的なシグナル。候補グリフを単体で描画し、塗られた
/// （ink）ピクセル数を数える。0 ink は登録/フォールバックフォントにグリフが無い（バグ）、
/// >0 は描画できている。
#[test]
fn diagnose_glyph_ink() {
    if std::env::var_os("HAYATE_WRITE_SCREENSHOT").is_none() {
        return;
    }
    const W: u32 = 64;
    const H: u32 = 64;
    let candidates: &[(&str, &str)] = &[
        ("U+1F319 🌙 emoji moon", "🌙"),
        ("U+2600 ☀ sun dingbat", "☀"),
        ("U+263D ☽ first-quarter moon", "☽"),
        ("U+263E ☾ last-quarter moon", "☾"),
        ("U+1F311 🌑 emoji new moon", "🌑"),
        ("U+2713 ✓ check", "✓"),
        ("U+2715 ✕ multiply", "✕"),
        ("kana ク", "ク"),
        ("latin A", "A"),
    ];
    for (label, glyph) in candidates {
        let mut b = B::new();
        let root = b.view(&[
            StyleProp::Width(Dimension::px(W as f32)),
            StyleProp::Height(Dimension::px(H as f32)),
            StyleProp::BackgroundColor(Color::WHITE),
            StyleProp::DefaultFontFamily("Inter".to_string()),
        ]);
        b.tree.set_root(root);
        b.tree.set_viewport(W as f32, H as f32);
        let t = b.text(glyph, &[StyleProp::FontSize(40.0), StyleProp::Color(Color::BLACK)]);
        b.child(root, t);
        let graph = b.tree.render(0.0).clone();
        let mut pixmap = Pixmap::new(W, H).expect("pixmap");
        TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
        let data = pixmap.data();
        let ink = data.chunks_exact(4).filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200).count();
        eprintln!("[GLYPH-INK] {ink:>5} px  {label}");
    }
}

/// 同梱 Canvas フォント（NotoSansJP.ttf）が `ch` の実グリフを持つか。
/// `false` は cmap ミスを意味し、レンダラは意図したグリフの代わりに `.notdef`（豆腐□）を描く。
/// レンダラと同じ cmap である skrifa の charmap を使う。
fn font_has_glyph(ch: char) -> bool {
    use skrifa::{FontRef, MetadataProvider};
    let font = FontRef::new(NOTO_SANS_JP_BYTES).expect("parse NotoSansJP.ttf");
    font.charmap().map(ch).is_some()
}

/// 回帰テスト。todo example の削除コントロール（各行ボタン + フッターヒント）は、
/// 同梱 Canvas フォントが実際に描けるグリフを使う必要がある。これにより vello/tiny-skia は
/// `.notdef` 豆腐□ではなく本物の "×" を描く。`DELETE_GLYPH` を介して example と共有するグリフを守る。
#[test]
fn delete_glyph_renders_in_canvas() {
    // U+2715 ✕ はバグの証拠。同梱フォントにグリフが無く、Canvas は豆腐□に
    // フォールバックする（DOM はブラウザのフォントフォールバックで隠してしまう）。
    assert!(
        !font_has_glyph('\u{2715}'),
        "U+2715 ✕ is expected to be absent from NotoSansJP.ttf (the bug's cause)",
    );
    // example が実際に使うグリフは存在し、Canvas が描けなければならない。
    let delete_char = DELETE_GLYPH.chars().next().expect("DELETE_GLYPH is non-empty");
    assert!(
        font_has_glyph(delete_char),
        "delete glyph {DELETE_GLYPH:?} must exist in NotoSansJP.ttf, \
         else Canvas renders a .notdef tofu box instead of '×'",
    );
}

/// `s` を単体で 40px 描画した後、塗られた（ほぼ白でない）ピクセル数を数える。
/// `diagnose_glyph_ink` と同じ ink プローブ。0 は何も描かれない（無言の□）。
fn glyph_ink(s: &str) -> usize {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut b = B::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::px(W as f32)),
        StyleProp::Height(Dimension::px(H as f32)),
        StyleProp::BackgroundColor(Color::WHITE),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(W as f32, H as f32);
    let t = b.text(s, &[StyleProp::FontSize(40.0), StyleProp::Color(Color::BLACK)]);
    b.child(root, t);
    let graph = b.tree.render(0.0).clone();
    let mut pixmap = Pixmap::new(W, H).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
    pixmap
        .data()
        .chunks_exact(4)
        .filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200)
        .count()
}

/// 回帰テスト。同梱 Canvas フォントに無いコードポイントが、無言の `.notdef` □に
/// 消えてはならない。ペインタは意図的なプレースホルダを描くので、U+2715 ✕
/// （NotoSansJP に無く、`.notdef` アウトラインも空で 0 ink だった）が、何も描かないのではなく
/// 見えるインクを描く。
#[test]
fn missing_glyph_draws_visible_placeholder() {
    // 前提条件: このコードポイントは本当に欠落している（バグの根本原因）。
    assert!(
        !font_has_glyph('\u{2715}'),
        "U+2715 ✕ must be absent from NotoSansJP.ttf for this regression to be meaningful",
    );
    // 存在するグリフが対照。引き続きインクを描く必要がある。
    let present = glyph_ink("A");
    assert!(present > 0, "control glyph 'A' must render ink, got {present}");
    // 欠落コードポイントは空白ではなく見えるプレースホルダを生成しなければならない。
    let missing = glyph_ink("\u{2715}");
    assert!(
        missing > 0,
        "missing glyph U+2715 ✕ must draw a visible placeholder box, got {missing} ink px",
    );
}

// ───────────────────────── 入力状態の比較 ─────────────────────
// 入力インタラクション（クリックでフォーカス、入力、ドラッグ選択、IME 変換、ボタン hover）を
// Canvas mode（tiny-skia）で描画し、DOM / EditContext 描画と目視比較する。
// `App.tsx` の AddForm を再現する: `:focus` バリアント付き `inputStyle()`
// （border→accent, bg→panel3）と、`:hover` バリアント（bg→success）付きの teal 追加ボタン。

#[derive(Clone, Copy)]
struct InputState {
    label: &'static str,
    /// 確定済みテキスト。空文字列ならプレースホルダが表示される。
    content: &'static str,
    /// `content` の後ろに付く、変換中の IME 合成テール。
    preedit: &'static str,
    /// true のとき preedit を変換中（太い下線）の先頭文節と確定（細い下線）のテールに
    /// 分割し、変換中の見た目を表す（ADR-0102）。false なら preedit 全体が単一の細い下線。
    convert: bool,
    focused: bool,
    select_all: bool,
    hover_add: bool,
}

/// preedit を文字数の中点で、太い（変換中）先頭文節と細いテールに分割する。
/// IME の `textformatupdate` のテスト用代替。
fn convert_clauses(preedit: &str) -> Vec<hayate_core::CompositionClause> {
    use hayate_core::{CompositionClause, CompositionUnderline};
    let mid = preedit
        .char_indices()
        .nth(preedit.chars().count() / 2)
        .map(|(i, _)| i)
        .unwrap_or(preedit.len());
    let mut clauses = Vec::new();
    if mid > 0 {
        clauses.push(CompositionClause { start: 0, end: mid, underline: CompositionUnderline::Thick });
    }
    if mid < preedit.len() {
        clauses.push(CompositionClause { start: mid, end: preedit.len(), underline: CompositionUnderline::Thin });
    }
    clauses
}

const PANEL_W: u32 = 560;
const PANEL_H: u32 = 96;

/// AddForm の 1 行（テキスト入力 + 優先度セグメント 3 つ + 追加ボタン）を、`App.tsx` の
/// AddForm に忠実に構築する。入力の `:focus` と追加ボタンの `:hover` バリアントを
/// 登録し、インタラクション状態が解決されるようにする。(input, add_btn) を返す。
fn build_addform(b: &mut B, p: &P, root: ElementId, label: &str) -> (ElementId, ElementId) {
    let lbl = b.text(label, &[StyleProp::Color(p.muted()), StyleProp::FontSize(11.0)]);

    let form = b.view(&row(8.0));
    let input = b.el(ElementKind::TextInput, &[
        StyleProp::FlexGrow(1.0),
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::BackgroundColor(p.panel2()),
        StyleProp::Color(p.text()),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(13.0),
    ]);
    // アプリの `inputStyle()` 由来の `:focus` バリアント。
    b.tree.element_set_pseudo_style(input, PseudoState::Focus, &[
        StyleProp::BorderColor(p.accent()),
        StyleProp::BackgroundColor(p.panel3()),
    ]);
    // TextInput のプレースホルダテキストは `el.text` に置く（ADR-0058）。
    b.tree.element_set_text(input, "新しいタスクを入力…");

    let segs = b.view(&row(4.0));
    for (prio, active) in [(3u8, false), (2u8, true), (1u8, false)] {
        let tone = prio_tone(p, prio);
        let seg = b.button(PRIO_LABEL[prio as usize], &[
            StyleProp::Height(Dimension::px(38.0)),
            StyleProp::MinWidth(Dimension::px(40.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(if active { tone } else { p.panel2() }),
            StyleProp::Color(if active { p.black() } else { p.muted() }),
            StyleProp::BorderRadius(9.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(if active { tone } else { p.line() }),
            StyleProp::FontSize(13.0),
        ]);
        b.child(segs, seg);
    }

    let add = b.button("追加", &[
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(18.0)),
        StyleProp::PaddingRight(Dimension::px(18.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::BackgroundColor(p.accent()),
        StyleProp::Color(p.black()),
        StyleProp::BorderRadius(9.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(p.accent()),
        StyleProp::FontSize(13.0),
    ]);
    // アプリの追加ボタンの `:hover` バリアント（bg/border → success）。
    b.tree.element_set_pseudo_style(add, PseudoState::Hover, &[
        StyleProp::BackgroundColor(p.success()),
        StyleProp::BorderColor(p.success()),
    ]);

    b.children(form, &[input, segs, add]);
    b.children(root, &[lbl, form]);
    (input, add)
}

/// 1 つのインタラクション状態を専用のパネル pixmap に描画する。
fn render_input_state(st: &InputState) -> Pixmap {
    let p = P;
    let mut b = B::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::px(PANEL_W as f32)),
        StyleProp::Height(Dimension::px(PANEL_H as f32)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(6.0)),
        StyleProp::PaddingLeft(Dimension::px(16.0)),
        StyleProp::PaddingRight(Dimension::px(16.0)),
        StyleProp::PaddingTop(Dimension::px(14.0)),
        StyleProp::PaddingBottom(Dimension::px(14.0)),
        StyleProp::BackgroundColor(p.bg()),
        StyleProp::DefaultColor(p.text()),
        StyleProp::DefaultFontSize(14.0),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(PANEL_W as f32, PANEL_H as f32);
    let (input, add) = build_addform(&mut b, &p, root, st.label);

    // ── インタラクションを適用 ──
    if !st.content.is_empty() {
        b.tree.element_set_text_content(input, st.content);
    }
    if !st.preedit.is_empty() {
        if st.convert {
            b.tree
                .element_set_preedit_with_clauses(input, st.preedit, convert_clauses(st.preedit));
        } else {
            b.tree.element_set_preedit(input, st.preedit);
        }
    }
    if st.focused {
        b.tree.element_focus(input); // cursor_visible + :focus 疑似クラスを設定
    }
    if st.hover_add {
        b.tree.hover_enter_element(add); // :hover 疑似クラス
    }

    // 最初の描画でレイアウトを確定させ、座標からドラッグ選択を駆動できるようにする。
    let _ = b.tree.render(0.0);
    if st.select_all {
        if let Some((rx, ry, rw, rh)) = b.tree.element_layout_rect(input) {
            let my = ry + rh / 2.0;
            // 左パディングのすぐ内側から最後のグリフを越えるまでドラッグする。
            b.tree.on_pointer_down_on(input, rx + 13.0, my);
            b.tree.on_pointer_move(rx + rw - 6.0, my);
            b.tree.on_pointer_up(rx + rw - 6.0, my);
        }
    }

    let graph = b.tree.render(0.0).clone();
    let mut pixmap = Pixmap::new(PANEL_W, PANEL_H).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
    pixmap
}

/// addform 入力が `preedit`（任意で `convert` 分割）に対して出力する合成下線矩形の
/// 高さを左から右へ返す。グリフに埋もれず thin↔thick を正確に区別するため、
/// シーングラフを直接読む。
fn preedit_underline_heights(preedit: &str, convert: bool) -> Vec<f32> {
    let p = P;
    let mut b = B::new();
    let root = b.view(&col(0.0));
    b.tree.set_root(root);
    b.tree.set_viewport(PANEL_W as f32, PANEL_H as f32);
    let (input, _add) = build_addform(&mut b, &p, root, "");
    if convert {
        b.tree
            .element_set_preedit_with_clauses(input, preedit, convert_clauses(preedit));
    } else {
        b.tree.element_set_preedit(input, preedit);
    }
    b.tree.element_focus(input);
    let _ = b.tree.render(0.0);

    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(b.tree.scene_graph(), &mut painter);
    let mut heights: Vec<(f32, f32)> = painter
        .ops()
        .iter()
        .filter_map(|op| match op {
            hayate_core::DrawOp::FillRect { x, width, height, .. }
                if *height <= 3.0 && *width >= 5.0 =>
            {
                Some((*x, *height))
            }
            _ => None,
        })
        .collect();
    heights.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    heights.into_iter().map(|(_, h)| h).collect()
}

/// 比較のために描画するインタラクションの並び。
fn interaction_states() -> Vec<InputState> {
    vec![
        InputState { label: "1. rest — 未フォーカス（placeholder）", content: "", preedit: "", focused: false, select_all: false, hover_add: false, convert: false },
        InputState { label: "2. click → focus（空・caret + :focus リング）", content: "", preedit: "", focused: true, select_all: false, hover_add: false, convert: false },
        InputState { label: "3. type「牛乳を買う」（caret 末尾）", content: "牛乳を買う", preedit: "", focused: true, select_all: false, hover_add: false, convert: false },
        InputState { label: "4. drag select all（選択ハイライト）", content: "牛乳を買う", preedit: "", focused: true, select_all: true, hover_add: false, convert: false },
        InputState { label: "5. IME compose「ぎゅうにゅう」（preedit・変換前 単一下線）", content: "", preedit: "ぎゅうにゅう", focused: true, select_all: false, hover_add: false, convert: false },
        InputState { label: "5b. IME convert（clause 分割・太/細 下線）", content: "", preedit: "ぎゅうにゅう", focused: true, select_all: false, hover_add: false, convert: true },
        InputState { label: "6. hover 追加ボタン（:hover）", content: "牛乳を買う", preedit: "", focused: false, select_all: false, hover_add: true, convert: false },
    ]
}

#[test]
fn render_interaction_states() {
    if std::env::var_os("HAYATE_WRITE_SCREENSHOT").is_none() {
        return;
    }
    let panels: Vec<Pixmap> = interaction_states().iter().map(render_input_state).collect();
    let sep = 2u32;
    let total_h = panels.iter().map(|p| p.height()).sum::<u32>() + sep * (panels.len() as u32 - 1);
    let mut out = vec![0xffu8; (PANEL_W * total_h * 4) as usize];
    let mut y = 0u32;
    for pm in &panels {
        let mut d = pm.data().to_vec();
        premultiplied_to_straight(&mut d);
        for r in 0..pm.height() {
            let src = (r * pm.width() * 4) as usize;
            let dst = ((y + r) * PANEL_W * 4) as usize;
            let n = (pm.width().min(PANEL_W) * 4) as usize;
            out[dst..dst + n].copy_from_slice(&d[src..src + n]);
        }
        y += pm.height() + sep;
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/ui-comparison/interaction-states.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_png(&path, &out, PANEL_W, total_h);
    eprintln!("wrote {}", path.display());
}

/// 領域内で最も暗いテキストピクセルの straight-alpha RGB を取り、テキストが実際に
/// 塗られた色を復元する。
fn darkest_text_rgb(pm: &Pixmap, x0: u32, y0: u32, x1: u32, y1: u32) -> (u8, u8, u8) {
    let mut d = pm.data().to_vec();
    premultiplied_to_straight(&mut d);
    let w = pm.width();
    let mut best = (255u32, 0u8, 0u8, 0u8); // (luma, r, g, b)
    for y in y0..y1.min(pm.height()) {
        for x in x0..x1.min(w) {
            let i = ((y * w + x) * 4) as usize;
            let (r, g, b) = (d[i], d[i + 1], d[i + 2]);
            let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            if luma < best.0 {
                best = (luma, r, g, b);
            }
        }
    }
    (best.1, best.2, best.3)
}

/// straight-alpha のピクセルを 1 つサンプリングする。
fn sample_rgb(pm: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let mut d = pm.data().to_vec();
    premultiplied_to_straight(&mut d);
    let i = ((y * pm.width() + x) * 4) as usize;
    (d[i], d[i + 1], d[i + 2])
}

/// インタラクション状態の差異を捉える決定的なシグナル。`diagnose_glyph_ink` と同様に、
/// 各行が鋭く再現可能なプローブ。
#[test]
fn diagnose_interaction_signals() {
    if std::env::var_os("HAYATE_WRITE_SCREENSHOT").is_none() {
        return;
    }
    let p = P;

    // パネル内のテキスト領域の行（ラベル ~y14..28、入力行 ~y34..72）。
    let (ty0, ty1) = (34u32, 72u32);
    let (tx0, tx1) = (30u32, 360u32);

    // 1. プレースホルダ色: 空（プレースホルダ）と確定テキスト、同じグリフで比較。
    let placeholder = render_input_state(&InputState { label: "", content: "", preedit: "", focused: false, select_all: false, hover_add: false, convert: false });
    let committed = render_input_state(&InputState { label: "", content: "新しいタスクを入力…", preedit: "", focused: false, select_all: false, hover_add: false, convert: false });
    let ph = darkest_text_rgb(&placeholder, tx0, ty0, tx1, ty1);
    let cm = darkest_text_rgb(&committed, tx0, ty0, tx1, ty1);
    eprintln!("[PLACEHOLDER-RGB] placeholder={:?} committed={:?}  (#334 fixed: Canvas now paints ::placeholder muted — Chromium UA ~54% black/white per ADR-0102 — distinct from committed body color {:?}; exact value pending real-Chromium calibration vs DOM ~#9a93a3)", ph, cm, (p.text().to_array_f32()));

    // 2. フォーカスリング: 入力の左ボーダー色、非フォーカス vs フォーカス。
    let unfoc = render_input_state(&InputState { label: "", content: "", preedit: "", focused: false, select_all: false, hover_add: false, convert: false });
    let foc = render_input_state(&InputState { label: "", content: "", preedit: "", focused: true, select_all: false, hover_add: false, convert: false });
    // :focus 背景は panel2→panel3 に変わる。1px の line→accent ボーダー変化も
    // 読み取れる（`border-style: solid` を与え、ネイティブのフォーカスリングが
    // ボックスを消さなくなった）。ボーダー側は `addform_input_1px_border_renders`
    // で別途検証する。入力の塗りを中央でサンプリングする。
    eprintln!(
        "[FOCUS-FILL] unfocused={:?} focused={:?}  (panel2={:?} → panel3={:?}; :focus background applies; the 1px accent border + native focus ring are now both visible — see addform_input_1px_border_renders)",
        sample_rgb(&unfoc, 180, 50), sample_rgb(&foc, 180, 50),
        (p.panel2().to_array_f32()), (p.panel3().to_array_f32()),
    );

    // 3. キャレット: フォーカス済み空入力が非フォーカス空入力より多く描くインク。
    let ink = |pm: &Pixmap| -> u32 {
        let mut d = pm.data().to_vec();
        premultiplied_to_straight(&mut d);
        d.chunks_exact(4).filter(|c| c[0] < 120 && c[1] < 120 && c[2] < 120).count() as u32
    };
    eprintln!("[CARET-INK] focused-empty={} unfocused-empty={} (Δ≈caret px; Canvas core-draws the caret, DOM/EditContext uses the native caret)", ink(&foc), ink(&unfoc));

    // 4. 選択ハイライト: Material ブルーの色味のピクセル数を数え、色を報告する。
    let sel = render_input_state(&InputState { label: "", content: "牛乳を買う", preedit: "", focused: true, select_all: true, hover_add: false, convert: false });
    let mut sd = sel.data().to_vec();
    premultiplied_to_straight(&mut sd);
    let sel_px = sd.chunks_exact(4).filter(|c| c[2] > c[0] && c[2] > 110 && c[0] < 170 && c[1] < 200).count();
    eprintln!("[SELECTION-PX] material-blue-tint px={} (Canvas paints a fixed Material tint #3373F2~0.35; DOM uses the OS/::selection colour — colour & semantics differ)", sel_px);

    // 5. IME preedit: Canvas は合成に下線を引く（ADR-0102）。変換前は単一の細い下線、
    // 変換中は太い（アクティブ文節）+ 細い（確定テール）の分割。正確なシグナルは
    // 出力された下線矩形なので、シーングラフから直接その高さを報告する（ピクセル総数は
    // グリフ支配で 1px↔2px の下線差に鈍感）。高さは scene_build の
    // COMPOSITION_UNDERLINE_THIN/THICK に一致する。
    let pre = preedit_underline_heights("ぎゅうにゅう", false);
    let conv = preedit_underline_heights("ぎゅうにゅう", true);
    eprintln!(
        "[PREEDIT-INK] pre-conversion underlines={:?}; converting underlines={:?} (#336 fixed: Canvas underlines the composition — one thin line before conversion, a thick active clause + thin tail while converting, per Chromium/EditContext; exact px weights pending real-Chromium calibration)",
        pre, conv,
    );
}

/// AddForm 入力を描画し、(pixmap, 入力のレイアウト矩形) を返す。
fn render_input_state_with_rect(st: &InputState) -> (Pixmap, (f32, f32, f32, f32)) {
    let p = P;
    let mut b = B::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::px(PANEL_W as f32)),
        StyleProp::Height(Dimension::px(PANEL_H as f32)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(6.0)),
        StyleProp::PaddingLeft(Dimension::px(16.0)),
        StyleProp::PaddingRight(Dimension::px(16.0)),
        StyleProp::PaddingTop(Dimension::px(14.0)),
        StyleProp::PaddingBottom(Dimension::px(14.0)),
        StyleProp::BackgroundColor(p.bg()),
        StyleProp::DefaultColor(p.text()),
        StyleProp::DefaultFontSize(14.0),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(PANEL_W as f32, PANEL_H as f32);
    let (input, _add) = build_addform(&mut b, &p, root, st.label);
    if st.focused {
        b.tree.element_focus(input);
    }
    let _ = b.tree.render(0.0);
    let rect = b.tree.element_layout_rect(input).unwrap();
    let graph = b.tree.render(0.0).clone();
    let mut pixmap = Pixmap::new(PANEL_W, PANEL_H).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
    (pixmap, rect)
}

/// 受け入れプローブ（README の `[SCAN]` 手法を assertion 化）。AddForm 入力の左 1px
/// ボーダーは独立した不透明な列として現れること（非フォーカスは `line` 色、フォーカスは
/// `accent` 色）。さらにフォーカス時の内側は不透明のままであること（ネイティブの
/// フォーカスリングが消してはならない）。
#[test]
fn addform_input_1px_border_renders() {
    let p = P;
    let (unfoc, rect) = render_input_state_with_rect(&InputState { label: "", content: "", preedit: "", focused: false, select_all: false, hover_add: false, convert: false });
    let (foc, _) = render_input_state_with_rect(&InputState { label: "", content: "", preedit: "", focused: true, select_all: false, hover_add: false, convert: false });
    let (rx, ry, rw, rh) = rect;
    let bx = rx as u32; // 整数化した左端 — 1px ボーダーの列
    let my = (ry + rh / 2.0) as u32;

    let near = |got: (u8, u8, u8), want: Color, tol: i32, label: &str| {
        let [r, g, b, _] = want.to_array_f32();
        let w = ((r * 255.0) as i32, (g * 255.0) as i32, (b * 255.0) as i32);
        let d = (got.0 as i32 - w.0, got.1 as i32 - w.1, got.2 as i32 - w.2);
        assert!(
            d.0.abs() <= tol && d.1.abs() <= tol && d.2.abs() <= tol,
            "{label}: got {got:?}, want ~{w:?} (±{tol})"
        );
    };

    // 非フォーカス: 左ボーダー列はパネルの塗りではなく `line` 色。
    near(sample_rgb(&unfoc, bx, my), p.line(), 8, "unfocused 1px border = line colour");
    // フォーカス: アプリの `:focus` ボーダーは teal の `accent` に切り替わり、
    // 表示される（フォーカスリングが下のボーダー/塗りを消さなくなった）。
    near(sample_rgb(&foc, bx, my), p.accent(), 8, "focused 1px border = accent teal");

    // フォーカス時の入力内側は不透明のまま — 透明な穴は空かない。
    let cx = (rx + rw / 2.0) as u32;
    let i = ((my * foc.width() + cx) * 4) as usize;
    assert_eq!(foc.data()[i + 3], 255, "focused input interior must stay opaque");
}

fn write_png(path: &std::path::Path, rgba: &[u8], w: u32, h: u32) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}
