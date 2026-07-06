use crate::color::Color;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::style::{BorderStyleValue, OverflowValue, Shadow};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::pointer::PointerKind;
use crate::element::scene_lowering::{
    clear_lowered_content, AnchorEntry, LoweringDirtySnapshot, SceneLowering,
};
use crate::element::visual_invalidation::{self, VisualInvalidationReach};
use crate::element::taffy_projection::TraversalStep;
use crate::element::tree::{ElementTree, Visual};
use crate::node::{Node, NodeId, NodeKind, SceneGraph, ShadowOccluder};
use crate::render::shadow::{shadow_sigma, HARD_SHADOW_BLUR_THRESHOLD};
use std::collections::HashSet;

use crate::scroll::{overscroll_stretch_scale, ScrollPhysicsProfile, ScrollPhysicsTuning};

/// 恒等 2x3 アフィン。scroll group がこれに等しいとき（オフセット 0・stretch 無し）は
/// group ノードを省く。
const IDENTITY_AFFINE: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// scroll group の 2x3 アフィン `[a, b, c, d, e, f]`（`x' = a·x + c·y + e`, `y' = b·x + d·y + f`）を
/// 稼働 Scroll Physics Profile に応じて計算する（ADR-0131）。**プロファイル差はこの一箇所に閉じる。**
///
/// iOS 風（`Auto`）は overshoot 込みで content を丸ごと `translate(-offset)`（scale 無し）——従来
/// 挙動を厳密に維持する。Android 風 stretch（`Android`）は overshoot を **軸ごとに独立して**
/// 「端クランプ translate ＋ ピン端アンカー scale」へ分割する: 限界に達した端をビューポート境界に
/// ピン留めしたまま（clamp した translate）、コンテンツを内側へ一様スケール（[`overscroll_stretch_scale`]）
/// で伸ばす。スケールは `max > 0` の軸だけに掛かる（軸独立: 縦のみのページは横に伸びない）。
///
/// `offset` は表示スクロールオフセット（越境中は `[0, max]` 外）、`max` は軸ごとの最大オフセット、
/// `viewport` はシーン座標の ScrollView ボックス `(x, y, w, h)`。物理・保存 `scroll_offset` は不変で、
/// ここは既存の越境変位を read し替えて見せ方だけを変える。
pub(crate) fn scroll_group_affine(
    profile: ScrollPhysicsProfile,
    offset: (f32, f32),
    max: (f32, f32),
    viewport: (f32, f32, f32, f32),
    t: &ScrollPhysicsTuning,
) -> [f64; 6] {
    let (sx, sy) = offset;
    if !profile.uses_stretch_overscroll() {
        // iOS 風: overshoot 込みで丸ごと translate、scale 無し。
        return [1.0, 0.0, 0.0, 1.0, -sx as f64, -sy as f64];
    }
    let (vx, vy, vw, vh) = viewport;
    let (scale_x, tx) = stretch_axis(sx, max.0, vx, vx + vw, vw, t);
    let (scale_y, ty) = stretch_axis(sy, max.1, vy, vy + vh, vh, t);
    [scale_x, 0.0, 0.0, scale_y, tx, ty]
}

/// Android stretch の 1 軸分の `(scale, translate)`（ADR-0131）。範囲内 or スクロール不可（`max <= 0`）
/// なら scale 1.0・素の `translate(-offset)`（iOS とパリティ）。越境時はピン端（`near`/`far` の
/// シーン座標ビューポート境界）を固定するアンカー scale を、端クランプ translate に畳み込む:
///   `final = scale·(content − clamped − anchor) + anchor = scale·content + (−scale·clamped + anchor·(1 − scale))`
fn stretch_axis(offset: f32, max: f32, near: f32, far: f32, dimension: f32, t: &ScrollPhysicsTuning) -> (f64, f64) {
    let max = max.max(0.0);
    if max <= 0.0 {
        // スクロール不可な軸は伸ばさない（軸独立）。iOS と同じ素の translate。
        return (1.0, -offset as f64);
    }
    let clamped = offset.clamp(0.0, max);
    let overshoot = offset - clamped;
    if overshoot == 0.0 {
        // 範囲内: clamped == offset なので iOS と同一の translate。
        return (1.0, -clamped as f64);
    }
    let scale = overscroll_stretch_scale(overshoot, dimension, t) as f64;
    // ピン端アンカー: 上/左越境（overshoot < 0）は near 端、下/右越境は far 端に固定。
    let anchor = if overshoot < 0.0 { near } else { far } as f64;
    let translate = -scale * clamped as f64 + anchor * (1.0 - scale);
    (scale, translate)
}

/// `:focus-visible` のフォーカスリング幅（ADR-0102）。Chromium はボーダーボックスの
/// すぐ外側に角丸に沿った実線リングを描く。Chrome の既定 `outline: auto` 相当。
pub const FOCUS_RING_WIDTH: f32 = 2.0;
/// ボーダーボックスとリング内縁の隙間。
pub const FOCUS_RING_OFFSET: f32 = 1.0;
/// Chromium 既定のアクセントフォーカスリング色（Google Blue、不透明）。
pub const FOCUS_RING_COLOR: Color = Color::new(0.102, 0.451, 0.910, 1.0);

/// スクロールバーオーバーレイ（ADR-0110）。`ScrollView` のオーバーフロー軸ごとに、
/// Scroll Offset とコンテンツサイズから導いた常時表示の Mouse/Pen 用サムをコンテンツ上に描く。
/// オーバーレイ描画でレイアウト領域を占有しない（ガター無し）ためコンテンツボックスを縮めない。
///
/// 以降のチューニング値はフォーカスリングや選択クロームと同様、scene-build パスに
/// インラインのマジックナンバーを置かないための名前付き定数（ADR-0102）。
///
/// スクロールバー本体の太さ（横断方向の長さ）。
pub const SCROLLBAR_THICKNESS: f32 = 6.0;
/// トラック（およびサム）のスクロールビューボックス端からのインセット。
pub const SCROLLBAR_TRACK_MARGIN: f32 = 2.0;
/// スクロール軸方向のサム長の下限。背の高い／幅広いコンテンツでも掴める長さを残す。
pub const SCROLLBAR_MIN_THUMB_LENGTH: f32 = 24.0;
/// サムの塗り色（RGB）。[`SCROLLBAR_THUMB_OPACITY`] でコンテンツ上に合成する。
pub const SCROLLBAR_THUMB_COLOR: Color = Color::new(0.0, 0.0, 0.0, 1.0);
/// サムの不透明度（オーバーレイとしての透け具合）。
pub const SCROLLBAR_THUMB_OPACITY: f32 = 0.4;
/// トラックマージンの1クリックで進む Scroll Offset 距離（ページ送り、ADR-0110）。
pub const SCROLLBAR_PAGE_STEP: f32 = 240.0;

/// Touch の一時インジケータの寸法とフェードタイミング（ADR-0110）。
/// Touch 形態はスクロール中に現れ停止後にフェードする非操作インジケータ（Android ネイティブ、
/// ADR-0087）。Mouse/Pen の操作可能サムより細く、ヒット領域を持たない。
///
/// インジケータバーの横断方向の長さ（[`SCROLLBAR_THICKNESS`] より細い）。
pub const SCROLLBAR_INDICATOR_THICKNESS: f32 = 4.0;
/// インジケータの塗り色（RGB）。[`SCROLLBAR_INDICATOR_OPACITY`] に現在のフェード係数を掛けて合成。
pub const SCROLLBAR_INDICATOR_COLOR: Color = Color::BLACK;
/// フェード前の完全表示時の不透明度。
pub const SCROLLBAR_INDICATOR_OPACITY: f32 = 0.4;
/// 最後のスクロール後、フェード開始までフル表示を保つ時間（ホールド窓）。
pub const SCROLLBAR_INDICATOR_HOLD_MS: f64 = 600.0;
/// ホールド窓経過後、フルから不可視までフェードに要する時間。
pub const SCROLLBAR_INDICATOR_FADE_MS: f64 = 400.0;

/// スクロールバーのサムが滑る軸。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// オーバーフロー軸1本の Mouse/Pen スクロールバーのキャンバス座標ジオメトリ。ボックス矩形・
/// Scroll Offset・コンテンツサイズから導く（ADR-0110）。オーバーレイ描画
/// (`emit_scrollbar_overlay`) とポインタヒットテスト (`interaction.rs`) が共有する単一の源で、
/// 押下が見えているサムに正確に当たり、操作が描画と同じ offset へ戻る。
#[derive(Clone, Copy, Debug)]
pub struct ScrollbarAxisGeometry {
    pub axis: ScrollAxis,
    /// サム矩形 `(x, y, w, h)`（キャンバス座標）。
    pub thumb: (f32, f32, f32, f32),
    /// トラック矩形 `(x, y, w, h)`（キャンバス座標、スライド可能な全幅）。
    pub track: (f32, f32, f32, f32),
    /// この軸の最大 Scroll Offset。
    pub max_offset: f32,
    /// サムのスライド可能量（トラックpx、`track_len − thumb_len`）。サムがトラックを満たすと0。
    /// ドラッグのトラックpx差分を offset 差分へ写す。
    pub thumb_travel: f32,
}

/// `id` のオーバーフロー軸ごとのスクロールバージオメトリ（キャンバス座標）。ポインタヒットテスト
/// (`interaction.rs`) が読む公開シーム。`ScrollView` でない・未レイアウト・コンテンツが収まる場合は空。
/// 要素自身のレイアウト矩形から計算し、オーバーレイ描画と一致させる。
pub fn scrollbar_axes(tree: &ElementTree, id: ElementId) -> Vec<ScrollbarAxisGeometry> {
    if tree.element_kind(id) != Some(ElementKind::ScrollView) {
        return Vec::new();
    }
    let Some((x, y, w, h)) = tree.element_layout_rect(id) else {
        return Vec::new();
    };
    scrollbar_axes_in_box(tree, id, x, y, w, h)
}

