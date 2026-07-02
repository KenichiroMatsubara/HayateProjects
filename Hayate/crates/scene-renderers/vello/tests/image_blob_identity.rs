//! Issue #630: 画像 Blob id の安定性。
//!
//! vello の画像アトラス（`vello_encoding::ImageCache`）は Blob id をキーに常駐管理
//! するため、同一の `RenderImage` が生きている間はエンコードのたびに同一の Blob id
//! が vello へ渡らなければならない。id が毎フレーム変わると、変化のない画像でも
//! 毎フレーム「CPU フルコピー → アトラス再割り当て → GPU 再アップロード」が走る。

use std::sync::Arc;

use hayate_core::{
    Blob, Node, NodeKind, RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph,
};
use hayate_scene_renderer_vello::debug_encode_scene;
use vello_encoding::Patch;

fn test_image() -> RenderImage {
    // 2x2 RGBA8
    let pixels: Vec<u8> = vec![
        255, 0, 0, 255, //
        0, 255, 0, 255, //
        0, 0, 255, 255, //
        255, 255, 255, 255,
    ];
    RenderImage {
        width: 2,
        height: 2,
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        data: Blob::from(pixels),
    }
}

fn scene_graph_with(image: Arc<RenderImage>) -> SceneGraph {
    let mut graph = SceneGraph::new();
    graph.insert(Node {
        kind: NodeKind::Image {
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
            data: image,
        },
        children: Vec::new(),
    });
    graph
}

/// エンコード結果から画像 patch の Blob id を取り出す。
fn image_blob_ids(scene: &vello::Scene) -> Vec<u64> {
    scene
        .encoding()
        .resources
        .patches
        .iter()
        .filter_map(|patch| match patch {
            Patch::Image { image, .. } => Some(image.data.id()),
            _ => None,
        })
        .collect()
}

/// エンコード結果から画像 patch のピクセルデータ先頭ポインタを取り出す。
fn image_data_ptrs(scene: &vello::Scene) -> Vec<*const u8> {
    scene
        .encoding()
        .resources
        .patches
        .iter()
        .filter_map(|patch| match patch {
            Patch::Image { image, .. } => Some(image.data.data().as_ptr()),
            _ => None,
        })
        .collect()
}

#[test]
fn same_render_image_yields_same_blob_id_across_frames() {
    let image = Arc::new(test_image());
    let graph = scene_graph_with(image);

    // 2 フレーム連続でエンコード（present ごとに Scene は作り直される）。
    let frame1 = debug_encode_scene(&graph, 1.0);
    let frame2 = debug_encode_scene(&graph, 1.0);

    let ids1 = image_blob_ids(&frame1);
    let ids2 = image_blob_ids(&frame2);
    assert_eq!(ids1.len(), 1, "expected exactly one image patch in frame 1");
    assert_eq!(ids2.len(), 1, "expected exactly one image patch in frame 2");
    assert_eq!(
        ids1[0], ids2[0],
        "same RenderImage must present the same Blob id to vello across frames \
         (otherwise the image atlas misses every frame and re-uploads to the GPU)"
    );
}

#[test]
fn distinct_render_image_yields_distinct_blob_id() {
    // 内容が実際に変わった画像（別の RenderImage インスタンス）は id が変わり、
    // 従来どおりアトラスへ再アップロードされる。
    let frame1 = debug_encode_scene(&scene_graph_with(Arc::new(test_image())), 1.0);
    let frame2 = debug_encode_scene(&scene_graph_with(Arc::new(test_image())), 1.0);

    let ids1 = image_blob_ids(&frame1);
    let ids2 = image_blob_ids(&frame2);
    assert_eq!(ids1.len(), 1);
    assert_eq!(ids2.len(), 1);
    assert_ne!(
        ids1[0], ids2[0],
        "a new RenderImage instance must present a new Blob id so changed content re-uploads"
    );
}

#[test]
fn encoding_shares_pixel_buffer_without_copying() {
    let image = Arc::new(test_image());
    let source_ptr = image.data.data().as_ptr();
    let graph = scene_graph_with(image);

    let frame = debug_encode_scene(&graph, 1.0);

    let ptrs = image_data_ptrs(&frame);
    assert_eq!(ptrs.len(), 1, "expected exactly one image patch");
    assert_eq!(
        ptrs[0], source_ptr,
        "vello must see the RenderImage's own pixel buffer, not a per-frame full copy"
    );
}
