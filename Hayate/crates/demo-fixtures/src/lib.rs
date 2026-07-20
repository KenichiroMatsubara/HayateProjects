//! Renderer-independent demo `ElementTree` fixtures.
//!
//! Pure construction logic: each fixture assembles an [`ElementTree`] and returns
//! it, with no dependency on any Scene Renderer (tiny-skia / vello / wgpu). Shared
//! by the tiny-skia screenshot tests and the desktop bin (ADR-0118) so the "Tasks"
//! demo tree lives in one place. The fixture reproduces
//! `Tsubame/examples/hello-world/src/App.tsx` (light theme, teal accent) closely
//! enough to exercise layout / text / style differences.

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, ElementId, ElementKind, ElementTree,
    FlexDirectionValue, JustifyValue, Shadow, StyleProp,
};

/// Bundled Canvas font (also registered under the app's requested family names).
pub static NOTO_SANS_JP_BYTES: &[u8] = include_bytes!("../../core/assets/fonts/NotoSansJP.ttf");

/// Logical viewport the [`tasks_tree`] fixture is laid out for, in CSS px.
pub const TASKS_VIEWPORT: (f32, f32) = (980.0, 1060.0);

/// Logical viewport the [`dual_transition_tree`] fixture is laid out for, in CSS px.
pub const DUAL_TRANSITION_VIEWPORT: (f32, f32) = (200.0, 80.0);

/// `#rgb`, `#rrggbb`, `#rrggbbaa` parsed into core's [`Color`].
pub fn hex(s: &str) -> Color {
    let h = s.trim_start_matches('#');
    let n = |a: usize, b: usize| u8::from_str_radix(&h[a..b], 16).unwrap() as f64 / 255.0;
    match h.len() {
        6 => Color::new(n(0, 2), n(2, 4), n(4, 6), 1.0),
        8 => Color::new(n(0, 2), n(2, 4), n(4, 6), n(6, 8)),
        _ => panic!("bad hex {s}"),
    }
}

/// Light-theme + teal-accent palette transcribed from `theme.ts`.
pub struct Palette;

impl Palette {
    pub fn bg(&self) -> Color {
        hex("#f1ede3")
    }
    pub fn rail(&self) -> Color {
        hex("#fbf8f1")
    }
    pub fn panel(&self) -> Color {
        hex("#fdfdfb")
    }
    pub fn panel2(&self) -> Color {
        hex("#ece6d8")
    }
    pub fn panel3(&self) -> Color {
        hex("#e0d8c7")
    }
    pub fn ink(&self) -> Color {
        hex("#262130")
    }
    pub fn text(&self) -> Color {
        hex("#322c3f")
    }
    pub fn muted(&self) -> Color {
        hex("#6f6878")
    }
    pub fn quiet(&self) -> Color {
        hex("#9a93a3")
    }
    pub fn line(&self) -> Color {
        hex("#d9d3c6")
    }
    pub fn accent(&self) -> Color {
        hex("#14b8a6")
    }
    pub fn danger(&self) -> Color {
        hex("#e5484d")
    }
    pub fn success(&self) -> Color {
        hex("#2fa86a")
    }
    pub fn blue(&self) -> Color {
        hex("#4b8ef0")
    }
    pub fn black(&self) -> Color {
        hex("#14101c")
    }
    pub fn shadow(&self) -> Color {
        hex("#2621301f")
    }
}

pub fn prio_tone(p: &Palette, prio: u8) -> Color {
    match prio {
        3 => p.danger(),
        2 => hex("#ef9d2e"), // accent2
        _ => p.blue(),
    }
}

pub const PRIO_LABEL: [&str; 4] = ["", "低", "中", "高"];

/// Delete control glyph used by the todo rows + footer hint. The bundled Canvas
/// font (NotoSansJP.ttf) must have an outline for it. U+2715 ✕ is absent (Canvas
/// falls back to `.notdef` / 0 ink), but U+00D7 × renders. Kept in sync with
/// `Tsubame/examples/solid-demo`.
pub const DELETE_GLYPH: &str = "×";