/// 両アンカー戦略が共有する単一のシーンウォークを通して回す環境コンテキスト。戦略に依らず
/// 全エミッションが必要とするもの——ドキュメントツリー、構築先のシーングラフ、effective-visual
/// 解決を駆動する interaction スナップショット（ADR-0067）、ノードごとの絶対原点＋継承コンテキスト
/// ——を保持する。戦略固有の状態（retained のアンカー/時計、ephemeral は無し）は隣を回す
/// [`AnchorSink`] 側にあり、ここには無い。子への下降は [`WalkCtx::child`]。
struct WalkCtx<'a> {
    tree: &'a ElementTree,
    interaction: &'a crate::element::pseudo_state::InteractionSnapshot,
    sg: &'a mut SceneGraph,
    /// 子がレイアウトされる絶対原点（親ボックスの左上）。
    ox: f32,
    oy: f32,
    inherited: InheritedVisualContext,
}

impl WalkCtx<'_> {
    /// 子へ下降するための再借用。環境フィールド（tree, sg, interaction）はそのまま引き継ぎ、
    /// カーソル（原点・継承コンテキスト）を差し替える。
    fn child(&mut self, ox: f32, oy: f32, inherited: InheritedVisualContext) -> WalkCtx<'_> {
        WalkCtx {
            tree: self.tree,
            interaction: self.interaction,
            sg: &mut *self.sg,
            ox,
            oy,
            inherited,
        }
    }
}

/// 共有シーンウォークが要素のエミッション内容をどう接続するか。
///
/// エミッション本体——transform/clip ラッパ、box shadow、ビジュアルボックス、image/text/text-input
/// ラン——は retained のインクリメンタル lowering（ADR-0086）と golden-frame パリティを支える
/// ephemeral の全再構築（ADR-0079）で同一。違いは接続方法だけ：retained は永続 `ElementAnchor`
/// を付け直し、記憶した値に対して進行中のトランジションを補間する（ADR-0093）。ephemeral は
/// 親グループ下に新規ノードを出して解決済みターゲットを直接描く。各々がこのシームの一アダプタ
/// ([`RetainedSink`] / [`EphemeralSink`]) なので、エミッション修正は一箇所で済む。
trait AnchorSink {
    /// 戦略がウォークに沿って回すノードごとのカーソル。retained は接続先の親＋再lowering の `reach`、
    /// ephemeral は親グループのみ。
    type Cursor: Copy;

    /// 各訪問ノード（スキップ/None 含む）で作業前に1回呼ばれる。retained のウォーク数計上シーム
    /// （ADR-0086「クリーンフレーム ⇒ ウォーク0」）。
    fn enter_node(&mut self);

    /// 要素 `id` 自身の内容を出すシーンノード（`effective_parent` の種）を確立する。retained は
    /// 永続アンカーを確保し旧内容をクリア、ephemeral はカーソルの親グループを転送する。
    fn begin(&mut self, ctx: &mut WalkCtx, cursor: Self::Cursor, id: ElementId) -> Option<NodeId>;

    /// 実際に描かれるビジュアル。retained は `resolved` をアンカーの記憶表示値に対し補間、
    /// ephemeral は `resolved` をそのまま描く。
    fn displayed(&mut self, id: ElementId, resolved: Visual) -> Visual;

    /// 再帰する子と各子のカーソル。`effective_parent` 下に接続する。retained は `reach` で絞り、
    /// ephemeral は順序付き全子を取る。
    fn children(
        &self,
        tree: &ElementTree,
        cursor: Self::Cursor,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, Self::Cursor)>;

    /// この要素の内容と子を出し終えた後の子配置の確定。retained は内容の後ろに子アンカーを
    /// 積み直す、ephemeral は no-op（新規ノードは既に描画順）。
    fn end_element(&mut self, ctx: &mut WalkCtx, effective_parent: Option<NodeId>, id: ElementId);
}

/// [`RetainedSink`] のノードごとカーソル。この要素の接続先と再lowering reach の残り伝播範囲。
#[derive(Clone, Copy)]
struct RetainedCursor {
    parent_anchor: Option<NodeId>,
    reach: VisualInvalidationReach,
}

/// retained インクリメンタル lowering アダプタ（ADR-0086）。永続 `ElementAnchor` の付け直しと、
/// アンカーが保持する表示値に対するトランジション補間（ADR-0093）。
struct RetainedSink<'a> {
    lowering: &'a mut SceneLowering,
    now_ms: f64,
}

impl AnchorSink for RetainedSink<'_> {
    type Cursor = RetainedCursor;

    fn enter_node(&mut self) {
        self.lowering.walk_count += 1;
    }

    fn begin(&mut self, ctx: &mut WalkCtx, cursor: RetainedCursor, id: ElementId) -> Option<NodeId> {
        let tree = ctx.tree;
        let anchor_id = ensure_anchor(tree, ctx.sg, self.lowering, id, cursor.parent_anchor);
        let children = tree.ordered_children(id);
        clear_lowered_content(ctx.sg, anchor_id, &children, self.lowering);
        Some(anchor_id)
    }

    fn displayed(&mut self, id: ElementId, resolved: Visual) -> Visual {
        // 変更後の解決ビジュアルを前フレームの表示値と resolve シームで差分し、変化した連続
        // プロパティを補間する（ADR-0093）。retained アンカーは変更前の値を保持する。
        self.lowering
            .anchors
            .get_mut(&id)
            .map(|entry| entry.resolve_displayed(&resolved, self.now_ms))
            .unwrap_or(resolved)
    }

    fn children(
        &self,
        tree: &ElementTree,
        cursor: RetainedCursor,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, RetainedCursor)> {
        visual_invalidation::children_for_reach(tree, id, cursor.reach)
            .into_iter()
            .map(|(child, reach)| {
                (
                    child,
                    RetainedCursor {
                        parent_anchor: effective_parent,
                        reach,
                    },
                )
            })
            .collect()
    }

    fn end_element(&mut self, ctx: &mut WalkCtx, effective_parent: Option<NodeId>, id: ElementId) {
        let tree = ctx.tree;
        reparent_child_anchors_under(
            ctx.sg,
            effective_parent,
            &tree.ordered_children(id),
            self.lowering,
        );
    }
}

/// ephemeral 全再構築アダプタ（ADR-0079 golden-frame パリティ）。親グループ下に新規ノードを出し、
/// アンカーも補間も持たない。
struct EphemeralSink;

impl AnchorSink for EphemeralSink {
    /// 子ノードを接続する親グループ。
    type Cursor = Option<NodeId>;

    fn enter_node(&mut self) {}

    fn begin(&mut self, _ctx: &mut WalkCtx, cursor: Option<NodeId>, _id: ElementId) -> Option<NodeId> {
        cursor
    }

    fn displayed(&mut self, _id: ElementId, resolved: Visual) -> Visual {
        // 全再構築には retained の `last_displayed` が無いので補間せず、解決済みターゲットを
        // 直接描く（ADR-0093）。
        resolved
    }

    fn children(
        &self,
        tree: &ElementTree,
        _cursor: Option<NodeId>,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, Option<NodeId>)> {
        tree.ordered_children(id)
            .into_iter()
            .map(|child| (child, effective_parent))
            .collect()
    }

    fn end_element(&mut self, _ctx: &mut WalkCtx, _effective_parent: Option<NodeId>, _id: ElementId) {}
}

/// retained アンカー無しの ephemeral 全再構築（パリティ参照／テスト用）。
pub fn build_ephemeral(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    let interaction = tree.interaction_snapshot();
    if let Some(root) = tree.root() {
        let mut sink = EphemeralSink;
        let mut ctx = WalkCtx {
            tree,
            interaction: &interaction,
            sg: &mut sg,
            ox: 0.0,
            oy: 0.0,
            inherited: InheritedVisualContext::root(),
        };
        walk(&mut ctx, &mut sink, None, root);
    }
    // 選択クロームはドキュメントレベルのオーバーレイとして最前面に浮く（ADR-0097）。
    // 先にドラッグハンドル、その上にツールバー。
    if let Some(handles) = tree.selection_handles() {
        emit_selection_handles(&mut sg, &handles);
    }
    if let Some(toolbar) = tree.selection_toolbar() {
        emit_selection_toolbar(&mut sg, tree, &toolbar);
    }
    sg
}

