//! transition トリガーのクロスレンダラー間 Semantics Parity（ADR-0002 / ADR-0093）。
//! Canvas Render Layer は `setStyle` および継承由来の変化を補間するため、
//! その画面フレームは同一時刻のブラウザ CSS transition（DOM パス）と一致しなければならない。
//!
//! Canvas 側は実際の `ElementTree`: `render(timestamp_ms)` が retained 補間を進め、
//! `draw_ops` が描画後（ブレンド後）の色を読む。DOM 側はブラウザ CSS transition の
//! 独立した参照シミュレータ — 画面値から解決済みターゲットへ、変化後の
//! `transition-duration` にわたる線形補間。両者は同一の Hayate CSS 入力で駆動し
//! 同一タイムスタンプでサンプルする。比較するのは外部描画結果のみで（ADR-0079）、
//! どちらのレンダラーの内部にも触れない。

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState, RecordingPainter, StyleProp,
    TransitionTimingValue, render_scene_graph,
};

/// 独立に計算された2つの色チャンネルを一致とみなす許容誤差。
const PARITY_EPS: f32 = 1e-3;

// ---------------------------------------------------------------------------
// Canvas 側: 実際の retained Render Layer を描画出力越しに読む。
// ---------------------------------------------------------------------------

/// 現在のシーンで最初に塗られた rect の背景色。
fn canvas_background(tree: &ElementTree) -> [f32; 4] {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    for op in painter.into_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

// ---------------------------------------------------------------------------
// DOM 側: ブラウザ CSS transition の独立した参照モデル。
//
// ブラウザは animatable プロパティの計算値が変わると transition を開始し、画面値
// （`from`）から新しい計算値（`target`）へ、変化後の `transition-duration` にわたり
// timing 関数で補間する。クロックは変化後の最初のフレームに固定する。duration 0 は
// transition なしで即座に新値を表示。Blink を Canvas Render Layer とコードを共有せず
// 再現するため、Canvas 実装がずれれば結果が乖離する。
// ---------------------------------------------------------------------------

struct DomTransition {
    from: Color,
    target: Color,
    duration_ms: f32,
    start_ms: Option<f64>,
}

/// ブラウザが transition するように扱う単一の animatable プロパティ。
struct DomProperty {
    displayed: Color,
    active: Option<DomTransition>,
}

impl DomProperty {
    fn new(initial: Color) -> Self {
        Self {
            displayed: initial,
            active: None,
        }
    }

    /// 計算スタイルの変化を適用する。`duration_ms` は変化後に解決された
    /// `transition-duration`（ブラウザは状態/スタイル変化の後に有効な値を読む。
    /// 例えば `transition-duration: 0` を設定した `:hover` を抜ける際はベース値）。
    fn set_target(&mut self, target: Color, duration_ms: f32) {
        if colors_eq(target, self.displayed) {
            return;
        }
        if duration_ms <= 0.0 {
            // transition なし: 新しい計算値を即座に表示する。
            self.displayed = target;
            self.active = None;
            return;
        }
        self.active = Some(DomTransition {
            from: self.displayed,
            target,
            duration_ms,
            start_ms: None,
        });
    }

    /// `now_ms` まで進める。クロックは変化後の最初のフレームに固定する。
    fn render(&mut self, now_ms: f64) -> Color {
        if let Some(tr) = self.active.as_mut() {
            let start = *tr.start_ms.get_or_insert(now_ms);
            let progress = (((now_ms - start) as f32) / tr.duration_ms).clamp(0.0, 1.0);
            self.displayed = lerp_color(tr.from, tr.target, progress);
            if progress >= 1.0 {
                self.active = None;
            }
        }
        self.displayed
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t as f64;
    let l = |x: f64, y: f64| x + (y - x) * t;
    Color::new(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

fn colors_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-9
        && (a.g - b.g).abs() < 1e-9
        && (a.b - b.b).abs() < 1e-9
        && (a.a - b.a).abs() < 1e-9
}

fn assert_parity(label: &str, canvas: [f32; 4], dom: Color) {
    let dom = [dom.r as f32, dom.g as f32, dom.b as f32, dom.a as f32];
    for (i, channel) in ["r", "g", "b", "a"].iter().enumerate() {
        assert!(
            (canvas[i] - dom[i]).abs() < PARITY_EPS,
            "{label}: {channel} channel diverged — canvas {} vs dom {}",
            canvas[i],
            dom[i],
        );
    }
}

// ---------------------------------------------------------------------------
// 共有シーンビルダー。
// ---------------------------------------------------------------------------

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// `transition-duration` が `duration_ms`、timing が linear な赤いボックス
/// （ブラウザ参照の補間が一意でコード非依存になるよう linear にする）。
fn linear_box(duration_ms: f32) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(RED),
            StyleProp::TransitionDuration(duration_ms),
            StyleProp::TransitionTiming(TransitionTimingValue::Linear),
        ],
    );
    (tree, root)
}

/// `:hover` で緑になる linear ボックス。`hover_duration_ms` を指定すると hover 中の
/// `transition-duration` も上書きする（`:hover { … 0 }` の非対称ケース）:
/// hover-in は上書き値、hover-out はベース値を読む。
fn linear_hover_box(
    base_duration_ms: f32,
    hover_duration_ms: Option<f32>,
) -> (ElementTree, hayate_core::ElementId) {
    let (mut tree, root) = linear_box(base_duration_ms);
    let mut hover = vec![StyleProp::BackgroundColor(GREEN)];
    if let Some(d) = hover_duration_ms {
        hover.push(StyleProp::TransitionDuration(d));
    }
    tree.element_set_pseudo_style(root, PseudoState::Hover, &hover);
    (tree, root)
}

