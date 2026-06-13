//! Pixel-level regression tests for every Hayate CSS catalog property (Vello).
//!
//! Skips entirely when no wgpu adapter is available.

use hayate_scene_test_support::{run_all_vello, run_vello, try_vello_harness, CSS_PIXEL_CASES};

#[test]
fn all_catalog_css_properties_vello() {
    let _ran = run_all_vello(CSS_PIXEL_CASES);
}

macro_rules! vello_property_test {
    ($fn_name:ident, $idx:literal) => {
        #[test]
        fn $fn_name() {
            let Some(mut harness) = try_vello_harness() else {
                eprintln!(
                    "skip {}: no wgpu adapter",
                    CSS_PIXEL_CASES[$idx].css_property
                );
                return;
            };
            run_vello(&CSS_PIXEL_CASES[$idx], &mut harness);
        }
    };
}

vello_property_test!(css_pixel_background_color, 0);
vello_property_test!(css_pixel_opacity, 1);
vello_property_test!(css_pixel_border_radius, 2);
vello_property_test!(css_pixel_border_width, 3);
vello_property_test!(css_pixel_border_color, 4);
vello_property_test!(css_pixel_width, 5);
vello_property_test!(css_pixel_height, 6);
vello_property_test!(css_pixel_min_width, 7);
vello_property_test!(css_pixel_min_height, 8);
vello_property_test!(css_pixel_max_width, 9);
vello_property_test!(css_pixel_max_height, 10);
vello_property_test!(css_pixel_display_flex, 11);
vello_property_test!(css_pixel_display_none, 12);
vello_property_test!(css_pixel_display_grid, 13);
vello_property_test!(css_pixel_grid_template_columns_fr, 14);
vello_property_test!(css_pixel_grid_template_columns_px, 15);
vello_property_test!(css_pixel_flex_direction, 16);
vello_property_test!(css_pixel_align_items, 17);
vello_property_test!(css_pixel_justify_content, 18);
vello_property_test!(css_pixel_gap, 19);
vello_property_test!(css_pixel_padding, 20);
vello_property_test!(css_pixel_padding_top, 21);
vello_property_test!(css_pixel_padding_right, 22);
vello_property_test!(css_pixel_padding_bottom, 23);
vello_property_test!(css_pixel_padding_left, 24);
vello_property_test!(css_pixel_margin, 25);
vello_property_test!(css_pixel_margin_top, 26);
vello_property_test!(css_pixel_margin_right, 27);
vello_property_test!(css_pixel_margin_bottom, 28);
vello_property_test!(css_pixel_margin_left, 29);
vello_property_test!(css_pixel_font_size, 30);
vello_property_test!(css_pixel_color, 31);
vello_property_test!(css_pixel_font_family, 32);
vello_property_test!(css_pixel_font_weight, 33);
vello_property_test!(css_pixel_text_decoration_underline, 34);
vello_property_test!(css_pixel_text_decoration_line_through, 35);
vello_property_test!(css_pixel_z_index, 36);
vello_property_test!(css_pixel_flex_grow, 37);
vello_property_test!(css_pixel_flex_shrink, 38);
vello_property_test!(css_pixel_flex_basis, 39);
vello_property_test!(css_pixel_align_self, 40);
vello_property_test!(css_pixel_align_content, 41);
vello_property_test!(css_pixel_flex_wrap, 42);
vello_property_test!(css_pixel_border_style, 43);
