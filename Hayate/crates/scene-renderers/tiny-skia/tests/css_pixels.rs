//! Pixel-level regression tests for every Hayate CSS catalog property (tiny-skia).

use hayate_scene_test_support::{run_all_tiny_skia, run_tiny_skia, CSS_PIXEL_CASES};

#[test]
fn all_catalog_css_properties_tiny_skia() {
    run_all_tiny_skia(CSS_PIXEL_CASES);
}

macro_rules! tiny_skia_property_test {
    ($fn_name:ident, $idx:literal) => {
        #[test]
        fn $fn_name() {
            run_tiny_skia(&CSS_PIXEL_CASES[$idx]);
        }
    };
}

tiny_skia_property_test!(css_pixel_background_color, 0);
tiny_skia_property_test!(css_pixel_opacity, 1);
tiny_skia_property_test!(css_pixel_border_radius, 2);
tiny_skia_property_test!(css_pixel_border_width, 3);
tiny_skia_property_test!(css_pixel_border_color, 4);
tiny_skia_property_test!(css_pixel_width, 5);
tiny_skia_property_test!(css_pixel_height, 6);
tiny_skia_property_test!(css_pixel_min_width, 7);
tiny_skia_property_test!(css_pixel_min_height, 8);
tiny_skia_property_test!(css_pixel_max_width, 9);
tiny_skia_property_test!(css_pixel_max_height, 10);
tiny_skia_property_test!(css_pixel_display_flex, 11);
tiny_skia_property_test!(css_pixel_display_none, 12);
tiny_skia_property_test!(css_pixel_display_grid, 13);
tiny_skia_property_test!(css_pixel_grid_template_columns_fr, 14);
tiny_skia_property_test!(css_pixel_grid_template_columns_px, 15);
tiny_skia_property_test!(css_pixel_flex_direction, 16);
tiny_skia_property_test!(css_pixel_align_items, 17);
tiny_skia_property_test!(css_pixel_justify_content, 18);
tiny_skia_property_test!(css_pixel_gap, 19);
tiny_skia_property_test!(css_pixel_padding, 20);
tiny_skia_property_test!(css_pixel_padding_top, 21);
tiny_skia_property_test!(css_pixel_padding_right, 22);
tiny_skia_property_test!(css_pixel_padding_bottom, 23);
tiny_skia_property_test!(css_pixel_padding_left, 24);
tiny_skia_property_test!(css_pixel_margin, 25);
tiny_skia_property_test!(css_pixel_margin_top, 26);
tiny_skia_property_test!(css_pixel_margin_right, 27);
tiny_skia_property_test!(css_pixel_margin_bottom, 28);
tiny_skia_property_test!(css_pixel_margin_left, 29);
tiny_skia_property_test!(css_pixel_font_size, 30);
tiny_skia_property_test!(css_pixel_color, 31);
tiny_skia_property_test!(css_pixel_font_family, 32);
tiny_skia_property_test!(css_pixel_font_weight, 33);
tiny_skia_property_test!(css_pixel_text_decoration_underline, 34);
tiny_skia_property_test!(css_pixel_text_decoration_line_through, 35);
tiny_skia_property_test!(css_pixel_z_index, 36);
tiny_skia_property_test!(css_pixel_flex_grow, 37);
tiny_skia_property_test!(css_pixel_flex_shrink, 38);
tiny_skia_property_test!(css_pixel_flex_basis, 39);
tiny_skia_property_test!(css_pixel_align_self, 40);
tiny_skia_property_test!(css_pixel_align_content, 41);
