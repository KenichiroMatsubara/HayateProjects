//! 共通 ScenePainter walk の allocation 回帰 probe（issue #852）。
//!
//! 実アプリ相当の `tasks_tree` を `NullPainter` で描画し、scene 構築や painter の
//! 出力バッファではなく `render_scene_graph` の区間だけを計測する。これにより
//! Group / Clip / ElementAnchor の子走査で frame-local clone が復活すると検出できる。

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use hayate_core::{render_scene_graph, Node, NodeKind, NullPainter};
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
    let _ = tree.render(0.0);
    let mut graph = tree.scene_graph().clone();
    let group = graph.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    });
    let clip = graph.insert_child(
        group,
        Node {
            kind: NodeKind::Clip {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                corner_radii: [0.0; 4],
            },
            children: Vec::new(),
        },
    );
    graph.insert_child(
        clip,
        Node {
            kind: NodeKind::Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                color: [0.0; 4],
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );

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
