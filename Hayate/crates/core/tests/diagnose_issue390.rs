//! Canvas のテキストレイアウトが DOM と乖離する問題の診断ハーネス:
//! (1) text-input 内の短いテキストが 1 文字/行に折り返す、(2) ボタンのテキストが
//! 中央寄せされず上で見切れる、(3) input のボーダーが左端の細片になる。
//!
//! CSS ギャラリーの `PopCard` デモコンテナ（`align-items: flex-start` の列方向 flex）に
//! 実際のギャラリー要素を入れて再現し、レイアウト幾何を出力して根本原因を
//! 鋭く決定的なシグナルにする。
//!
//! 知見（env ゲート付き。`HAYATE_DIAGNOSE=1 … -- --nocapture` で実行）:
//!   * 乖離は完全に core レイアウト（Taffy projection）内、シーン描画の上流にある
//!     ため vello と tiny-skia は同一に再現する。
//!   * 根本原因 A（症状 1・3）: `text-input` は measure fn を持たない Taffy リーフで
//!     intrinsic content width が 0 となり、width:auto + flex-grow なし + 非 stretch の
//!     交差軸の下で padding 幅に潰れていた。現在はブラウザ `<input>` の UA デフォルト幅
//!     （解決済みフォントで N=20 文字分）を持つ。下のアサーションが修正を保証する。
//!   * 根本原因 B（症状 2）: `button` はプレーンな Taffy flex box に projection され、
//!     ブラウザ `<button>` の UA デフォルト（内容を中央寄せ）を焼き込まないため、
//!     ラベルが stretch（align-items:stretch）され glyph がボックス上端に描かれる。
//!     content width とは独立。

use hayate_core::{
    AlignValue, BorderStyleValue, Color, Dimension, DisplayValue, ElementId, ElementKind,
    ElementTree, FlexDirectionValue, StyleProp,
};

static FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");

fn input_style() -> Vec<StyleProp> {
    // theme.ts の `inputStyle()` に合わせる — width も flex-grow も持たないことに注意。
    vec![
        StyleProp::Height(Dimension::px(38.0)),
        StyleProp::PaddingLeft(Dimension::px(12.0)),
        StyleProp::PaddingRight(Dimension::px(12.0)),
        StyleProp::BorderRadius(8.0),
        StyleProp::BorderWidth(1.0),
        StyleProp::BorderStyle(BorderStyleValue::Solid),
        StyleProp::BorderColor(Color::new(0.85, 0.83, 0.78, 1.0)),
        StyleProp::FontSize(13.0),
    ]
}

#[test]
fn diagnose_390() {
    if std::env::var_os("HAYATE_DIAGNOSE").is_none() {
        return;
    }
    let mut tree = ElementTree::new();
    tree.register_font("Inter", FONT.to_vec());
    let mut next = 1u64;
    let mut mk = |tree: &mut ElementTree, kind: ElementKind, styles: &[StyleProp]| {
        let id = tree.element_create(next, kind);
        next += 1;
        tree.element_set_style(id, styles);
        id
    };

    // ルートサーフェス
    let root = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::DefaultFontFamily("Inter".to_string()),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.set_root(root);
    tree.set_viewport(300.0, 400.0);

    // PopCard デモコンテナ: 列方向 flex、align-items: flex-start、padding 14。
    let demo = mk(
        &mut tree,
        ElementKind::View,
        &[
            StyleProp::Display(DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::Gap(Dimension::px(8.0)),
            StyleProp::AlignItems(AlignValue::FlexStart),
            StyleProp::Padding(Dimension::px(14.0)),
        ],
    );
    tree.element_append_child(root, demo);

    // (A) placeholder 付き text-input、inputStyle（width なし）。
    let input = mk(&mut tree, ElementKind::TextInput, &input_style());
    tree.element_set_text(input, "Type here");
    tree.element_append_child(demo, input);

    // (B) button "Click" — height 36 と padding を持つが display/align は持たない
    // （ブラウザデフォルトの中央寄せに依存）。
    let button = mk(
        &mut tree,
        ElementKind::Button,
        &[
            StyleProp::Height(Dimension::px(36.0)),
            StyleProp::PaddingLeft(Dimension::px(14.0)),
            StyleProp::PaddingRight(Dimension::px(14.0)),
            StyleProp::BorderRadius(10.0),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    let label = mk(&mut tree, ElementKind::Text, &[]);
    tree.element_set_text(label, "Click");
    tree.element_append_child(button, label);
    tree.element_append_child(demo, button);

    let _ = tree.render(0.0);

    let rect = |t: &ElementTree, id: ElementId| {
        t.element_layout_rect(id)
            .unwrap_or((-1.0, -1.0, -1.0, -1.0))
    };
    let (dx, _dy, dw, _dh) = rect(&tree, demo);
    let (ix, _iy, iw, ih) = rect(&tree, input);
    let (bx, by, bw, bh) = rect(&tree, button);
    let (lx, ly, lw, lh) = rect(&tree, label);

    eprintln!(
        "[D390] demo box: x={dx} w={dw} (content width = {})",
        dw - 28.0
    );
    eprintln!(
        "[D390] INPUT  box: x={ix} w={iw} h={ih}  (content width = {})",
        iw - 24.0 - 2.0
    );
    eprintln!("[D390] BUTTON box: x={bx} y={by} w={bw} h={bh}");
    eprintln!(
        "[D390] LABEL  box: x={lx} y={ly} w={lw} h={lh}  (top gap above text = {}, bottom gap = {})",
        ly - by,
        (by + bh) - (ly + lh)
    );
    eprintln!(
        "[D390] root cause A FIXED (issue #403): input content width = {} → placeholder fits 1 line; border wraps the whole field (no left-edge sliver)",
        (iw - 26.0).max(0.0),
        // iw は下のアサーションで使う。
    );
    eprintln!(
        "[D390] symptom2 (root cause B): button label top gap {} vs bottom gap {} — equal when centered",
        ly - by,
        (by + bh) - (ly + lh),
    );

    // レンダラー独立性は構造的: レイアウトは SceneGraph 走査の前に core で解決される
    // ため、両 Scene Renderer はこれらの rect を等しく観測する。
    //
    // 両根本原因とも修正済みで、ここで実際のリグレッションガードとしてアサートする。
    //
    // 根本原因 A: width 未指定の text-input がフォント相対の UA デフォルト幅を持つように
    // なり、content width が 0 を十分上回る（1 文字/行の折り返しや左端ボーダーの細片なし）。
    // 専用のリグレッションテストは `tests/text_input_default_width.rs` にある。
    assert!(
        iw - 26.0 > 50.0,
        "regression guard (#403): input must carry the UA default width (content width = {})",
        iw - 26.0,
    );
    // 根本原因 B（ADR-0109）: button が UA デフォルトの `align-items: center` を
    // 与えるようになり、ラベルが上端で見切れず垂直中央寄せされる。
    let top_gap = ly - by;
    let bottom_gap = (by + bh) - (ly + lh);
    assert!(
        top_gap > 2.0 && (top_gap - bottom_gap).abs() < 2.0,
        "button label must be vertically centered (top gap={top_gap}, bottom gap={bottom_gap})"
    );
}