/// Imperative [`ElementTree`] builder shared by the demo fixtures. Registers the
/// bundled Canvas font under the family names the app requests.
pub struct TreeBuilder {
    pub tree: ElementTree,
    next: u64,
}

impl TreeBuilder {
    pub fn new() -> Self {
        let mut tree = ElementTree::new();
        tree.register_font("Noto Sans", NOTO_SANS_JP_BYTES.to_vec());
        // The app requests Inter/Segoe/system-ui. Register the bundled face under
        // those names too so missing-font fallback doesn't mask a real defect.
        tree.register_font("Inter", NOTO_SANS_JP_BYTES.to_vec());
        Self { tree, next: 1 }
    }

    pub fn el(&mut self, kind: ElementKind, styles: &[StyleProp]) -> ElementId {
        let id = self.next;
        self.next += 1;
        let e = self.tree.element_create(id, kind);
        self.tree.element_set_style(e, styles);
        e
    }

    pub fn view(&mut self, styles: &[StyleProp]) -> ElementId {
        self.el(ElementKind::View, styles)
    }

    pub fn text(&mut self, s: &str, styles: &[StyleProp]) -> ElementId {
        let e = self.el(ElementKind::Text, styles);
        self.tree.element_set_text(e, s);
        e
    }

    /// Reproduces tsubame-solid's button construction: a Button container holding a
    /// child Text node (ADR-0058). The label colour/size are passed as the Button's
    /// `DefaultColor` / `DefaultFontSize` so the child text inherits them (matching
    /// the app's `defaultColor` usage).
    pub fn button(&mut self, s: &str, styles: &[StyleProp]) -> ElementId {
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

    pub fn child(&mut self, parent: ElementId, child: ElementId) {
        self.tree.element_append_child(parent, child);
    }

    pub fn children(&mut self, parent: ElementId, kids: &[ElementId]) {
        for &k in kids {
            self.tree.element_append_child(parent, k);
        }
    }
}

impl Default for TreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn row(gap: f32) -> Vec<StyleProp> {
    vec![
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::Gap(Dimension::px(gap)),
    ]
}

pub fn col(gap: f32) -> Vec<StyleProp> {
    vec![
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::Gap(Dimension::px(gap)),
    ]
}

/// Build the "Tasks" demo screen (Tsubame Task Studio) as a renderer-independent
/// [`ElementTree`]. `renderer_label` is shown in the AppBar badge so each consumer
/// can name the renderer it presents with (e.g. `"tiny-skia"`, `"vello"`). The
/// returned tree has its root and viewport set but is not yet rendered.
pub fn tasks_tree(renderer_label: &str) -> ElementTree {
    let (vw, vh) = TASKS_VIEWPORT;
    let p = Palette;
    let mut b = TreeBuilder::new();

    // Root column. ルートは固定 px ではなくビューポート充填にして窓リサイズへ
    // 追従させる (#567)。レイアウトエンジンは Percent ルートをビューポートに
    // ピン留めするので、`set_viewport` のたびに新しい寸法へ再レイアウトされる。
    let root = b.view(&[
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::BackgroundColor(p.bg()),
        StyleProp::DefaultColor(p.text()),
        StyleProp::DefaultFontSize(14.0),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(vw, vh);

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
    // Brand
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
    let logo_t = b.text(
        "TS",
        &[StyleProp::FontSize(18.0), StyleProp::Color(p.black())],
    );
    b.child(logo, logo_t);
    let titles = b.view(&col(2.0));
    let t1 = b.text(
        "Tsubame Task Studio",
        &[StyleProp::FontSize(20.0), StyleProp::Color(p.ink())],
    );
    let t2 = b.text(
        "POP TODO + Hayate CSS gallery",
        &[StyleProp::FontSize(12.0), StyleProp::Color(p.muted())],
    );
    b.children(titles, &[t1, t2]);
    b.children(brand, &[logo, titles]);

    // Right cluster
    let right = b.view(&row(10.0));
    let tab_tasks = b.button(
        "Tasks",
        &[
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
        ],
    );
    let tab_gallery = b.button(
        "CSS Gallery",
        &[
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
        ],
    );
    // Accent colour swatches
    let swatches = b.view(&row(6.0));
    for (i, c) in ["#14b8a6", "#e84d8a", "#ef8f3c", "#5ca80f", "#7c5cf0"]
        .iter()
        .enumerate()
    {
        let selected = i == 0;
        let sw = b.button(
            " ",
            &[
                StyleProp::Width(Dimension::px(22.0)),
                StyleProp::Height(Dimension::px(22.0)),
                StyleProp::BackgroundColor(hex(c)),
                StyleProp::BorderRadius(999.0),
                StyleProp::BorderWidth(if selected { 3.0 } else { 1.0 }),
                StyleProp::BorderStyle(BorderStyleValue::Solid),
                StyleProp::BorderColor(if selected { p.ink() } else { p.line() }),
            ],
        );
        b.child(swatches, sw);
    }
    let theme_btn = b.button(
        "🌙",
        &[
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
        ],
    );
    let rlabel = b.text(
        "renderer",
        &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)],
    );
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
    let badge_t = b.text(
        renderer_label,
        &[StyleProp::Color(p.accent()), StyleProp::FontSize(13.0)],
    );
    let badge_t2 = b.text(
        renderer_label,
        &[StyleProp::Color(p.muted()), StyleProp::FontSize(12.0)],
    );
    b.children(badge, &[badge_t, badge_t2]);
    b.children(
        right,
        &[tab_tasks, tab_gallery, swatches, theme_btn, rlabel, badge],
    );
    b.children(appbar, &[brand, right]);

    // ── Content panel ─────────────────────────────────────────────────────
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
            offset_x: 0.0,
            offset_y: 18.0,
            blur: 40.0,
            spread: -8.0,
            color: p.shadow(),
            inset: false,
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
    let htitle = b.text(
        "きょうのタスク",
        &[StyleProp::Color(p.ink()), StyleProp::FontSize(24.0)],
    );
    let hsub = b.text(
        "残り 3 件 / 全 5 件",
        &[StyleProp::Color(p.muted()), StyleProp::FontSize(13.0)],
    );
    b.children(hrow, &[htitle, hsub]);
    // Progress bar (40%)
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
    let input = b.el(
        ElementKind::TextInput,
        &[
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
        ],
    );
    b.tree.element_set_text(input, "新しいタスクを入力…");
    let segs = b.view(&row(4.0));
    for (prio, active) in [(3u8, false), (2u8, true), (1u8, false)] {
        let tone = prio_tone(&p, prio);
        let seg = b.button(
            PRIO_LABEL[prio as usize],
            &[
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
            ],
        );
        b.child(segs, seg);
    }
    let addbtn = b.button(
        "追加",
        &[
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
        ],
    );
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
    let tl1 = b.text(
        "表示",
        &[StyleProp::Color(p.quiet()), StyleProp::FontSize(12.0)],
    );
    b.child(toolbar, tl1);
    for (label, active) in [("すべて", true), ("未完了", false), ("完了済み", false)] {
        let chip = b.button(
            label,
            &[
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
            ],
        );
        b.child(toolbar, chip);
    }
    let tl2 = b.text(
        "並び",
        &[StyleProp::Color(p.quiet()), StyleProp::FontSize(12.0)],
    );
    b.child(toolbar, tl2);
    for (label, active) in [("手動", true), ("名前", false), ("優先度", false)] {
        let chip = b.button(
            label,
            &[
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
            ],
        );
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
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(p.line()),
            StyleProp::Opacity(if done { 0.62 } else { 1.0 }),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 2.0,
                blur: 6.0,
                spread: -1.0,
                color: p.shadow(),
                inset: false,
            }]),
        ]);
        let check = b.button(
            if done { "✓" } else { " " },
            &[
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
            ],
        );
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
        let label = b.button(
            txt,
            &[
                StyleProp::Display(hayate_core::DisplayValue::Flex),
                StyleProp::AlignItems(AlignValue::Center),
                StyleProp::BackgroundColor(Color::TRANSPARENT),
                StyleProp::Color(if done { p.quiet() } else { p.ink() }),
                StyleProp::FontSize(15.0),
                StyleProp::BorderWidth(0.0),
                StyleProp::BorderStyle(BorderStyleValue::Solid),
            ],
        );
        b.child(labelwrap, label);
        let prio_t = b.text(
            &format!("優先度 {}", PRIO_LABEL[prio as usize]),
            &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)],
        );
        let del = b.button(
            DELETE_GLYPH,
            &[
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
            ],
        );
        b.children(r, &[check, dot, labelwrap, prio_t, del]);
        b.child(list, r);
    }

    // Divider + footer
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
    let f1 = b.text(
        "40% 完了",
        &[StyleProp::Color(p.muted()), StyleProp::FontSize(13.0)],
    );
    let fright = b.view(&row(12.0));
    let f2 = b.text(
        &format!("クリックで完了 / {DELETE_GLYPH} で削除"),
        &[StyleProp::Color(p.quiet()), StyleProp::FontSize(11.0)],
    );
    let clearbtn = b.button(
        "完了を消す",
        &[
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
        ],
    );
    b.children(fright, &[f2, clearbtn]);
    b.children(footer, &[f1, fright]);

    b.children(
        panel,
        &[header, note, addform, toolbar, list, divider, footer],
    );
    b.child(stage, panel);
    b.children(root, &[appbar, stage]);

    b.tree
}

