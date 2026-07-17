use crate::cases::{render_tree_to_scene, CssPixelCase};
use crate::tiny_skia;
use crate::vello::{self, VelloHarness};

pub fn run_tiny_skia(case: &CssPixelCase) {
    let sg = render_tree_to_scene((case.build)());
    let pixels = tiny_skia::render_scene_to_pixels(&sg);
    (case.check)(&pixels);
}

pub fn run_vello(case: &CssPixelCase, harness: &mut VelloHarness) {
    let sg = render_tree_to_scene((case.build)());
    let pixels = vello::render_scene_to_pixels(harness, &sg)
        .unwrap_or_else(|| panic!("vello render failed for {}", case.css_property));
    (case.check)(&pixels);
}

/// 全ケースを実行する。CI で読みやすいよう、プロパティ名を付けて再パニックする。
pub fn run_all_tiny_skia(cases: &[CssPixelCase]) {
    for case in cases {
        let prop = case.css_property;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_tiny_skia(case);
        }));
        if let Err(payload) = result {
            std::panic::resume_unwind(match payload.downcast::<String>() {
                Ok(s) => Box::new(format!("{prop}: {s}")),
                Err(p) => p,
            });
        }
    }
}

pub fn run_all_vello(cases: &[CssPixelCase]) -> bool {
    let Some(mut harness) = vello::try_vello_harness() else {
        eprintln!("css pixel tests (vello): skipped — no wgpu adapter available");
        return false;
    };
    for case in cases {
        let prop = case.css_property;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_vello(case, &mut harness);
        }));
        if let Err(payload) = result {
            std::panic::resume_unwind(match payload.downcast::<String>() {
                Ok(s) => Box::new(format!("{prop}: {s}")),
                Err(p) => p,
            });
        }
    }
    true
}
