//! 共通 ScenePainter walk の allocation 回帰 probe（issue #852）。
//!
//! 実アプリ相当の `tasks_tree` を `NullPainter` で描画し、scene 構築や painter の
//! 出力バッファではなく `render_scene_graph` の区間だけを計測する。これにより
//! Group / Clip / ElementAnchor の子走査で frame-local clone が復活すると検出できる。

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use hayate_core::{render_scene_graph, NodeKind, NullPainter, OverflowValue, StyleProp};
use hayate_demo_fixtures::tasks_tree;

struct ThreadTrackingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: ThreadTrackingAllocator = ThreadTrackingAllocator;

unsafe impl GlobalAlloc for ThreadTrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: this adapter preserves `System`'s allocation contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: this adapter preserves `System`'s allocation contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: this adapter preserves `System`'s reallocation contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: this adapter preserves `System`'s deallocation contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

fn record_allocation() {
    TRACK_ALLOCATIONS.with(|tracking| {
        if tracking.get() {
            ALLOCATION_COUNT.with(|count| count.set(count.get() + 1));
        }
    });
}

fn allocation_count(f: impl FnOnce()) -> usize {
    ALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    f();
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
    ALLOCATION_COUNT.with(Cell::get)
}

#[test]
fn tasks_scene_walk_allocates_nothing_for_structural_child_traversal() {
    let mut tree = tasks_tree("allocation-probe");
    let root = tree.root().expect("tasks fixture root");
    tree.element_set_transform(root, Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    tree.element_set_style(root, &[StyleProp::Overflow(OverflowValue::Hidden)]);
    let _ = tree.render(0.0);
    let graph = tree.scene_graph();

    let (groups, clips, anchors) = graph.iter().fold(
        (0usize, 0usize, 0usize),
        |(groups, clips, anchors), (_, node)| match node.kind {
            NodeKind::Group { .. } => (groups + 1, clips, anchors),
            NodeKind::Clip { .. } => (groups, clips + 1, anchors),
            NodeKind::ElementAnchor { .. } => (groups, clips, anchors + 1),
            _ => (groups, clips, anchors),
        },
    );
    assert!(groups > 0, "fixture must exercise Group child traversal");
    assert!(clips > 0, "fixture must exercise Clip child traversal");
    assert!(
        anchors > 0,
        "fixture must exercise ElementAnchor child traversal"
    );

    // Warm the call path before measuring so one-time runtime setup is excluded.
    render_scene_graph(&graph, &mut NullPainter);

    let allocations = allocation_count(|| render_scene_graph(&graph, &mut NullPainter));

    assert_eq!(
        allocations, 0,
        "scene walk allocated while visiting {groups} groups, {clips} clips, and {anchors} anchors"
    );
}

#[test]
fn steady_state_text_resource_lookup_allocates_nothing() {
    let mut tree = tasks_tree("text-resource-allocation-probe");
    let _ = tree.render(0.0);
    let graph = tree.scene_graph();
    let text_runs: Vec<_> = graph
        .iter()
        .filter_map(|(_, node)| match node.kind {
            NodeKind::TextRun { text_run, .. } => Some(text_run),
            _ => None,
        })
        .collect();
    assert!(
        !text_runs.is_empty(),
        "representative tasks fixture must contain shaped text"
    );

    for &text_run in &text_runs {
        let run = graph
            .resources()
            .text_run(text_run)
            .expect("scene text identity resolves");
        graph
            .resources()
            .font_instance(run.font_instance)
            .expect("scene font identity resolves");
    }

    let allocations = allocation_count(|| {
        for &text_run in &text_runs {
            let run = graph.resources().text_run(text_run).unwrap();
            let font = graph.resources().font_instance(run.font_instance).unwrap();
            std::hint::black_box((run.glyphs.len(), font.normalized_coords.len()));
        }
    });

    assert_eq!(
        allocations, 0,
        "fixed-size text/font ID lookup must not clone vectors, rebuild cache keys, or allocate"
    );
}
