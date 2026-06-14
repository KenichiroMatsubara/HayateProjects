//! Semantic parity pixel golden baselines for tiny-skia (#151).

use hayate_scene_test_support::{run_all_parity_golden, run_parity_golden, PARITY_GOLDEN_CASES};

#[test]
fn all_parity_golden_pixels_tiny_skia() {
    run_all_parity_golden(PARITY_GOLDEN_CASES);
}

macro_rules! parity_golden_test {
    ($fn_name:ident, $idx:literal) => {
        #[test]
        fn $fn_name() {
            run_parity_golden(&PARITY_GOLDEN_CASES[$idx]);
        }
    };
}

parity_golden_test!(parity_golden_default_color, 0);
parity_golden_test!(parity_golden_block_noop, 1);
parity_golden_test!(parity_golden_text_direct, 2);
parity_golden_test!(parity_golden_ifc_inheritance, 3);
parity_golden_test!(parity_golden_font_weight_600, 4);
parity_golden_test!(parity_golden_font_style_italic, 5);
parity_golden_test!(parity_golden_font_weight_700_bold, 6);
parity_golden_test!(parity_golden_box_shadow_drop, 7);
