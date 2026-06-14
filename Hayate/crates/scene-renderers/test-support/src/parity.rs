//! Semantic parity pixel fixtures for tiny-skia golden baselines (#151).

use std::path::{Path, PathBuf};

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, FontStyleValue, Shadow, StyleProp,
};

use crate::cases::render_tree_to_scene;
use crate::golden::assert_pixels_match_golden;
use crate::pixel::{CANVAS_H, CANVAS_W};
use crate::tiny_skia;

static NOTO_SANS_JP_BYTES: &[u8] = include_bytes!("../../../core/assets/fonts/NotoSansJP.ttf");

fn register_bundled_font(tree: &mut ElementTree) {
    tree.register_font("Noto Sans", NOTO_SANS_JP_BYTES.to_vec());
}

fn viewport(tree: &mut ElementTree) {
    tree.set_viewport(CANVAS_W as f32, CANVAS_H as f32);
}

fn root_view(tree: &mut ElementTree, id: u64) -> hayate_core::ElementId {
    let root = tree.element_create(id, ElementKind::View);
    tree.set_root(root);
    viewport(tree);
    root
}

fn view_text_tree(view_styles: &[StyleProp], text_styles: &[StyleProp], text: &str) -> ElementTree {
    let mut tree = ElementTree::new();
    register_bundled_font(&mut tree);
    let view = root_view(&mut tree, 1);
    let text_id = tree.element_create(2, ElementKind::Text);
    let mut view_props = vec![
        StyleProp::Width(Dimension::px(CANVAS_W as f32)),
        StyleProp::Height(Dimension::px(CANVAS_H as f32)),
    ];
    view_props.extend_from_slice(view_styles);
    tree.element_set_style(view, &view_props);
    tree.element_append_child(view, text_id);
    let mut text_props = vec![StyleProp::FontSize(24.0)];
    text_props.extend_from_slice(text_styles);
    tree.element_set_style(text_id, &text_props);
    tree.element_set_text(text_id, text);
    tree
}

fn ifc_text_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    register_bundled_font(&mut tree);
    let view = root_view(&mut tree, 10);
    let outer = tree.element_create(11, ElementKind::Text);
    let inner = tree.element_create(12, ElementKind::Text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(CANVAS_W as f32)),
            StyleProp::Height(Dimension::px(CANVAS_H as f32)),
            StyleProp::Color(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::FontSize(32.0),
        ],
    );
    tree.element_append_child(view, outer);
    tree.element_append_child(outer, inner);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Color(Color::new(0.2, 0.4, 0.6, 1.0)),
            StyleProp::FontSize(18.0),
        ],
    );
    tree.element_set_text(outer, "Hi ");
    tree.element_set_text(inner, "there");
    tree
}

pub struct ParityGoldenCase {
    pub name: &'static str,
    pub build: fn() -> ElementTree,
    pub check: fn(&[u8]),
}

pub fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tiny-skia/tests/golden")
        .join(format!("{name}.png"))
}


fn assert_has_ink(data: &[u8]) {
    use crate::pixel::pixel;
    let has_ink = (0..CANVAS_H).any(|y| {
        (0..CANVAS_W).any(|x| {
            let px = pixel(data, CANVAS_W, x, y);
            px[0] < 240 || px[1] < 240 || px[2] < 240
        })
    });
    assert!(has_ink, "expected rendered text ink");
}

fn box_shadow_drop_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = root_view(&mut tree, 30);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 8.0,
                offset_y: 8.0,
                blur: 6.0,
                spread: 0.0,
                color: Color::new(0.0, 0.0, 0.0, 0.5),
                inset: false,
            }]),
        ],
    );
    tree
}

fn assert_box_shadow_drop_ink(data: &[u8]) {
    use crate::pixel::pixel;
    // The blurred drop shadow darkens pixels down-right of the box…
    let shadow = pixel(data, CANVAS_W, 56, 56);
    assert!(
        shadow[0] < 240 && shadow[3] > 0,
        "expected box-shadow ink below-right of box, got {shadow:?}"
    );
    // …while a far corner stays clear.
    let far = pixel(data, CANVAS_W, 95, 95);
    assert!(far[0] >= 240, "expected clear far corner, got {far:?}");
}

pub const PARITY_GOLDEN_CASES: &[ParityGoldenCase] = &[
    ParityGoldenCase {
        name: "parity_default_color_penetrates",
        build: || {
            view_text_tree(
                &[StyleProp::DefaultColor(Color::new(1.0, 0.4, 0.0, 1.0))],
                &[],
                "ambient",
            )
        },
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_block_color_noop",
        build: || {
            view_text_tree(
                &[
                    StyleProp::Color(Color::new(1.0, 0.0, 0.0, 1.0)),
                    StyleProp::FontSize(24.0),
                ],
                &[],
                "child",
            )
        },
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_text_direct_color",
        build: || {
            view_text_tree(
                &[],
                &[StyleProp::Color(Color::new(0.0, 1.0, 0.0, 1.0))],
                "styled",
            )
        },
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_ifc_text_inheritance",
        build: ifc_text_tree,
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_font_weight_600",
        build: || view_text_tree(&[], &[StyleProp::FontWeight(600.0)], "w600"),
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_font_style_italic",
        build: || {
            view_text_tree(
                &[],
                &[StyleProp::FontStyle(FontStyleValue::Italic)],
                "italic",
            )
        },
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_font_weight_700_bold",
        build: || view_text_tree(&[], &[StyleProp::FontWeight(700.0)], "bold"),
        check: assert_has_ink,
    },
    ParityGoldenCase {
        name: "parity_box_shadow_drop",
        build: box_shadow_drop_tree,
        check: assert_box_shadow_drop_ink,
    },
];

pub fn run_parity_golden(case: &ParityGoldenCase) {
    let tree = (case.build)();
    let pixels = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(tree));
    (case.check)(&pixels);
    assert_pixels_match_golden(&golden_path(case.name), &pixels, CANVAS_W, CANVAS_H);
}

pub fn run_all_parity_golden(cases: &[ParityGoldenCase]) {
    for case in cases {
        let name = case.name;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_parity_golden(case);
        }));
        if let Err(payload) = result {
            std::panic::resume_unwind(match payload.downcast::<String>() {
                Ok(s) => Box::new(format!("{name}: {s}")),
                Err(p) => p,
            });
        }
    }
}