/// issue #680 実機回帰の再現構成: 優先度セグメントボタン（`Tsubame/examples/solid-demo/src/components/
/// AddForm.tsx` の `seg()`）と同型に、2 要素が同一フレームで同時に transition を開始し、同じ
/// duration（160ms・`seg()` が使う `EASE` 定数と同値）で同時に終わる。選択されていた方（`a`）が
/// 非アクティブ色へ、新しく選択された方（`b`）がアクティブ色へ切り替わる。
///
/// 返る tree はコールドフレーム（`render(0.0)`）で `a`＝アクティブ／`b`＝非アクティブの状態を
/// 一度確定させ、その直後に両方の背景色を入れ替え済み——呼び出し側の最初の `render(t)` が
/// transition 1 フレーム目になる（perf プローブが「実際に 2 レイヤが同時に dirty になった
/// フレーム」を計測できるようにするため）。
pub fn dual_transition_tree() -> ElementTree {
    let (vw, vh) = DUAL_TRANSITION_VIEWPORT;
    let mut b = TreeBuilder::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::px(vw)),
        StyleProp::Height(Dimension::px(vh)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Row),
        StyleProp::AlignItems(AlignValue::Center),
        StyleProp::JustifyContent(JustifyValue::Center),
        StyleProp::Gap(Dimension::px(4.0)),
        StyleProp::BackgroundColor(Color::new(0.95, 0.93, 0.89, 1.0)),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(vw, vh);

    let active = Color::new(0.05, 0.72, 0.63, 1.0); // accent tone
    let inactive = Color::new(0.93, 0.90, 0.85, 1.0); // panel2 tone
    let seg = |bg: Color| {
        vec![
            StyleProp::Height(Dimension::px(38.0)),
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(AlignValue::Center),
            StyleProp::JustifyContent(JustifyValue::Center),
            StyleProp::BackgroundColor(bg),
            StyleProp::BorderRadius(9.0),
            StyleProp::BorderWidth(1.0),
            StyleProp::BorderStyle(BorderStyleValue::Solid),
            StyleProp::BorderColor(bg),
            StyleProp::TransitionDuration(160.0),
        ]
    };
    let a = b.view(&seg(active));
    let bx = b.view(&seg(inactive));
    b.children(root, &[a, bx]);

    let _ = b.tree.render(0.0);
    b.tree.element_set_style(
        a,
        &[
            StyleProp::BackgroundColor(inactive),
            StyleProp::BorderColor(inactive),
        ],
    );
    b.tree.element_set_style(
        bx,
        &[
            StyleProp::BackgroundColor(active),
            StyleProp::BorderColor(active),
        ],
    );
    b.tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{render_scene_graph, DrawOp, RecordingPainter};

    /// Lay the fixture out and collect every string the scene draws as text.
    fn text_runs(tree: &mut ElementTree) -> Vec<String> {
        let _ = tree.render(0.0);
        let mut painter = RecordingPainter::new();
        render_scene_graph(tree.scene_graph(), &mut painter);
        painter
            .ops()
            .iter()
            .filter_map(|op| match op {
                DrawOp::DrawTextRun { data, .. } => Some(data.text.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tasks_tree_lays_out_to_viewport() {
        let mut tree = tasks_tree("tiny-skia");
        let _ = tree.render(0.0);
        let root = tree.root().expect("fixture must set a root element");
        let (_, _, w, h) = tree
            .element_layout_rect(root)
            .expect("root must have a layout rect");
        assert_eq!((w, h), TASKS_VIEWPORT);
    }

    #[test]
    fn tasks_tree_root_follows_viewport_resize() {
        // 窓リサイズ追従 (#567): ルートはビューポートを充填し、`set_viewport`
        // で寸法が変わったら再レイアウト後に新しいビューポートへ追従する。
        // 固定 px ルートだとここで旧寸法のまま固まる。
        let mut tree = tasks_tree("tiny-skia");
        let _ = tree.render(0.0);

        let resized = (1280.0, 720.0);
        tree.set_viewport(resized.0, resized.1);
        let _ = tree.render(0.0);

        let root = tree.root().expect("fixture must set a root element");
        let (_, _, w, h) = tree
            .element_layout_rect(root)
            .expect("root must have a layout rect");
        assert_eq!((w, h), resized);
    }

    #[test]
    fn tasks_tree_renders_the_studio_content() {
        let mut tree = tasks_tree("tiny-skia");
        let runs = text_runs(&mut tree);
        // AppBar brand + a seeded todo row must be present — proves the whole
        // "Tasks" tree (not just a root) was built.
        assert!(
            runs.iter().any(|t| t == "Tsubame Task Studio"),
            "brand title missing; got {runs:?}"
        );
        assert!(
            runs.iter()
                .any(|t| t == "レイアウトエンジンに flex-wrap を実装"),
            "seeded todo label missing; got {runs:?}"
        );
    }

    #[test]
    fn renderer_label_names_the_badge() {
        // The badge text follows the caller's renderer label so the desktop bin
        // (vello) and the tiny-skia test can share the fixture without forking it.
        let mut tree = tasks_tree("vello");
        let runs = text_runs(&mut tree);
        assert!(
            runs.iter().any(|t| t == "vello"),
            "badge should say 'vello'; got {runs:?}"
        );
        assert!(
            !runs.iter().any(|t| t == "tiny-skia"),
            "tiny-skia label must not leak when building for another renderer; got {runs:?}"
        );
    }

    #[test]
    fn dual_transition_tree_returns_with_both_segments_already_toggled_and_pending() {
        // The fixture pre-applies the #680 toggle (a active->inactive, b inactive->active) so the
        // caller's first render() is transition frame 1, not the cold frame.
        let mut tree = dual_transition_tree();
        assert!(
            tree.has_pending_visual_work(),
            "fixture must return with a transition already in flight"
        );
        let root = tree.root().expect("fixture must set a root element");
        let _ = tree.render(16.0);
        let (_, _, w, h) = tree
            .element_layout_rect(root)
            .expect("root must have a layout rect");
        assert_eq!((w, h), DUAL_TRANSITION_VIEWPORT);
    }
}