// ---------------------------------------------------------------------------
// AC1: `setStyle` による連続プロパティの変化が、全サンプル時刻で Canvas と DOM で
// 同一に補間される。
// ---------------------------------------------------------------------------

#[test]
fn set_style_change_has_canvas_dom_parity() {
    let (mut tree, root) = linear_box(200.0);
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // 両側に同一入力: setStyle で背景を青に変える。
    tree.element_set_style(root, &[StyleProp::BackgroundColor(BLUE)]);
    dom.set_target(BLUE, 200.0);

    // アンカーフレーム（ここでクロック開始）: 両側ともまだ赤。
    let anchor = canvas_background(&tree);
    tree.render(100.0);
    assert_parity("anchor", canvas_background(&tree), dom.render(100.0));
    assert_eq!(anchor[0], 1.0, "anchor frame still red before advancing");

    // 1/4・1/2・3/4 の各点で一致する。
    tree.render(150.0);
    assert_parity("t=150", canvas_background(&tree), dom.render(150.0));
    tree.render(200.0);
    assert_parity("t=200", canvas_background(&tree), dom.render(200.0));
    tree.render(250.0);
    assert_parity("t=250", canvas_background(&tree), dom.render(250.0));

    // ウィンドウ経過後: 両側とも正確に青で落ち着く。
    tree.render(300.0);
    let end = canvas_background(&tree);
    assert_parity("settled", end, dom.render(300.0));
    assert!(end[2] > 0.999 && end[0].abs() < PARITY_EPS, "settles on blue: {end:?}");
}

// ---------------------------------------------------------------------------
// AC2: 逆方向の割り込み（進行中の un-hover）は、両レンダラーとも画面値から継続する
// — どちらも解決済みターゲットへジャンプしない。
// ---------------------------------------------------------------------------

#[test]
fn reverse_interrupt_has_canvas_dom_parity() {
    let (mut tree, root) = linear_hover_box(200.0, None);
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // hover-in: 赤 → 緑をアニメーション。
    tree.update_pointer_hover(Some(root));
    dom.set_target(GREEN, 200.0);
    tree.render(100.0); // アンカー
    assert_parity("anchor", canvas_background(&tree), dom.render(100.0));
    tree.render(200.0); // 中間
    let mid = canvas_background(&tree);
    assert_parity("midway", mid, dom.render(200.0));
    assert!(mid[0] > 0.0 && mid[1] > 0.0, "captured a mid value: {mid:?}");

    // 同一時刻で逆転: ターゲットが赤に戻る。両側とも表示中の中間値を保つ
    // （連続的な反転）べきで、赤や緑へジャンプしてはならない。
    tree.update_pointer_hover(None);
    dom.set_target(RED, 200.0);
    tree.render(200.0);
    let reversed = canvas_background(&tree);
    assert_parity("reverse-instant", reversed, dom.render(200.0));
    assert!(
        (reversed[0] - mid[0]).abs() < 1e-2 && (reversed[1] - mid[1]).abs() < 1e-2,
        "reversal is continuous, not a jump: {mid:?} -> {reversed:?}"
    );

    // 逆転を継続すると、両側そろって赤へ戻る。
    tree.render(300.0);
    let back = canvas_background(&tree);
    assert_parity("reverse-midway", back, dom.render(300.0));
    assert!(back[0] > reversed[0], "red channel climbs back: {} -> {}", reversed[0], back[0]);

    tree.render(400.0);
    let done = canvas_background(&tree);
    assert_parity("reverse-settled", done, dom.render(400.0));
    assert!((done[0] - 1.0).abs() < PARITY_EPS, "settles back on red: {done:?}");
}

// ---------------------------------------------------------------------------
// AC3: ベース duration の上に `:hover { transition-duration: 0 }` を重ねると、
// hover-in は即時、hover-out はアニメーションになる。両レンダラーとも変化後の解決値から
// duration を読むため、この in/out 非対称が一致する。
// ---------------------------------------------------------------------------

#[test]
fn hover_duration_zero_asymmetry_has_canvas_dom_parity() {
    let (mut tree, root) = linear_hover_box(500.0, Some(0.0));
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // hover-in: 変化後 duration が 0 → 両側とも即時に緑、アニメーションなし。
    tree.update_pointer_hover(Some(root));
    dom.set_target(GREEN, 0.0);
    tree.render(100.0);
    let hovered = canvas_background(&tree);
    assert_parity("hover-in", hovered, dom.render(100.0));
    assert!(
        (hovered[1] - 1.0).abs() < PARITY_EPS && hovered[0].abs() < PARITY_EPS,
        "hover-in is instant green: {hovered:?}"
    );

    // hover-out: 変化後 duration はベースの 500ms → 両側とも緑→赤をアニメーション。
    tree.update_pointer_hover(None);
    dom.set_target(RED, 500.0);
    tree.render(200.0); // アンカー
    assert_parity("hover-out-anchor", canvas_background(&tree), dom.render(200.0));
    tree.render(450.0); // 500ms ウィンドウの 250ms 地点
    let mid = canvas_background(&tree);
    assert_parity("hover-out-midway", mid, dom.render(450.0));
    assert!(
        mid[0] > 0.0 && mid[0] < 1.0 && mid[1] > 0.0 && mid[1] < 1.0,
        "hover-out animates over 500ms: {mid:?}"
    );

    tree.render(700.0);
    let done = canvas_background(&tree);
    assert_parity("hover-out-settled", done, dom.render(700.0));
    assert!((done[0] - 1.0).abs() < PARITY_EPS, "settles back on red: {done:?}");
}
