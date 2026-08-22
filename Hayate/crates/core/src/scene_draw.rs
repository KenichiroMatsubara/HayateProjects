//! Scene 側の draw display list 語彙（#732）。
//!
//! wire 側の [`crate::wire::protocol::DrawCommand`] と**あえて別の型**にしてある。
//! 分岐するのはテキストだけで、他のコマンドは形も意味も同じ:
//!
//! | | wire | scene |
//! | --- | --- | --- |
//! | パス・変換・クリップ | 数値のみ | 同じ |
//! | テキスト | 生の文字列 ＋ フォント指定 | シェープ済みの [`TextRunId`] 列 |
//!
//! 理由は [`crate::render::painter::render_scene_graph`] の walk が
//! `&impl SceneRead` を取る immutable な経路で、interner を持たないこと。
//! walk 中にシェープも intern もできないので、**レイアウトパスでシェープし、
//! scene build で intern して `TextRunId` を確定し、walk は塗るだけ**にする。
//! これは要素テキスト（`el.text_layout` → `NodeKind::TextRun`）が既に取っている
//! 段取りとまったく同じで、draw をその輪に乗せたのがこの型である。
//!
//! 変換は [`SceneDrawCommand::from_wire`] 1 箇所に閉じる。wire enum を網羅的に
//! match するのもそこだけなので、op が増えればコンパイルエラーが必ず指す。

use crate::text_resources::TextRunId;
use crate::wire::protocol::{DrawCommand, DrawPaint, PathVerb};

/// scene graph が保持する draw command。[`crate::node::NodeKind::DrawList`] の要素。
#[derive(Debug, Clone, PartialEq)]
pub enum SceneDrawCommand {
    FillPath {
        verbs: Vec<PathVerb>,
        paint: DrawPaint,
    },
    StrokePath {
        verbs: Vec<PathVerb>,
        paint: DrawPaint,
    },
    ClipPath {
        verbs: Vec<PathVerb>,
    },
    Save,
    Restore,
    Translate {
        dx: f32,
        dy: f32,
    },
    Rotate {
        radians: f32,
    },
    Scale {
        sx: f32,
        sy: f32,
    },
    Transform {
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        e: f32,
        f: f32,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    /// シェープ済みテキスト。`(x, y)` はレイアウトボックスの左上（display list 座標系＝
    /// ボーダーボックス相対）で、`width` / `height` は実測のレイアウト寸法。
    ///
    /// 寸法を持つのは**文字が path verb を持たない**ため: レイヤーのラスタ境界
    /// （[`crate::layer_raster_bounds`]）は verb からは面積を出せず、実測の advance が
    /// 要る。ここで持っておけば境界計算がシェーパを再び呼ばずに済む。
    ///
    /// フォント fallback でランが複数に割れることがあり、`runs` は描画順に全部持つ。
    /// グリフ座標はランごとにレイアウト原点相対なので、全ランが同じ `(x, y)` に載る。
    Text {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        runs: Vec<TextRunId>,
    },
}

impl SceneDrawCommand {
    /// テキスト以外の wire コマンドを scene 語彙へ写す。
    ///
    /// [`DrawCommand::Text`] だけは `None` を返す — シェープ済みのランが要るので、
    /// scene build が [`SceneDrawCommand::Text`] を自分で組み立てる。この
    /// 「文字だけ別扱い」を型で表すために戻り値を `Option` にしてある。
    pub fn from_wire(command: &DrawCommand) -> Option<Self> {
        match command {
            DrawCommand::FillPath { verbs, paint } => Some(Self::FillPath {
                verbs: verbs.clone(),
                paint: paint.clone(),
            }),
            DrawCommand::StrokePath { verbs, paint } => Some(Self::StrokePath {
                verbs: verbs.clone(),
                paint: paint.clone(),
            }),
            DrawCommand::ClipPath { verbs } => Some(Self::ClipPath {
                verbs: verbs.clone(),
            }),
            DrawCommand::Save => Some(Self::Save),
            DrawCommand::Restore => Some(Self::Restore),
            DrawCommand::Translate { dx, dy } => Some(Self::Translate { dx: *dx, dy: *dy }),
            DrawCommand::Rotate { radians } => Some(Self::Rotate { radians: *radians }),
            DrawCommand::Scale { sx, sy } => Some(Self::Scale { sx: *sx, sy: *sy }),
            DrawCommand::Transform { a, b, c, d, e, f } => Some(Self::Transform {
                a: *a,
                b: *b,
                c: *c,
                d: *d,
                e: *e,
                f: *f,
            }),
            DrawCommand::ClipRect {
                x,
                y,
                width,
                height,
            } => Some(Self::ClipRect {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            }),
            DrawCommand::Text { .. } => None,
        }
    }

    /// このコマンドが参照する `TextRunId`（テキスト以外は空）。
    /// resource sweep の生存判定が使う（参照中の id を回収しないため）。
    pub fn text_runs(&self) -> &[TextRunId] {
        match self {
            Self::Text { runs, .. } => runs,
            _ => &[],
        }
    }
}

/// wire の draw list に [`DrawCommand::Text`] が 1 つでもあるか。
///
/// レイアウトパスが「この要素の draw をシェープし直す必要があるか」を判定するのに使う。
/// draw を載せるのは `view` で `is_text_like()` ではないので、これが無いと
/// 非同期で届いたフォントが draw テキストへ反映されない（豆腐のまま固定される）。
pub fn carries_draw_text(commands: &[DrawCommand]) -> bool {
    commands
        .iter()
        .any(|c| matches!(c, DrawCommand::Text { .. }))
}
