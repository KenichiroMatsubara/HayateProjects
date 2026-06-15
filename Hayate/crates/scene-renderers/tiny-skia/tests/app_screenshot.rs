//! Renders the Tsubame hello-world "Tasks" screen through tiny-skia so the
//! Canvas-mode output can be eyeballed against DOM-mode (browser) rendering.
//!
//! Run with `HAYATE_WRITE_SCREENSHOT=1` to emit the PNG; otherwise the test is
//! a no-op so it never gates CI. The fixture mirrors `Tsubame/examples/
//! hello-world/src/App.tsx` (light theme, teal accent) closely enough to surface
//! layout / text / styling divergences from the DOM renderer.

use hayate_core::{
    AlignValue, Color, Dimension, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    JustifyValue, Shadow, StyleProp,
};
use hayate_scene_renderer_tiny_skia::{premultiplied_to_straight, TinySkiaSceneRenderer};
use tiny_skia::Pixmap;

static NOTO_SANS_JP_BYTES: &[u8] =
    include_bytes!("../../../core/assets/fonts/NotoSansJP.ttf");

const VW: f32 = 980.0;
const VH: f32 = 1060.0;

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into a core `Color`.
fn hex(s: &str) -> Color {
    let h = s.trim_start_matches('#');
    let n = |a: usize, b: usize| u8::from_str_radix(&h[a..b], 16).unwrap() as f64 / 255.0;
    match h.len() {
        6 => Color::new(n(0, 2), n(2, 4), n(4, 6), 1.0),
        8 => Color::new(n(0, 2), n(2, 4), n(4, 6), n(6, 8)),
        _ => panic!("bad hex {s}"),
    }
}

/// Light-theme + teal-accent palette, copied from `theme.ts`.
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

struct B {
    tree: ElementTree,
    next: u64,
}

impl B {
    fn new() -> Self {
        let mut tree = ElementTree::new();
        tree.register_font("Noto Sans", NOTO_SANS_JP_BYTES.to_vec());
        // The app asks for Inter/Segoe/system-ui; register the bundled face under
        // those names too so missing-font fallback doesn't mask real findings.
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

    /// Mirrors how tsubame-solid builds a button: a Button container with a
    /// child Text node (ADR-0058). The label colour / size are passed as the
    /// button's ambient `DefaultColor` / `DefaultFontSize` so the child text
    /// inherits them, matching the app's `defaultColor` usage.
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

    // root column
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
        StyleProp::BorderColor(p.line()),
        StyleProp::PaddingLeft(Dimension::px(24.0)),
        StyleProp::PaddingRight(Dimension::px(24.0)),
    ]);
    // brand
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

    // right cluster
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
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(13.0),
    ]);
    // accent swatches
    let swatches = b.view(&row(6.0));
    for (i, c) in ["#14b8a6", "#e84d8a", "#ef8f3c", "#5ca80f", "#7c5cf0"].iter().enumerate() {
        let selected = i == 0;
        let sw = b.button(" ", &[
            StyleProp::Width(Dimension::px(22.0)),
            StyleProp::Height(Dimension::px(22.0)),
            StyleProp::BackgroundColor(hex(c)),
            StyleProp::BorderRadius(999.0),
            StyleProp::BorderWidth(if selected { 3.0 } else { 1.0 }),
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
        StyleProp::BorderColor(p.line()),
    ]);
    let badge_t = b.text("tiny-skia", &[StyleProp::Color(p.accent()), StyleProp::FontSize(13.0)]);
    let badge_t2 = b.text("tiny-skia", &[StyleProp::Color(p.muted()), StyleProp::FontSize(12.0)]);
    b.children(badge, &[badge_t, badge_t2]);
    b.children(right, &[tab_tasks, tab_gallery, swatches, theme_btn, rlabel, badge]);
    b.children(appbar, &[brand, right]);

    // ── content panel ─────────────────────────────────────────────────────
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
        StyleProp::BorderColor(p.line()),
        StyleProp::BoxShadow(vec![Shadow {
            offset_x: 0.0, offset_y: 18.0, blur: 40.0, spread: -8.0,
            color: p.shadow(), inset: false,
        }]),
    ]);

    // Header
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
    // progress bar (40%)
    let pbar = b.view(&[
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::px(12.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::BackgroundColor(p.black()),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
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
            StyleProp::BorderColor(if active { p.accent() } else { p.line() }),
            StyleProp::FontSize(12.0),
        ]);
        b.child(toolbar, chip);
    }

    // Todo rows
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
        ]);
        b.child(labelwrap, label);
        let prio_t = b.text(&format!("優先度 {}", PRIO_LABEL[prio as usize]),
            &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)]);
        let del = b.button("✕", &[
            StyleProp::Width(Dimension::px(30.0)),
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(p.panel()),
            StyleProp::Color(p.muted()),
            StyleProp::BorderRadius(8.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderColor(p.line()),
            StyleProp::FontSize(14.0),
        ]);
        b.children(r, &[check, dot, labelwrap, prio_t, del]);
        b.child(list, r);
    }

    // divider + footer
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
    let f2 = b.text("クリックで完了 / ✕ で削除", &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)]);
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
        StyleProp::BorderColor(p.line()),
        StyleProp::FontSize(12.0),
    ]);
    b.children(fright, &[f2, clearbtn]);
    b.children(footer, &[f1, fright]);

    b.children(panel, &[header, note, addform, toolbar, list, divider, footer]);
    b.child(stage, panel);
    b.children(root, &[appbar, stage]);

    // ── render ──────────────────────────────────────────────────────────────
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

/// Sharp, deterministic signal for the "blank glyph" divergence: render each
/// candidate glyph alone and count painted (ink) pixels. 0 ink == no glyph in
/// any registered/fallback font (the bug); >0 == renders.
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

fn write_png(path: &std::path::Path, rgba: &[u8], w: u32, h: u32) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}
