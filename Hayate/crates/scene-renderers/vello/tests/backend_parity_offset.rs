//! Backend parity: Vello must paint transformed, non-origin geometry at the
//! same physical position the math predicts AND the same place TinySkia does.
//! The existing content_scale test only checks a rect at the origin, where any
//! additive translation error is invisible (0*s == 0); these cover translate,
//! non-integer dpr far from origin, and scale-about-center nested under a clip
//! (the #246 "live POP cards" shape) so a renderer-side offset can't hide.

use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_test_support::vello::{render_scene_to_pixels_scaled, try_vello_harness};

fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 180 && p[1] < 80 && p[2] < 80
}

/// Scan for the bounding box of red pixels: (min_x, min_y, max_x, max_y).
fn red_bbox(data: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut minx, mut miny, mut maxx, mut maxy) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            if is_red(pixel(data, width, x, y)) {
                found = true;
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
    }
    found.then_some((minx, miny, maxx, maxy))
}

/// A red rect at (rx,ry,30,30) wrapped in a Group translate(dx,dy).
fn grouped_rect_scene(dx: f64, dy: f64, rx: f32, ry: f32) -> SceneGraph {
    let mut scene = SceneGraph::new();
    let group = scene.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, dx, dy],
        },
        children: Vec::new(),
    });
    scene.insert_child(
        group,
        Node {
            kind: NodeKind::Rect {
                x: rx,
                y: ry,
                width: 30.0,
                height: 30.0,
                color: [1.0, 0.0, 0.0, 1.0],
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );
    scene
}

#[test]
fn vello_paints_grouped_rect_at_predicted_position() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };

    // content_scale = 2. Rect (10,10)-(40,40) inside Group translate(20,20).
    // Expected physical bbox: ((10+20)*2, ..) = (60,60)-(120,120).
    let scene = grouped_rect_scene(20.0, 20.0, 10.0, 10.0);
    let pixels = render_scene_to_pixels_scaled(&mut harness, &scene, 200, 200, 2.0)
        .expect("vello render");
    let bbox = red_bbox(&pixels, 200, 200).expect("no red painted at all");
    eprintln!("scale2 group translate: bbox = {bbox:?}, expected ~(60,60,120,120)");

    let (minx, miny, maxx, maxy) = bbox;
    let tol = 2;
    assert!((minx as i32 - 60).abs() <= tol, "minx {minx} != ~60");
    assert!((miny as i32 - 60).abs() <= tol, "miny {miny} != ~60");
    assert!((maxx as i32 - 120).abs() <= tol, "maxx {maxx} != ~120");
    assert!((maxy as i32 - 120).abs() <= tol, "maxy {maxy} != ~120");
}

#[test]
fn vello_vs_tinyskia_bbox_parity_noninteger_dpr_far_from_origin() {
    use hayate_scene_test_support::tiny_skia::render_scene_to_pixels_scaled as ts_scaled;
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    // dpr = 1.5, rect far from origin: (120,120)-(150,150) + translate(20,20)
    // Expected physical: ((120+20)*1.5, ..) = (210,210)-(255,255).
    let scene = grouped_rect_scene(20.0, 20.0, 120.0, 120.0);
    let (w, h, s) = (300u32, 300u32, 1.5f32);

    let vello_px = render_scene_to_pixels_scaled(&mut harness, &scene, w, h, s).expect("vello");
    let ts_px = ts_scaled(&scene, w, h, s);

    let vbox = red_bbox(&vello_px, w, h).expect("vello: no red");
    let tbox = red_bbox(&ts_px, w, h).expect("tinyskia: no red");
    eprintln!("dpr1.5 far: vello={vbox:?} tinyskia={tbox:?} expected~(210,210,255,255)");

    let tol = 2i32;
    for (v, t) in [
        (vbox.0, tbox.0),
        (vbox.1, tbox.1),
        (vbox.2, tbox.2),
        (vbox.3, tbox.3),
    ] {
        assert!(
            (v as i32 - t as i32).abs() <= tol,
            "vello/tinyskia bbox diverge: vello={vbox:?} tinyskia={tbox:?}"
        );
    }
}

/// Scale transform around a non-origin center (CSS `transform: scale(s)` with
/// transform-origin at the element center), nested under a clip, with
/// content_scale — mirrors the #246 "live POP cards" hover animation.
fn popcard_scene(s: f64, cx: f64, cy: f64) -> SceneGraph {
    // matrix that scales by s about (cx,cy): [s,0,0,s, cx-s*cx, cy-s*cy]
    let mut scene = SceneGraph::new();
    let clip = scene.insert(Node {
        kind: NodeKind::Clip {
            x: 100.0,
            y: 100.0,
            width: 80.0,
            height: 80.0,
            corner_radii: [0.0; 4],
        },
        children: Vec::new(),
    });
    let group = scene.insert_child(
        clip,
        Node {
            kind: NodeKind::Group {
                transform: [s, 0.0, 0.0, s, cx - s * cx, cy - s * cy],
            },
            children: Vec::new(),
        },
    );
    scene.insert_child(
        group,
        Node {
            kind: NodeKind::Rect {
                x: 110.0,
                y: 110.0,
                width: 60.0,
                height: 60.0,
                color: [1.0, 0.0, 0.0, 1.0],
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );
    scene
}

#[test]
fn vello_vs_tinyskia_scale_about_center_nested_under_clip() {
    use hayate_scene_test_support::tiny_skia::render_scene_to_pixels_scaled as ts_scaled;
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    // scale 1.2 about card center (140,140), content_scale 2.0
    let scene = popcard_scene(1.2, 140.0, 140.0);
    let (w, h, s) = (400u32, 400u32, 2.0f32);
    let vbox = red_bbox(&render_scene_to_pixels_scaled(&mut harness, &scene, w, h, s).unwrap(), w, h)
        .expect("vello: no red");
    let tbox = red_bbox(&ts_scaled(&scene, w, h, s), w, h).expect("ts: no red");
    eprintln!("popcard scale1.2: vello={vbox:?} tinyskia={tbox:?}");
    let tol = 2i32;
    for (v, t) in [(vbox.0, tbox.0), (vbox.1, tbox.1), (vbox.2, tbox.2), (vbox.3, tbox.3)] {
        assert!((v as i32 - t as i32).abs() <= tol,
            "scale-about-center diverges: vello={vbox:?} tinyskia={tbox:?}");
    }
}

#[test]
fn vello_paints_grouped_rect_scale1() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    // scale 1: rect (10,10)-(40,40) + translate(20,20) -> (30,30)-(60,60)
    let scene = grouped_rect_scene(20.0, 20.0, 10.0, 10.0);
    let pixels = render_scene_to_pixels_scaled(&mut harness, &scene, 200, 200, 1.0)
        .expect("vello render");
    let bbox = red_bbox(&pixels, 200, 200).expect("no red painted");
    eprintln!("scale1 group translate: bbox = {bbox:?}, expected ~(30,30,60,60)");
    let (minx, miny, _, _) = bbox;
    let tol = 2;
    assert!((minx as i32 - 30).abs() <= tol, "minx {minx} != ~30");
    assert!((miny as i32 - 30).abs() <= tol, "miny {miny} != ~30");
}