/// retained 要素アンカーを使ってシーングラフをインクリメンタルに更新する。
///
/// `now_ms` は進行中トランジションを駆動するホスト時計。要素ごとの `resolve_effective` シームが
/// 解決ビジュアルを保持表示値と差分し、補間を開始/進行させる（ADR-0093）。
pub(crate) fn update(
    tree: &ElementTree,
    scene_cache: &mut SceneGraph,
    lowering: &mut SceneLowering,
    dirty: LoweringDirtySnapshot,
    now_ms: f64,
) {
    lowering.walk_count = 0;
    let interaction = tree.interaction_snapshot();

    if dirty.full_rebuild || !lowering.built {
        *scene_cache = SceneGraph::new();
        lowering.anchors.clear();
        if let Some(root) = tree.root() {
            let mut sink = RetainedSink {
                lowering: &mut *lowering,
                now_ms,
            };
            let mut ctx = WalkCtx {
                tree,
                interaction: &interaction,
                sg: &mut *scene_cache,
                ox: 0.0,
                oy: 0.0,
                inherited: InheritedVisualContext::root(),
            };
            walk(
                &mut ctx,
                &mut sink,
                RetainedCursor {
                    parent_anchor: None,
                    reach: VisualInvalidationReach::Subtree,
                },
                root,
            );
        }
        lowering.built = true;
        // 新しいグラフでは旧オーバーレイが落ちているので一から再エミットする。
        lowering.toolbar_root = None;
        lowering.handles_root = None;
        refresh_selection_chrome(tree, scene_cache, lowering);
        return;
    }

    if dirty.elements.is_empty() {
        // 要素の再描画が無くても選択（つまりそのクローム）は移動/クリアし得るので、
        // オーバーレイは常に更新する。
        refresh_selection_chrome(tree, scene_cache, lowering);
        return;
    }

    for &parent_id in &dirty.z_index_reorder_parents {
        reorder_children_for_z_index(tree, scene_cache, lowering, parent_id);
    }

    let patch_roots = visual_invalidation::minimal_patch_roots(tree, &dirty.elements);
    {
        let mut sink = RetainedSink {
            lowering: &mut *lowering,
            now_ms,
        };
        for patch_root in patch_roots {
            let reach = dirty
                .elements
                .get(&patch_root)
                .copied()
                .unwrap_or(VisualInvalidationReach::Subtree);
            let parent_anchor = tree
                .elements
                .get(&patch_root)
                .and_then(|el| el.parent)
                .and_then(|parent| sink.lowering.anchors.get(&parent).map(|entry| entry.anchor_id));
            let (ox, oy) = tree
                .elements
                .get(&patch_root)
                .and_then(|el| el.parent)
                .and_then(|parent| tree.layout.geometry(parent))
                .map(|(x, y, _, _)| (x, y))
                .unwrap_or((0.0, 0.0));
            let inherited = effective_visual::inherited_context_at(&tree.elements, patch_root);
            let mut ctx = WalkCtx {
                tree,
                interaction: &interaction,
                sg: &mut *scene_cache,
                ox,
                oy,
                inherited,
            };
            walk(
                &mut ctx,
                &mut sink,
                RetainedCursor {
                    parent_anchor,
                    reach,
                },
                patch_root,
            );
        }
    }
    refresh_selection_chrome(tree, scene_cache, lowering);
}

/// コア描画の選択オーバーレイを再エミットする（ADR-0097）。先にドラッグハンドル、次に浮動
/// ツールバー。ツールバーを最後に挿入することでハンドルの上に描かれる。
fn refresh_selection_chrome(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    refresh_selection_handles(tree, sg, lowering);
    refresh_selection_toolbar(tree, sg, lowering);
}

/// 選択ドラッグハンドルのオーバーレイを再エミットする（ADR-0097）。前回のオーバーレイ部分木を
/// 除去し、非崩壊の選択がアクティブなら新しいノブを描く。冪等：ハンドルが無ければ（除去以外は）no-op。
fn refresh_selection_handles(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    if let Some(prev) = lowering.handles_root.take() {
        sg.remove_subtree(prev);
    }
    let Some(handles) = tree.selection_handles() else {
        return;
    };
    lowering.handles_root = Some(emit_selection_handles(sg, &handles));
}

/// 選択ドラッグハンドルをトップレベルのオーバーレイ部分木へ lowering する。両端ごとに塗りつぶし
/// 円形ノブ（一辺の半分を角丸半径にした正方形）を1個持つ `Group`。クロームスタイルで色付け。
/// グループ id を返す。
fn emit_selection_handles(
    sg: &mut SceneGraph,
    handles: &crate::element::selection_chrome::SelectionHandles,
) -> NodeId {
    let color = handles.style.handle_color();
    let group = sg.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    });
    for handle in [&handles.start, &handles.end] {
        // しずく型 = 3 隅を丸め tip の隅だけ角にした角丸クリップ × ベタ塗り矩形
        // （Chrome Android お手本、ADR-0097）。`Rect` の単一 `corner_radius` では
        // 隅ごとに丸めを変えられないため、隅別の `Clip` で形を切り出す。
        let (x, y, w, h) = handle.draw_box();
        let clip = sg.insert_child(
            group,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: handle.corner_radii(),
                },
                children: Vec::new(),
            },
        );
        sg.insert_child(
            clip,
            Node {
                kind: NodeKind::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
    group
}

/// 浮動選択ツールバーのオーバーレイを再エミットする（ADR-0097）。前回のオーバーレイ部分木を
/// 除去し、選択がアクティブなら新しいものを最前面に描く。冪等：何も選択されていなければ（除去以外は）no-op。
fn refresh_selection_toolbar(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    if let Some(prev) = lowering.toolbar_root.take() {
        sg.remove_subtree(prev);
    }
    let Some(toolbar) = tree.selection_toolbar() else {
        return;
    };
    lowering.toolbar_root = Some(emit_selection_toolbar(sg, tree, &toolbar));
}

/// `[r,g,b,a]`（0..1）の tuning 色を core の [`Color`] へ。
fn toolbar_color(c: [f32; 4]) -> Color {
    Color::new(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64)
}

/// 角丸パネル（Material elevation の drop shadow ＋ 背景）を `group` に描く。影は既存の
/// box-shadow lowering（[`emit_box_shadows`]）を再利用し、新規描画プリミティブを足さない
/// （ADR-0095/0097）。影パラメータと色は tuning 由来、背景色はテーマ所有。
fn emit_toolbar_panel(
    sg: &mut SceneGraph,
    group: NodeId,
    style: crate::element::selection_chrome::SelectionChromeStyle,
    bounds: crate::element::selection_chrome::ToolbarRect,
    ct: &crate::element::chrome_tuning::ChromeTuning,
) {
    let shadow = Shadow {
        offset_x: 0.0,
        offset_y: ct.toolbar_elevation_offset_y,
        blur: ct.toolbar_elevation_blur,
        spread: ct.toolbar_elevation_spread,
        color: toolbar_color(ct.toolbar_shadow_color),
        inset: false,
    };
    emit_box_shadows(
        sg,
        Some(group),
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        ct.toolbar_corner_radius,
        std::slice::from_ref(&shadow),
        1.0,
        false,
        // ツールバーパネルの背景は直後に不透明で塗られるが、occluder 最適化はこの小さな
        // オーバーレイでは省く（正しさには影響しない・全面塗り）。
        None,
    );
    sg.insert_child(
        group,
        Node {
            kind: NodeKind::Rect {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
                // パネル/ラベルの色はテーマ所有（Material か Cupertino、ADR-0097）。
                // テーマ非依存の視覚値（角丸・elevation・divider）だけがチューニング値。
                color: style.toolbar_background(),
                corner_radius: ct.toolbar_corner_radius,
            },
            children: Vec::new(),
        },
    );
}

/// セル `cell` 内に中央寄せでシェイプ済みラベルのグリフ run を描く。
fn emit_toolbar_label(
    sg: &mut SceneGraph,
    group: NodeId,
    cell: crate::element::selection_chrome::ToolbarRect,
    label: &crate::element::text::TextLayout,
    color: [f32; 4],
) {
    let lx = cell.x + (cell.width - label.layout.width()) / 2.0;
    let ly = cell.y + (cell.height - label.layout.height()) / 2.0;
    for run in &label.runs {
        sg.insert_child(
            group,
            Node {
                kind: NodeKind::TextRun {
                    x: lx,
                    y: ly,
                    color,
                    data: run.clone(),
                },
                children: Vec::new(),
            },
        );
    }
}

/// 縦長の Material ディバイダ（境界 `cx` を中心に幅 `width`、`y..y+height` を覆う）を描く。
fn emit_toolbar_divider(
    sg: &mut SceneGraph,
    group: NodeId,
    cx: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    sg.insert_child(
        group,
        Node {
            kind: NodeKind::Rect {
                x: cx - width / 2.0,
                y,
                width,
                height,
                color,
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );
}

/// [`SelectionToolbar`] をトップレベルのオーバーレイ部分木へ lowering する（ADR-0097）。
/// drop shadow（elevation）・角丸背景パネル・ボタンラベル・ボタン間 Material ディバイダ、
/// そして畳みが起きていれば ⋮ オーバーフロートグルと（開いていれば）副メニューパネルを
/// 持つ `Group`。最後に挿入してドキュメントの上に描く。グループ id を返す。
fn emit_selection_toolbar(
    sg: &mut SceneGraph,
    tree: &ElementTree,
    toolbar: &crate::element::selection_chrome::SelectionToolbar,
) -> NodeId {
    let ct = tree.chrome_tuning();
    // オーバーレイのルートは Group。子は `insert_child` で挿入し、トップレベルのルートとしても
    // 登録されないようにする（さもないとルートとグループウォークで二重描画される）。
    let group = sg.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    });

    let b = toolbar.bounds;
    let label_color = toolbar.style.toolbar_label();
    // ディバイダ色は tuning 由来の非プリマルチプライ RGBA をそのまま Rect 色として使う。
    let divider_color = ct.toolbar_divider_color;

    // elevation + 背景パネル（影が最初なので背面に来る）。
    emit_toolbar_panel(sg, group, toolbar.style, b, ct);

    // 可視ボタンのラベル。
    for button in &toolbar.buttons {
        if let Some(label) = tree.toolbar_label_layout(button.action) {
            emit_toolbar_label(sg, group, button.bounds, label, label_color);
        }
    }

    // 可視行のボタン間に縦ディバイダ。境界 = 各ボタンの左端（先頭を除く）と、
    // オーバーフローがあれば ⋮ トグルの左端。
    for button in toolbar.buttons.iter().skip(1) {
        emit_toolbar_divider(sg, group, button.bounds.x, b.y, ct.toolbar_divider_width, b.height, divider_color);
    }
    if let Some(of) = &toolbar.overflow {
        if !toolbar.buttons.is_empty() {
            emit_toolbar_divider(sg, group, of.toggle.x, b.y, ct.toolbar_divider_width, b.height, divider_color);
        }
        // ⋮ トグルグリフ。
        if let Some(label) = tree.toolbar_overflow_label_layout() {
            emit_toolbar_label(sg, group, of.toggle, label, label_color);
        }
        // 開いていれば副メニューパネル（elevation 付き）＋畳まれた項目を縦積みで描く。
        if of.open {
            emit_toolbar_panel(sg, group, toolbar.style, of.panel, ct);
            for (i, item) in of.items.iter().enumerate() {
                if i > 0 {
                    // 縦積み項目間の横ディバイダ（境界 = 項目の上端）。
                    emit_toolbar_divider(
                        sg,
                        group,
                        item.bounds.x + item.bounds.width / 2.0,
                        item.bounds.y - ct.toolbar_divider_width / 2.0,
                        item.bounds.width,
                        ct.toolbar_divider_width,
                        divider_color,
                    );
                }
                if let Some(label) = tree.toolbar_label_layout(item.action) {
                    emit_toolbar_label(sg, group, item.bounds, label, label_color);
                }
            }
        }
    }

    group
}

fn reorder_children_for_z_index(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    parent_id: ElementId,
) {
    let Some(parent_entry) = lowering.anchors.get(&parent_id) else {
        return;
    };
    let parent_anchor = parent_entry.anchor_id;
    let ordered = tree.ordered_children(parent_id);
    let child_anchors: Vec<NodeId> = ordered
        .iter()
        .filter_map(|child| lowering.anchors.get(child).map(|e| e.anchor_id))
        .collect();
    if let Some(parent) = sg.get_mut(parent_anchor) {
        parent.children = child_anchors;
    }
}


fn first_child_matching(
    sg: &SceneGraph,
    parent: NodeId,
    pred: impl Fn(&NodeKind) -> bool,
) -> Option<NodeId> {
    let parent_node = sg.get(parent)?;
    parent_node.children.iter().copied().find(|&child| {
        sg.get(child).is_some_and(|n| pred(&n.kind))
    })
}

/// 子要素アンカーを接続すべきノード。親が ScrollView のときは Clip/scroll Group ラッパを辿る。
fn find_content_attachment_point(
    sg: &SceneGraph,
    anchor_id: NodeId,
    el: &crate::element::tree::Element,
) -> NodeId {
    let mut node = anchor_id;
    if el.transform.is_some() {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
            .unwrap_or(node);
    }
    if el.kind == ElementKind::ScrollView {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Clip { .. }))
            .unwrap_or(node);
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
                .unwrap_or(node);
        }
    }
    node
}

fn resolve_parent_attachment(
    tree: &ElementTree,
    sg: &SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> Option<NodeId> {
    let parent_id = tree.elements.get(&id).and_then(|el| el.parent)?;
    let parent_entry = lowering.anchors.get(&parent_id)?;
    let parent_el = tree.elements.get(&parent_id)?;
    Some(find_content_attachment_point(
        sg,
        parent_entry.anchor_id,
        parent_el,
    ))
    .or(parent_anchor)
}

fn ensure_anchor(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> NodeId {
    let attach_parent = resolve_parent_attachment(tree, sg, lowering, id, parent_anchor);
    if let Some(entry) = lowering.anchors.get(&id) {
        let anchor_id = entry.anchor_id;
        if let Some(parent) = attach_parent {
            insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
        }
        return anchor_id;
    }

    let anchor_id = sg.insert(Node {
        kind: NodeKind::ElementAnchor { element_id: id },
        children: Vec::new(),
    });
    if let Some(parent) = attach_parent {
        insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
    }
    lowering.anchors.insert(id, AnchorEntry::new(anchor_id));
    anchor_id
}

/// `child`（要素 `id` のアンカー）を `parent` 下に、`id` の兄弟内位置に合うシーン子インデックスへ接続する。
///
/// 部分パッチは親の子の一部だけを再ウォークする（例：ホバーされたカード、挿入で伸長/押された兄弟）。
/// 再ウォークしたアンカーを無条件に `parent.children` 末尾へ追加すると描画順が崩れ、操作した要素が
/// 別の兄弟の上に描かれる（「ホバー/クリックが別の要素を壊す」症状）。`parent` 下に実際に存在する
/// 先行兄弟アンカーを基準に配置することで、retained 子順を要素順と同期させ、Clip/Group の
/// 内容接続ラッパにも頑健（全兄弟が1つの接続点を共有）。
fn insert_anchor_ordered(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent: NodeId,
    child: NodeId,
) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&c| c != child);
        }
    }
    // 要素順で `id` より後ろの兄弟アンカー。`child` を `parent` 下に存在する最初の後続兄弟の
    // 直前に挿入。まだ存在しなければ末尾に追加。後続兄弟の前に挿入する（先行兄弟の後ろではなく）
    // ことで、親自身の内容ノード（どの子アンカーより前に出る fill/border）を全子の前に保ち、
    // ボックスが子の下に描かれる。
    let following: HashSet<NodeId> = tree
        .elements
        .get(&id)
        .and_then(|el| el.parent)
        .map(|p| tree.ordered_children(p))
        .unwrap_or_default()
        .into_iter()
        .skip_while(|&sib| sib != id)
        .skip(1)
        .filter_map(|sib| lowering.anchors.get(&sib).map(|e| e.anchor_id))
        .collect();
    if let Some(p) = sg.get_mut(parent) {
        let index = p
            .children
            .iter()
            .position(|c| following.contains(c))
            .unwrap_or(p.children.len());
        p.children.insert(index, child);
    }
}

fn attach_under(sg: &mut SceneGraph, parent: NodeId, child: NodeId) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&id| id != child);
        }
    }
    if let Some(p) = sg.get_mut(parent) {
        if !p.children.contains(&child) {
            p.children.push(child);
        }
    }
}

/// 再ウォークした要素の子アンカーを、自身の内容の後ろへ要素順で積み直す。`emit_element` は
/// ボックス自身の内容（fill/border/text）を追加で出すが、`clear_lowered_content` は子アンカーを
/// リスト先頭に保つ——このパスが無いとボックス自身の fill が子の上に描かれ、古い兄弟順も残る。
/// 内容エミット後に全子を要素順で付け直すことで `[content..., child0, child1, ...]` を復元する。
///
/// Clip/scroll-Group ラッパのケースも扱う：`effective_parent` がアンカー内のラッパなら、子は
/// ラッパ下に入りクリッピングが効く。
fn reparent_child_anchors_under(
    sg: &mut SceneGraph,
    effective_parent: Option<NodeId>,
    children: &[ElementId],
    lowering: &SceneLowering,
) {
    let Some(parent) = effective_parent else {
        return;
    };
    for &child_id in children {
        let Some(child_anchor) = lowering.anchors.get(&child_id).map(|e| e.anchor_id) else {
            continue;
        };
        attach_under(sg, parent, child_anchor);
    }
}

/// 両アンカー戦略が共有する単一のシーンウォーク。戦略固有の接続は [`AnchorSink`] に委譲し、
/// エミッション本体は [`emit_element`] にある。スキップ（未訪問）要素も `begin`/再帰パスを
/// 受けるので、retained はそのアンカーを付け直す。
fn walk<S: AnchorSink>(ctx: &mut WalkCtx, sink: &mut S, cursor: S::Cursor, id: ElementId) {
    sink.enter_node();

    let tree = ctx.tree;
    let (taffy_node, el) = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (taffy_node, el),
        Some(TraversalStep::Skip(_)) => {
            let base = sink.begin(ctx, cursor, id);
            for (child, child_cursor) in sink.children(tree, cursor, id, base) {
                let mut child_ctx = ctx.child(ctx.ox, ctx.oy, ctx.inherited.clone());
                walk(&mut child_ctx, sink, child_cursor, child);
            }
            return;
        }
        None => return,
    };

    // `display: none`（base もしくはレイアウト系 variant 由来）の要素はサブツリーごと
    // 描画しない。Taffy はレイアウトから除外するが、scene_build は要素ツリーを歩くため
    // 明示的に枝刈りしないと子（特に text 要素のグリフ）が漏れて描かれてしまう。
    if is_display_none(tree, taffy_node) {
        clear_hidden_subtree(ctx, sink, cursor, id);
        return;
    }

    let base = sink.begin(ctx, cursor, id);
    emit_element(ctx, sink, cursor, id, el, taffy_node, base);
}

/// 要素の実効 Taffy display が `none` か。
fn is_display_none(tree: &crate::element::tree::ElementTree, taffy_node: taffy::NodeId) -> bool {
    tree.layout
        .projection
        .taffy
        .style(taffy_node)
        .map(|style| style.display == taffy::Display::None)
        .unwrap_or(false)
}

/// `display: none` のサブツリーを「内容ゼロ」で処理する。各ノードで `begin`（retained では
/// 旧内容のクリア）と `end_element`（子アンカーの再配置）は行うが、ボックス・テキスト・
/// 子の visual は一切 emit しない。これで隠れた要素の旧描画が次フレームに残らない。
fn clear_hidden_subtree<S: AnchorSink>(
    ctx: &mut WalkCtx,
    sink: &mut S,
    cursor: S::Cursor,
    id: ElementId,
) {
    let base = sink.begin(ctx, cursor, id);
    let tree = ctx.tree;
    for (child, child_cursor) in sink.children(tree, cursor, id, base) {
        let mut child_ctx = ctx.child(ctx.ox, ctx.oy, ctx.inherited.clone());
        clear_hidden_subtree(&mut child_ctx, sink, child_cursor, child);
    }
    sink.end_element(ctx, base, id);
}

fn emit_element<S: AnchorSink>(
    ctx: &mut WalkCtx,
    sink: &mut S,
    cursor: S::Cursor,
    id: ElementId,
    el: &crate::element::tree::Element,
    taffy_node: taffy::NodeId,
    base: Option<NodeId>,
) {
    let tree = ctx.tree;
    let inherited_base = effective_visual::apply_text_inheritance(&ctx.inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &ctx.inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let resolved = effective_visual::resolve_effective(
        &ctx.inherited,
        &el.visual,
        &el.viewport_variants,
        tree.viewport(),
        &el.pseudo_styles,
        ctx.interaction,
        id,
    );
    let visual = sink.displayed(id, resolved);
    let layout = match tree.layout.projection.taffy.layout(taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ctx.ox + layout.location.x;
    let y = ctx.oy + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    let confirmed_color = visual.text_color.unwrap_or(Color::BLACK);
    let confirmed_font_size = visual.font_size.unwrap_or(16.0);

    let mut effective_parent = base;
    if let Some(transform) = el.transform {
        let group_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Group { transform },
                children: Vec::new(),
            },
        );
        effective_parent = Some(group_id);
    }

    // ネイティブフォーカスリングの親。要素自身のオーバーフロークリップより上に置き、リングが
    // ボックスに切り取られないようにする（Chromium は outline を要素のクリップ外に描く）。
    // ただし transform グループの内側には保つ。
    let ring_parent = effective_parent;

    let effective_parent = if el.kind == ElementKind::ScrollView {
        let clip_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [0.0; 4],
                },
                children: Vec::new(),
            },
        );
        // scroll offset → group アフィン。プロファイル差（iOS translate / Android stretch）は
        // ここ一箇所に閉じる（ADR-0131）。恒等変換なら group を省く。
        let transform = scroll_group_affine(
            tree.scroll_profile(),
            el.scroll_offset,
            tree.element_scroll_max_offset(id),
            (x, y, w, h),
            tree.scroll_tuning(),
        );
        if transform != IDENTITY_AFFINE {
            let scroll_group = emit(
                ctx.sg,
                Some(clip_id),
                Node {
                    kind: NodeKind::Group { transform },
                    children: Vec::new(),
                },
            );
            Some(scroll_group)
        } else {
            Some(clip_id)
        }
    } else if visual.overflow == OverflowValue::Hidden {
        let clip_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [visual.border_radius; 4],
                },
                children: Vec::new(),
            },
        );
        Some(clip_id)
    } else {
        effective_parent
    };

    if !visual.box_shadow.is_empty() {
        // owner 背景が不透明なら、drop shadow の被覆 border-box 内側を塗り領域から除外する
        // （issue #659。覆われて見えない中央の overdraw を削る。出力ピクセルは不変）。
        let occluder = drop_shadow_occluder(&visual, x, y, w, h);
        emit_box_shadows(
            ctx.sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            false,
            occluder,
        );
    }

    emit_visual_box(
        ctx.sg,
        effective_parent,
        x,
        y,
        w,
        h,
        visual.border_radius,
        visual.border_width,
        visual.background_color,
        visual.border_color,
        visual.border_style,
        visual.opacity,
    );

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            ctx.sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            true,
            // inset 影は border-box でクリップされ occluder の対象外。
            None,
        );
    }

    // draw display list（#724 / ADR-0141）: 描画順は background → border → draw →
    // children。ノードはボーダーボックス左上の絶対原点 `(x, y)` を持ち、コマンドの
    // パス座標はボーダーボックス相対のまま。クリップは上の overflow ラップに従う
    // （既定 visible = box 外へのはみ出し可）。
    if !visual.draw.is_empty() {
        emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::DrawList {
                    x,
                    y,
                    commands: visual.draw.clone(),
                },
                children: Vec::new(),
            },
        );
    }

    if el.kind == ElementKind::Image {
        if let Some(img) = el.src_image.clone() {
            emit(
                ctx.sg,
                effective_parent,
                Node {
                    kind: NodeKind::Image {
                        x,
                        y,
                        width: w,
                        height: h,
                        data: img,
                    },
                    children: Vec::new(),
                },
            );
        }
    } else if el.kind == ElementKind::TextInput {
        let content_x = x + layout.border.left + layout.padding.left;
        let content_y = y + layout.border.top + layout.padding.top;
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        // 選択ハイライトはテキストの背後に描くが（ADR-0097）、フォーカス中の text-input に
        // 限る（ADR-0104）。非フォーカスのフィールドは範囲を EditState に覚えていてもハイライトを
        // 隠すので、Mouse/Pen のフォーカス喪失は「隠す＋記憶」となり、ドキュメント全体で点灯する
        // 選択は高々1つ（＝フォーカス中）になる。
        if let Some(cl) = el.content_layout.as_ref() {
            let active_range = (tree.focused_element() == Some(id))
                .then(|| el.edit.as_ref().and_then(|e| e.selection_range()))
                .flatten();
            emit_edit_selection_highlight(
                &cl.layout,
                active_range,
                content_x,
                content_y,
                tree.chrome_tuning().selection_highlight_color,
                ctx.sg,
                effective_parent,
            );
        }
        // 空の入力はプレースホルダを表示する：layout_pass は `content_layout` を空にし、
        // プレースホルダを `text_layout` に積む（ADR-0058）。Chromium は `::placeholder` を本文
        // `color` ではなく淡色で描く。Canvas のビジュアル基準は Chromium DOM なので、
        // プレースホルダランは `confirmed_color` ではなく淡色で描く（ADR-0102）。
        let (runs, run_color) = if let Some(cl) = el.content_layout.as_ref() {
            (Some(cl.runs.as_slice()), color)
        } else {
            let muted = placeholder_muted_color(confirmed_color, tree.chrome_tuning().placeholder_alpha)
                .with_opacity(visual.opacity)
                .to_array_f32();
            (el.text_layout.as_ref().map(|tl| tl.runs.as_slice()), muted)
        };
        if let Some(runs) = runs {
            for run in runs {
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::TextRun {
                            x: content_x,
                            y: content_y,
                            color: run_color,
                            data: run.clone(),
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
        // IME 変換中の下線：文節ごとに1本、プリエディットグリフの下に描く。Chromium は
        // 変換中の文節を太く、確定済みを細く下線する（ADR-0102）。
        if let Some(cl) = el.content_layout.as_ref() {
            if let Some(edit) = el.edit.as_ref() {
                emit_composition_underlines(
                    &cl.layout,
                    &edit.composition_underlines(),
                    content_x,
                    content_y,
                    color,
                    tree.chrome_tuning().composition_underline_thin,
                    tree.chrome_tuning().composition_underline_thick,
                    ctx.sg,
                    effective_parent,
                );
            }
        }
        if el.cursor_visible {
            if let Some(cl) = el.content_layout.as_ref() {
                let cursor_index = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.display_cursor_byte_index())
                    .unwrap_or(0);
                let cursor = parley::Cursor::from_byte_index(
                    &cl.layout,
                    cursor_index,
                    parley::Affinity::Upstream,
                );
                let bbox = cursor.geometry(&cl.layout, 1.5_f32);
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x + bbox.x0 as f32,
                            y: content_y + bbox.y0 as f32,
                            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
                            height: (bbox.y1 - bbox.y0) as f32,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            } else {
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x,
                            y: content_y,
                            width: 1.5,
                            height: confirmed_font_size * 1.2,
                            color: confirmed_color
                                .with_opacity(visual.opacity)
                                .to_array_f32(),
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    } else if let Some(tl) = el.text_layout.as_ref() {
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        emit_selection_highlight(tree, id, &tl.layout, x, y, ctx.sg, effective_parent);
        for run in &tl.runs {
            emit(
                ctx.sg,
                effective_parent,
                Node {
                    kind: NodeKind::TextRun {
                        x,
                        y,
                        color,
                        data: run.clone(),
                    },
                    children: Vec::new(),
                },
            );
        }
    }

    // ネイティブフォーカスリング（`:focus-visible`）。ボックス自身の内容の上、ボーダーの外側に、
    // 角丸に沿って描く。アプリの `:focus` 背景/ボーダー切り替えは上の擬似スタイルで別途解決され
    // 影響しない。
    if tree.focus_visible_element() == Some(id) {
        emit_focus_ring(ctx.sg, ring_parent, x, y, w, h, visual.border_radius);
    }

    // スクロールバーオーバーレイ（ADR-0110）。オーバーフロー軸ごとにコンテンツの上、`ring_parent`
    // 下に描く。フォーカスリング同様コンテンツ Clip と scroll Group の上に乗り、自身はスクロール
    // 変換されない——サムはボックス端に固定され、トラック上の位置だけが Scroll Offset を追う。
    // ネストしたスクロールビューでは `ring_parent` が既に外側 Clip/scroll Group の下にあるので、
    // 内側サムは外側ボックスに従い外へ漏れない。
    if el.kind == ElementKind::ScrollView {
        emit_scrollbar_overlay(tree, id, ctx.sg, ring_parent, x, y, w, h);
    }

    for (child, child_cursor) in sink.children(tree, cursor, id, effective_parent) {
        let mut child_ctx = ctx.child(x, y, child_inherited.clone());
        walk(&mut child_ctx, sink, child_cursor, child);
    }
    sink.end_element(ctx, effective_parent, id);
}

/// ボックス `(x, y, width, height)` を外側から包む `RoundedRing`——ネイティブフォーカスリング——を
/// 出す。外矩形は各辺でオフセット＋リング幅だけ拡大し、リング内縁がボーダーボックスの
/// `FOCUS_RING_OFFSET` 外側に来る（Chromium の `outline-offset` 相当）。
fn emit_focus_ring(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
) {
    let grow = FOCUS_RING_OFFSET + FOCUS_RING_WIDTH;
    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::RoundedRing {
                x: x - grow,
                y: y - grow,
                width: width + 2.0 * grow,
                height: height + 2.0 * grow,
                outer_radius: border_radius.max(0.0) + grow,
                border_width: FOCUS_RING_WIDTH,
                color: FOCUS_RING_COLOR.to_array_f32(),
            },
            children: Vec::new(),
        },
    );
}

/// スクロール軸1本のサム範囲 `(start, length)`。ボックス端を原点とするボックスローカルなトラック
/// 空間で表す。`viewport` は軸上のボックス長、`content` はスクロール可能なコンテンツ長、`offset` は
/// 現在の Scroll Offset、`max` はその最大値。長さは viewport/content 比でスケールし
/// [`SCROLLBAR_MIN_THUMB_LENGTH`] を下限とする。start は offset のスクロール範囲に対する割合で
/// サムをトラック上を進める。
fn scrollbar_thumb_extent(
    viewport: f32,
    content: f32,
    offset: f32,
    max: f32,
    track_margin: f32,
    min_thumb_length: f32,
) -> (f32, f32) {
    let track_len = (viewport - 2.0 * track_margin).max(0.0);
    let thumb_len = (track_len * viewport / content)
        .max(min_thumb_length)
        .min(track_len);
    let progress = if max > 0.0 {
        (offset / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let start = track_margin + (track_len - thumb_len) * progress;
    (start, thumb_len)
}

/// 呼び出し側が解決済みのボックス `(x, y, w, h)` について、オーバーフロー軸ごとのスクロールバー
/// ジオメトリを計算する。レイアウトからボックスを与える [`scrollbar_axes`] とシーンウォークから
/// ボックスを与える [`emit_scrollbar_overlay`] の共有コアで、描画とヒットテストが同一ジオメトリを得る。
fn scrollbar_axes_in_box(
    tree: &ElementTree,
    id: ElementId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Vec<ScrollbarAxisGeometry> {
    let ct = tree.chrome_tuning();
    let (content_w, content_h) = tree.element_content_size(id);
    let (max_x, max_y) = tree.element_scroll_max_offset(id);
    let (offset_x, offset_y) = tree.element_get_scroll_offset(id);
    let mut axes = Vec::new();

    // 右端の縦バー。コンテンツがボックス高さを超えるときだけ。
    if content_h > h {
        let (start, thumb_len) = scrollbar_thumb_extent(
            h,
            content_h,
            offset_y,
            max_y,
            ct.scrollbar_track_margin,
            ct.scrollbar_min_thumb_length,
        );
        let track_len = (h - 2.0 * ct.scrollbar_track_margin).max(0.0);
        let bar_x = x + w - ct.scrollbar_track_margin - ct.scrollbar_thickness;
        axes.push(ScrollbarAxisGeometry {
            axis: ScrollAxis::Vertical,
            thumb: (bar_x, y + start, ct.scrollbar_thickness, thumb_len),
            track: (
                bar_x,
                y + ct.scrollbar_track_margin,
                ct.scrollbar_thickness,
                track_len,
            ),
            max_offset: max_y,
            thumb_travel: (track_len - thumb_len).max(0.0),
        });
    }

    // 下端の横バー。コンテンツが幅を超えるときだけ。
    if content_w > w {
        let (start, thumb_len) = scrollbar_thumb_extent(
            w,
            content_w,
            offset_x,
            max_x,
            ct.scrollbar_track_margin,
            ct.scrollbar_min_thumb_length,
        );
        let track_len = (w - 2.0 * ct.scrollbar_track_margin).max(0.0);
        let bar_y = y + h - ct.scrollbar_track_margin - ct.scrollbar_thickness;
        axes.push(ScrollbarAxisGeometry {
            axis: ScrollAxis::Horizontal,
            thumb: (x + start, bar_y, thumb_len, ct.scrollbar_thickness),
            track: (
                x + ct.scrollbar_track_margin,
                bar_y,
                track_len,
                ct.scrollbar_thickness,
            ),
            max_offset: max_x,
            thumb_travel: (track_len - thumb_len).max(0.0),
        });
    }

    axes
}

/// `ScrollView` のスクロールバーオーバーレイを lowering する（ADR-0110）。オーバーフロー軸ごとに
/// 角丸サムを1つ、`parent` 下（コンテンツクリップの上）に描く。縦バーは右端、横バーは下端。
/// コンテンツが収まる軸は何も描かない。
#[allow(clippy::too_many_arguments)]
fn emit_scrollbar_overlay(
    tree: &ElementTree,
    id: ElementId,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // ポインタモダリティ分岐（ADR-0110）。選択クロームを制御するのと同じ最後のポインタ種別
    // （ADR-0104）を再利用する——Mouse/Pen には操作可能サム、Touch には一時インジケータ。
    match tree.last_pointer_kind() {
        PointerKind::Touch => emit_touch_scroll_indicator(tree, id, sg, parent, x, y, w, h),
        PointerKind::Mouse | PointerKind::Pen => {
            let ct = tree.chrome_tuning();
            let thumb_rgba = ct
                .scrollbar_thumb_color
                .with_opacity(ct.scrollbar_thumb_opacity)
                .to_array_f32();
            let radius = ct.scrollbar_thickness / 2.0;
            for axis in scrollbar_axes_in_box(tree, id, x, y, w, h) {
                let (tx, ty, tw, th) = axis.thumb;
                emit_fill_rect(sg, parent, tx, ty, tw, th, thumb_rgba, radius);
            }
        }
    }
}

/// 最後に更新されてから `elapsed` ms 経った Touch インジケータの可視係数 `[0, 1]`（ADR-0110）。
/// ホールド窓の間はフル、フェード窓でゼロへ線形に下降、それ以降はゼロ。レンダー時の前進処理が
/// 各ライブインジケータの `fade` を再計算するのに使う単一の源。
pub fn touch_scroll_indicator_fade(elapsed: f64) -> f32 {
    if elapsed <= SCROLLBAR_INDICATOR_HOLD_MS {
        1.0
    } else if elapsed >= SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS {
        0.0
    } else {
        (1.0 - (elapsed - SCROLLBAR_INDICATOR_HOLD_MS) / SCROLLBAR_INDICATOR_FADE_MS) as f32
    }
}

/// `ScrollView` の Touch 一時インジケータを lowering する（ADR-0110）。スクロール中に現れ停止後に
/// フェードする非操作バーで、サム/トラックのヒット領域を持たない（フリックでスクロールし、ドラッグ
/// ではない）。表示→フェード窓の間だけ描き、静止した Touch 面ではスクロールバーを一切描かない
/// （モバイルに常時表示バーは無い）。
#[allow(clippy::too_many_arguments)]
fn emit_touch_scroll_indicator(
    tree: &ElementTree,
    id: ElementId,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let fade = tree.touch_scroll_indicator_opacity(id);
    if fade <= 0.0 {
        return;
    }
    let ct = tree.chrome_tuning();
    let rgba = ct
        .scrollbar_indicator_color
        .with_opacity(ct.scrollbar_indicator_opacity * fade)
        .to_array_f32();
    let radius = ct.scrollbar_indicator_thickness / 2.0;
    for axis in scrollbar_axes_in_box(tree, id, x, y, w, h) {
        // インジケータは同じサムジオメトリに乗る（位置は Scroll Offset を追う）が、より細く
        // ボックス端に固定される——縦バーは右端、横バーは下端。
        let (tx, ty, tw, th) = axis.thumb;
        let (ix, iy, iw, ih) = match axis.axis {
            ScrollAxis::Vertical => (
                tx + tw - ct.scrollbar_indicator_thickness,
                ty,
                ct.scrollbar_indicator_thickness,
                th,
            ),
            ScrollAxis::Horizontal => (
                tx,
                ty + th - ct.scrollbar_indicator_thickness,
                tw,
                ct.scrollbar_indicator_thickness,
            ),
        };
        emit_fill_rect(sg, parent, ix, iy, iw, ih, rgba, radius);
    }
}

/// TextInput がプレースホルダを表示するとき本文 `color` の代わりに使う Chromium UA `::placeholder`
/// の淡色（ADR-0102：Canvas のビジュアル基準は Chromium DOM）。Chromium は黒（ライト配色）または
/// 白（ダーク）の約54%でプレースホルダを描き、入力背景に合成する——本文 `color` から導かれず、
/// 並べて指定もできない。配色は本文色の輝度から推定する：暗い本文 ⇒ ライト配色 ⇒ 淡い黒、
/// 明るい本文 ⇒ ダーク配色 ⇒ 淡い白。0.54 は ADR-0102 の原則（黒/白の約54%）に従う。
pub(crate) const PLACEHOLDER_ALPHA: f64 = 0.54;

fn placeholder_muted_color(body: Color, alpha: f64) -> Color {
    let luma = 0.299 * body.r + 0.587 * body.g + 0.114 * body.b;
    let base = if luma < 0.5 { Color::BLACK } else { Color::WHITE };
    Color::new(base.r, base.g, base.b, alpha)
}

fn emit(sg: &mut SceneGraph, parent_group: Option<NodeId>, node: Node) -> NodeId {
    match parent_group {
        None => sg.insert(node),
        Some(p) => sg.insert_child(p, node),
    }
}

/// Material 風の選択ティント（ADR-0097：スタイルがテーマ切替可能な単一のコア描画クローム。
/// この値は初期テーマとしてここに置く）。
pub(crate) const SELECTION_HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

/// IFC ルート `id` のアクティブ選択ハイライトを、覆う行ごとに塗り矩形1つとして lowering する。
/// 要素の内容空間（テキストラン原点 `ox`, `oy` でオフセット）に配置する。ドキュメント選択が
/// `id` 内に無ければ no-op。
fn emit_selection_highlight(
    tree: &ElementTree,
    id: ElementId,
    layout: &parley::Layout<crate::element::text::TextBrush>,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    let Some((start, end)) = tree.selection_range_in_block(id) else {
        return;
    };
    let highlight_color = tree.chrome_tuning().selection_highlight_color;
    for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
        emit(
            sg,
            parent,
            Node {
                kind: NodeKind::Rect {
                    x: ox + rx,
                    y: oy + ry,
                    width: rw,
                    height: rh,
                    color: highlight_color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
}

/// text-input の編集選択ハイライトを lowering する（ADR-0097）。`EditState` のバイト `range` を
/// `content_layout` 上で、要素の内容空間（`content_x`, `content_y` でオフセット）に描く。テキストの
/// 背後に描く。範囲が崩壊/不在なら no-op。
fn emit_edit_selection_highlight(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    range: Option<(usize, usize)>,
    content_x: f32,
    content_y: f32,
    highlight_color: [f32; 4],
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    let Some((start, end)) = range else {
        return;
    };
    for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
        emit(
            sg,
            parent,
            Node {
                kind: NodeKind::Rect {
                    x: content_x + rx,
                    y: content_y + ry,
                    width: rw,
                    height: rh,
                    color: highlight_color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
}

/// IME 変換中の下線の太さ（ADR-0102）。Chromium は確定済み文節を細い下線で、変換中の文節を
/// 太い下線で描く。
pub(crate) const COMPOSITION_UNDERLINE_THIN: f32 = 1.0;
pub(crate) const COMPOSITION_UNDERLINE_THICK: f32 = 2.0;

/// text-input の IME 変換下線を lowering する（ADR-0102）。文節ごとに塗り矩形1つを、要素の内容
/// 空間（`content_x`, `content_y` でオフセット）の各覆う行の下端に、テキスト `color` で描く。
/// `underlines` は表示テキストのバイト範囲とその太さ。変換中でなければ no-op。
#[allow(clippy::too_many_arguments)]
fn emit_composition_underlines(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    underlines: &[(usize, usize, crate::element::edit_state::CompositionUnderline)],
    content_x: f32,
    content_y: f32,
    color: [f32; 4],
    thin: f32,
    thick: f32,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    use crate::element::edit_state::CompositionUnderline;
    for &(start, end, weight) in underlines {
        let thickness = match weight {
            CompositionUnderline::Thin => thin,
            CompositionUnderline::Thick => thick,
        };
        for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
            emit(
                sg,
                parent,
                Node {
                    kind: NodeKind::Rect {
                        x: content_x + rx,
                        y: content_y + ry + rh - thickness,
                        width: rw,
                        height: thickness,
                        color,
                        corner_radius: 0.0,
                    },
                    children: Vec::new(),
                },
            );
        }
    }
}

/// Parley レイアウトのバイト範囲 `start..end` を覆う行ごとのハイライト矩形（レイアウトローカル
/// 座標）。各行は、クランプした範囲開始のキャレットから範囲終端のキャレットまでの幅を寄与する。
pub(crate) fn selection_highlight_rects(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    start: usize,
    end: usize,
) -> Vec<(f32, f32, f32, f32)> {
    use parley::{Affinity, Cursor};
    let mut rects = Vec::new();
    if start >= end {
        return rects;
    }
    for line in layout.lines() {
        let line_range = line.text_range();
        let s = start.max(line_range.start);
        let e = end.min(line_range.end);
        if s >= e {
            continue;
        }
        let m = line.metrics();
        let y0 = m.block_min_coord;
        let height = m.block_max_coord - m.block_min_coord;
        let x_start = Cursor::from_byte_index(layout, s, Affinity::Downstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let x_end = Cursor::from_byte_index(layout, e, Affinity::Upstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let left = x_start.min(x_end);
        let width = (x_end - x_start).abs();
        if width > 0.0 && height > 0.0 {
            rects.push((left, y0, width, height));
        }
    }
    rects
}

fn emit_visual_box(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    border_width: f32,
    background_color: Option<Color>,
    border_color: Option<Color>,
    border_style: BorderStyleValue,
    opacity: f32,
) {
    let radius = border_radius.max(0.0);
    let border_w = border_width.max(0.0);
    let background = background_color.map(|c| c.with_opacity(opacity).to_array_f32());
    let border = border_color.map(|c| c.with_opacity(opacity).to_array_f32());

    // ボーダーは正の幅と明示的なスタイルの両方があるときだけ描く（CSS 同様 `border-style` の
    // 既定は `none`）。
    let draw_border = border_w > 0.0 && border_style != BorderStyleValue::None;

    if draw_border {
        let Some(border_rgba) = border else {
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            return;
        };

        if border_style == BorderStyleValue::Dashed {
            // 背景はボックス全体を塗り、破線が周囲をその上にストロークする。
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::DashedBorder {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        if let Some(bg) = background {
            emit_fill_rect(
                sg,
                parent_group,
                x,
                y,
                width,
                height,
                border_rgba,
                radius,
            );
            let inner_w = (width - 2.0 * border_w).max(0.0);
            let inner_h = (height - 2.0 * border_w).max(0.0);
            if inner_w > 0.0 && inner_h > 0.0 {
                let inner_radius = (radius - border_w).max(0.0);
                emit_fill_rect(
                    sg,
                    parent_group,
                    x + border_w,
                    y + border_w,
                    inner_w,
                    inner_h,
                    bg,
                    inner_radius,
                );
            }
            return;
        }

        if radius > 0.0 {
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::RoundedRing {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        for (bx, by, bw2, bh2) in [
            (x, y, width, border_w),
            (x, y + height - border_w, width, border_w),
            (x, y + border_w, border_w, (height - 2.0 * border_w).max(0.0)),
            (
                x + width - border_w,
                y + border_w,
                border_w,
                (height - 2.0 * border_w).max(0.0),
            ),
        ] {
            emit_fill_rect(sg, parent_group, bx, by, bw2, bh2, border_rgba, 0.0);
        }
        return;
    }

    if let Some(bg) = background {
        emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
    }
}

fn emit_fill_rect(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
    corner_radius: f32,
) {
    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                corner_radius,
            },
            children: Vec::new(),
        },
    );
}

/// 要素の box-shadow レイヤーのうち `inset == want_inset` の部分集合を出す。
///
/// CSS は最初に挙げた影を最前面に描くので、逆順で出す（最後の影が先＝最背面）。outset 影は
/// ボックスの背後に、inset 影は背景の上にボーダーボックスでクリップして出す。
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn emit_box_shadows(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    shadows: &[Shadow],
    opacity: f32,
    want_inset: bool,
    occluder: Option<ShadowOccluder>,
) {
    let radius = border_radius.max(0.0);
    for shadow in shadows.iter().rev() {
        if shadow.inset != want_inset {
            continue;
        }
        let color = shadow.color.with_opacity(opacity);
        if color.a <= 0.0 {
            continue;
        }
        if want_inset {
            emit_inset_shadow(sg, parent_group, x, y, width, height, radius, shadow, color);
        } else {
            emit_drop_shadow(sg, parent_group, x, y, width, height, radius, shadow, color, occluder);
        }
    }
}

/// 不透明判定の閾値（アルファ）。owner 背景と opacity がともにこの値以上なら「不透明」とみなし、
/// drop shadow の被覆 border-box 内側を塗り領域から除外する（issue #659）。ピクセル不変を保つため
/// 完全不透明（= 1.0）を要求する——半透明背景では影が透けるので除外できない。
const OPAQUE_OCCLUSION_ALPHA_THRESHOLD: f32 = 1.0;

/// occluder を owner の塗り境界から内側へ縮める余白（px）。owner 背景フィルの縁は
/// アンチエイリアスで被覆 < 1 になるため、その帯の影を省くと最終ピクセルが変わってしまう。
/// フィルが完全不透明な内側だけを occluder にするための余白（AA 帯 ~1px を吸収）。
const SHADOW_OCCLUDER_AA_INSET: f32 = 1.0;

/// owner の不透明背景が覆う border-box 内側（drop shadow の occluder）。背景・opacity がともに
/// 不透明でなければ `None`（半透明・transparent 背景では影が透けるため全面塗り）。border があれば
/// その内側（bg で不透明に塗られる領域）を返す——border リング自体は不透明とは限らないので除外しない。
fn drop_shadow_occluder(visual: &Visual, x: f32, y: f32, w: f32, h: f32) -> Option<ShadowOccluder> {
    if visual.opacity < OPAQUE_OCCLUSION_ALPHA_THRESHOLD {
        return None;
    }
    let bg = visual.background_color?;
    if (bg.a as f32) < OPAQUE_OCCLUSION_ALPHA_THRESHOLD {
        return None;
    }
    let bw = if visual.border_width > 0.0 && visual.border_style != BorderStyleValue::None {
        visual.border_width
    } else {
        0.0
    };
    // border-box 内側（bg で不透明に塗られる領域）を、AA 帯ぶん内側へ縮めた矩形。
    let m = bw + SHADOW_OCCLUDER_AA_INSET;
    let iw = w - 2.0 * m;
    let ih = h - 2.0 * m;
    if iw <= 0.0 || ih <= 0.0 {
        return None;
    }
    Some(ShadowOccluder {
        x: x + m,
        y: y + m,
        width: iw,
        height: ih,
        corner_radius: (visual.border_radius.max(0.0) - m).max(0.0),
    })
}

/// outset（ドロップ）影：`spread` で拡大しオフセットでずらした角丸矩形を、ぼかす。
///
/// ぼかしたシャドウは第一級プリミティブ [`NodeKind::BlurredRoundedRect`] 1 ノードとして emit し
/// （影1個 = 1描画）、ラスタ方式は painter に委ねる——解析パス（vello の
/// `draw_blurred_rounded_rect` / tiny-skia の per-pixel）か、erf シェル近似の default フォールバック。
/// ぼかしなし（`blur <= HARD_SHADOW_BLUR_THRESHOLD`）のハードシャドウは外形 1 枚を `Rect` で塗る。
#[allow(clippy::too_many_arguments)]
fn emit_drop_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
    occluder: Option<ShadowOccluder>,
) {
    let sx = x + shadow.offset_x - shadow.spread;
    let sy = y + shadow.offset_y - shadow.spread;
    let sw = (width + 2.0 * shadow.spread).max(0.0);
    let sh = (height + 2.0 * shadow.spread).max(0.0);
    let sr = (radius + shadow.spread).max(0.0);
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }

    let blur = shadow.blur.max(0.0);
    if blur <= HARD_SHADOW_BLUR_THRESHOLD {
        // ぼかしなしのハードシャドウは外形を 1 枚で不透明に塗る。
        emit_fill_rect(sg, parent_group, sx, sy, sw, sh, color.to_array_f32(), sr);
        return;
    }

    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::BlurredRoundedRect {
                x: sx,
                y: sy,
                width: sw,
                height: sh,
                corner_radius: sr,
                std_dev: shadow_sigma(blur),
                color: color.to_array_f32(),
                occluder,
            },
            children: Vec::new(),
        },
    );
}

/// inset 影：内縁に沿った暗い帯。ボーダーボックスの角丸に追従する `RoundedRing`
/// （even-odd の帯フィル）で描き、ボーダーボックスでクリップする。従来は上下左右の
/// 直線矩形を角丸クリップしていたため、角丸コーナーで帯が途切れ／角張っていた
/// （border-radius 非対応）。
#[allow(clippy::too_many_arguments)]
fn emit_inset_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    // オフセット付き inset 影がボーダーボックス外へはみ出さないよう、角丸でクリップする。
    let clip_id = emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Clip {
                x,
                y,
                width,
                height,
                corner_radii: [radius; 4],
            },
            children: Vec::new(),
        },
    );

    let spread = shadow.spread.max(0.0);
    let blur = shadow.blur.max(0.0);
    let max_band = width.min(height) * 0.5;

    if blur <= HARD_SHADOW_BLUR_THRESHOLD {
        // ぼかしなしのハードな内側リングは単色で塗る（厚み = spread）。ぼかしプリミティブ化せず
        // 従来どおり 1 枚の `RoundedRing`（ピクセル不変）。
        let bw = spread.max(0.5).min(max_band);
        if bw > 0.0 && color.a > 0.0 {
            emit(
                sg,
                Some(clip_id),
                Node {
                    kind: NodeKind::RoundedRing {
                        x: x + shadow.offset_x,
                        y: y + shadow.offset_y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: bw,
                        color: color.to_array_f32(),
                    },
                    children: Vec::new(),
                },
            );
        }
        return;
    }

    // ぼかしあり：inset 影を第一級プリミティブ 1 ノードとして border-box クリップ内に emit する
    // （影1個 = 1描画、現行の 10 段リングを置換・issue #660）。ラスタ方式は painter に委ねる——
    // 解析パス（vello の DestOut レイヤ / tiny-skia の per-pixel `1 − 被覆`）か、同心リングの
    // default フォールバック。border-box への角丸クリップは上の `Clip` ノードが与える。
    emit(
        sg,
        Some(clip_id),
        Node {
            kind: NodeKind::InsetBlurredRoundedRect {
                x,
                y,
                width,
                height,
                corner_radius: radius,
                offset_x: shadow.offset_x,
                offset_y: shadow.offset_y,
                spread,
                std_dev: shadow_sigma(blur),
                color: color.to_array_f32(),
            },
            children: Vec::new(),
        },
    );
}

#[cfg(test)]
mod scroll_group_affine_tests {
    use super::scroll_group_affine;
    use crate::scroll::{physics, ScrollPhysicsProfile, ScrollPhysicsTuning};

    fn t() -> ScrollPhysicsTuning {
        ScrollPhysicsTuning::default()
    }

    // シーン座標のビューポート: 左上 (0,0)、幅 200 × 高さ 100。
    const VP: (f32, f32, f32, f32) = (0.0, 0.0, 200.0, 100.0);

    #[test]
    fn ios_profile_translates_by_the_full_offset_with_no_scale() {
        // iOS 風は overshoot 込みで content を丸ごと translate、scale 無し（従来挙動を厳密維持）。
        // 範囲内。
        assert_eq!(
            scroll_group_affine(ScrollPhysicsProfile::Auto, (0.0, 50.0), (0.0, 400.0), VP, &t()),
            [1.0, 0.0, 0.0, 1.0, 0.0, -50.0],
        );
        // 下端を 20px overscroll していても、iOS は overshoot をそのまま translate に含める。
        assert_eq!(
            scroll_group_affine(ScrollPhysicsProfile::Auto, (0.0, 420.0), (0.0, 400.0), VP, &t()),
            [1.0, 0.0, 0.0, 1.0, 0.0, -420.0],
        );
    }

    #[test]
    fn android_in_range_matches_ios_translate_with_no_scale() {
        // 範囲内は stretch 無し: iOS と同じ素の translate、scale 1.0。パリティ。
        assert_eq!(
            scroll_group_affine(ScrollPhysicsProfile::Android, (0.0, 50.0), (0.0, 400.0), VP, &t()),
            [1.0, 0.0, 0.0, 1.0, 0.0, -50.0],
        );
    }

    #[test]
    fn android_bottom_overscroll_pins_the_bottom_edge_and_scales_content() {
        // 下端を 20px 越えて overscroll: 端クランプ translate（clamped=max=400）＋ ピン端
        // アンカー scale（far 端 = ビューポート下端 y=100）に分割する。
        let m = scroll_group_affine(ScrollPhysicsProfile::Android, (0.0, 420.0), (0.0, 400.0), VP, &t());
        // 縦のスケール = overscroll_stretch_scale(20, dim=100) = 1 + 0.2*0.15 = 1.03。
        let scale_y = 1.0 + (20.0f64 / 100.0) * physics::STRETCH_MAX as f64;
        assert!((m[3] - scale_y).abs() < 1e-6, "y scale = {} (want {scale_y})", m[3]);
        // 横はスクロール不可（max_x=0）→ 伸びない（軸独立）。
        assert_eq!(m[0], 1.0, "x is not stretched");
        assert_eq!(m[4], 0.0, "x translate stays 0");
        // ピン留めした下端はビューポート下端 (シーン y=100) に固定されたまま。
        // 範囲内 max では content-bottom は content-y=500（500-400=100）にある。
        let pinned = m[3] * 500.0 + m[5];
        assert!((pinned - 100.0).abs() < 1e-4, "bottom edge pinned at viewport bottom (got {pinned})");
    }

    #[test]
    fn android_top_overscroll_pins_the_top_edge_and_scales_content() {
        // 上端を 30px 越えて overscroll（offset 負）: 端クランプ translate（clamped=0）＋
        // ピン端アンカー scale（near 端 = ビューポート上端 y=0）。
        let m = scroll_group_affine(ScrollPhysicsProfile::Android, (0.0, -30.0), (0.0, 400.0), VP, &t());
        let scale_y = 1.0 + (30.0f64 / 100.0) * physics::STRETCH_MAX as f64;
        assert!((m[3] - scale_y).abs() < 1e-6, "y scale = {} (want {scale_y})", m[3]);
        // ピン留めした上端はビューポート上端 (シーン y=0) に固定: content-top は content-y=0。
        let pinned = m[3] * 0.0 + m[5];
        assert!(pinned.abs() < 1e-4, "top edge pinned at viewport top (got {pinned})");
    }

    #[test]
    fn android_stretch_is_symmetric_at_both_edges() {
        // 同じ大きさの上端／下端 overscroll は同じスケールを生む（両端対称）。
        let top = scroll_group_affine(ScrollPhysicsProfile::Android, (0.0, -40.0), (0.0, 400.0), VP, &t());
        let bottom = scroll_group_affine(ScrollPhysicsProfile::Android, (0.0, 440.0), (0.0, 400.0), VP, &t());
        assert!((top[3] - bottom[3]).abs() < 1e-6, "scale symmetric at both edges");
    }

    #[test]
    fn android_horizontal_overscroll_stretches_only_the_scrollable_axis() {
        // 横スクロール可能・縦不可のページ: 右端 overscroll は横だけ伸ばし縦は伸ばさない。
        let m = scroll_group_affine(ScrollPhysicsProfile::Android, (240.0, 0.0), (200.0, 0.0), VP, &t());
        // dim_x = ビューポート幅 200、overshoot = 40 → scale_x = 1 + 0.2*0.15 = 1.03。
        let scale_x = 1.0 + (40.0f64 / 200.0) * physics::STRETCH_MAX as f64;
        assert!((m[0] - scale_x).abs() < 1e-6, "x scale = {} (want {scale_x})", m[0]);
        assert_eq!(m[3], 1.0, "y is not stretched (vertical not scrollable)");
        assert_eq!(m[5], 0.0, "y translate stays 0");
        // ピン留めした右端はビューポート右端 (シーン x=200) に固定。max では content-right は
        // content-x=400（400-200=200）。
        let pinned = m[0] * 400.0 + m[4];
        assert!((pinned - 200.0).abs() < 1e-4, "right edge pinned at viewport right (got {pinned})");
    }
}
