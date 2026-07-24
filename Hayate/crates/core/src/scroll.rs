//! スクロールの **Scroll Gesture**（意図分類）と **Scroll Physics Profile**（感触）の
//! 純粋ロジック（ADR-0082 / ADR-0113）。Hayate Core が所有し、各 Platform Adapter は
//! `ScrollGesture` を保持して drain したポインタバッファから駆動し、毎フレーム物理 step を
//! 進め、結果の `Scroll Offset` を適用する薄い配線に徹する。判定（どのポインタがスクロール
//! するか、押下がいつスクロールに変わるか、指の差分がスクロールオフセットへどう写るか）も
//! 物理（慣性・ばね戻し・rubber-band）も全ターゲットでユニットテスト可能な純粋関数として
//! ここに置く。
//!
//! 物理は **iOS 風プロファイル**（[`ScrollPhysicsProfile`]）: 範囲内の 1:1 追従、フリックの
//! 指数減衰慣性、ばね戻し／バウンス付きの sigmoid ラバーバンドオーバースクロール。オフセットは
//! `element_set_scroll_offset`（SCR-02、未クランプ）で適用し、エッジ挙動——越えてドラッグ中の
//! 抵抗、解放後や慣性バウンス後にばねで引き戻す——はここの純粋関数が担う。Android 風
//! （OverScroller spline + Material stretch）プロファイルは将来追加する（ADR-0113）。

/// 物理ポインタデバイスの種別はコアの proto/wire 概念
/// [`PointerKind`](crate::PointerKind)。DOM の `PointerEvent.pointerType` 等から渡され、
/// `touch`/`pen` のみがドラッグ→スクロール経路に入る。`mouse` は選択/ドラッグ挙動のまま。
pub use crate::PointerKind;

/// この種別のポインタがドラッグ→スクロールジェスチャを駆動するか。`Touch` と `Pen` は駆動し、
/// `Mouse` は選択/ドラッグ経路に残る（ADR-0082）。
pub fn is_drag_scroll_pointer(kind: PointerKind) -> bool {
    matches!(kind, PointerKind::Touch | PointerKind::Pen)
}

/// 押下がタップでなくスクロールと見なされるために `pointerdown` から動くべき距離(px)。
/// これ未満なら解放で通常クリックが発火し、超えると押下をキャンセルしてスクロールへ移行する。
/// マジックナンバーにせず一度だけ名前付き定義してチューニング可能にする。iOS 風の既定値。
pub const SCROLL_SLOP_PX: f32 = 8.0;

/// scroll gesture と競合する touch/pen の pressed feedback を保留する時間。
/// Flutter の tap recognizer (`kPressTimeout`) と同じ 100ms。
pub const PRESS_TIMEOUT_MS: f64 = 100.0;

/// `current` が `start` から `slop` px 超（ユークリッド距離）動いたか。
/// デッドゾーンを半径にすることで、斜めのドラッグも全軸で同じ距離で閾値を越える。
pub fn exceeds_slop(start: (f32, f32), current: (f32, f32), slop: f32) -> bool {
    let dx = current.0 - start.0;
    let dy = current.1 - start.1;
    dx * dx + dy * dy > slop * slop
}

/// 1 回の `pointermove` が進行中ジェスチャに与える結果。純粋に判定し、wasm 層はこの結論に従うだけ。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveOutcome {
    /// まだ slop デッドゾーン内——押下は未確定のタップで適用すべきものはない
    /// （押下は生きたままなので解放でまだクリックできる）。
    Pending,
    /// この move が slop を越えた。押下を今キャンセル（`on_pointer_cancel`）し、スクロールへ移行する。
    /// 移行フレームではオフセットを適用しない——デッドゾーンを消費するので跳ねずにここからスクロールが始まる。
    StartScroll,
    /// すでにスクロール中。ロックしたスクロールビューのオフセットをこの指の差分だけ動かす
    /// （範囲内では指に 1:1 で追従し、エッジを越えるとラバーバンドが抵抗する）。
    Scroll { dx: f32, dy: f32 },
}

/// 1 つのスクロールビューにロックされた進行中の touch/pen ドラッグ（ADR-0082）。
/// `pointerdown` の起点（slop 用）、直前位置（move ごとの差分用）、slop を越えたかを追跡する。
/// レンダラはスクロールビュー上の touch/pen `pointerdown` で生成し、drain した move で駆動する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollGesture {
    /// ジェスチャがロックされたスクロールビュー。ジェスチャ中に祖先へ連鎖することはない。
    pub scroll_view: crate::ElementId,
    start: (f32, f32),
    last: (f32, f32),
    scrolling: bool,
}

impl ScrollGesture {
    /// `scroll_view` にロックし、`start`（`pointerdown` 位置）で pending 状態のジェスチャを開始する。
    /// slop を越えるまではスクロールしない。
    pub fn new(scroll_view: crate::ElementId, start: (f32, f32)) -> Self {
        Self {
            scroll_view,
            start,
            last: start,
            scrolling: false,
        }
    }

    /// `pos` への move を分類し、slop/scroll 状態を進める。レンダラが取るべきアクションを返す。
    /// `Scroll` の指の差分は `last - pos`（コンテンツが指に追従。上にドラッグするとコンテンツも上へ）。
    pub fn on_move(&mut self, pos: (f32, f32), slop: f32) -> MoveOutcome {
        if self.scrolling {
            let dx = self.last.0 - pos.0;
            let dy = self.last.1 - pos.1;
            self.last = pos;
            MoveOutcome::Scroll { dx, dy }
        } else if exceeds_slop(self.start, pos, slop) {
            self.scrolling = true;
            self.last = pos;
            MoveOutcome::StartScroll
        } else {
            MoveOutcome::Pending
        }
    }

    /// 今解放したらクリックを発火すべきか。slop を越えていなければ（タップ）true、
    /// スクロールになっていれば false。
    pub fn is_tap(&self) -> bool {
        !self.scrolling
    }
}

// ── 慣性スクロール（ADR-0082） ──
//
// フリックして指を離すと解放速度が摩擦積分器へ渡り、ロックしたスクロールビューを
// 止まるまで（またはエッジに当たってクランプ停止するまで）動かし減速させる。両者とも純粋：
// `CanvasRenderer` はドラッグ中に指のサンプルを記録し、`pointerup` でここの解放速度を推定し、
// rAF フレームごとに減衰を 1 ステップ進める。

/// iOS 風スクロール物理の係数を 1 ブロックに集約し、積分器に散らばるマジックナンバーでなく
/// 各係数を名前付きでチューニング可能なノブとして保つ。fling 上限とばね戻し値は実機で
/// `tuning.json` オーバーレイ経由でキャリブレーションしここに焼き込んだ。ここを調整すれば全体の感触が変わる。
pub mod physics {
    /// 摩擦下でのミリ秒あたりの速度保持率。実機チューニング値：
    /// `t` ms 後に fling は、一定減速を除いて速度の `0.999^t` を保つ
    /// （1 秒後に約 36.8%）。
    pub const DECELERATION_RATE: f32 = 0.999;
    /// 指数摩擦に加えて毎 ms 差し引く一定減速度（px/ms²）。低速域でも速度が確実に
    /// 落ちるようにし、停止しきい値まで長く這い続けるのを防ぐ。
    pub const LINEAR_DECELERATION: f32 = 0.002;
    /// 解放 fling 速度の上限（px/ms ≈ 40000 px/s）。激しいフリックでも 1 フレームで
    /// ドキュメント全体を横切らせないため。実機キャリブレーション済み。
    pub const MAX_RELEASE_VELOCITY: f32 = 40.0;
    /// この速度(px/ms)未満では慣性を停止扱いにして静止へスナップする——60fps フレームで
    /// 約サブピクセル——アニメーションが 0 へ漸近して這い続けず終了するように。
    pub const MIN_VELOCITY: f32 = 0.02;
    /// 最新サンプルからこのウィンドウ(ms)内の指サンプルのみが解放速度の推定に寄与する。
    /// 指を離す前に止めた押下は、古い初期フリックを再生せず静止状態で解放される。
    pub const SAMPLE_WINDOW_MS: f64 = 100.0;

    // ── オーバースクロール／ばね戻し ──

    /// ラバーバンド抵抗定数——エッジ直上で素の指移動量のうちコンテンツに届く割合（曲線の初期傾き）。
    /// iOS の `0.55` に一致：エッジを 1px 越えてドラッグしてもコンテンツは約半ピクセルしか動かず、
    /// 引くほど「重く」なる。
    pub const RUBBER_BAND_C: f32 = 0.55;
    /// オーバースクロールしたエッジを静止へ引き戻すばね剛性（px/ms² per px）。
    /// 実機キャリブレーション済み。穏やかな戻りのためやや柔らかめ。
    pub const SPRING_STIFFNESS: f32 = 0.0001;
    /// ばね減衰（px/ms per px/ms）。活きたバウンスのため臨界（`2 * sqrt(SPRING_STIFFNESS)` ≈ 0.02）
    /// よりわずかに下に保つ。減衰が軽いと通常はバウンスが境界を越えて振動するが、
    /// [`scroll_motion_step`] がバウンスをエッジちょうどで静止へスナップするので
    /// その行き過ぎはクランプされ、軽快な感触だけが残る。実機キャリブレーション済み。
    pub const SPRING_DAMPING: f32 = 0.015;
    /// エッジからの変位(px)がこれ未満ならばね戻しを home と見なす。
    pub const SPRING_REST_OFFSET: f32 = 0.5;
    /// [`SPRING_REST_OFFSET`] 内に入った時、この速度(px/ms)未満でばねがエッジへスナップし
    /// アニメーションを終える。実機キャリブレーション済み。
    pub const SPRING_REST_VELOCITY: f32 = 0.10;

    // ── Android stretch overscroll（ADR-0131） ──

    /// Android 風 stretch overscroll の上限伸び率（一様スケール近似）。エッジがビューポート
    /// 寸法いっぱいに overscroll したとき、ピン留めした端を固定してコンテンツを内側へ最大
    /// `1 + STRETCH_MAX` 倍に伸ばす。iOS profile は使わない（rubber-band translate のまま）。
    /// placeholder **0.15**（実機校正待ち）。マジックナンバーにせず `tuning.json` で再ビルド
    /// なしに上書きできるよう名前付き定数＋ [`ScrollPhysicsTuning`] フィールドにする。
    pub const STRETCH_MAX: f32 = 0.15;
}

/// スクロール物理ノブの、実行時に上書き可能なコピー。[`physics`] 定数（と [`SCROLL_SLOP_PX`]）が
/// 正となる既定値のまま——[`Default`] がそれを読むので数値を再記述しない——だが、開発ビルドは
/// 実行時に値をオーバーレイ（初期化時に読み込む `tuning.json`）し、再コンパイルなしに実機で
/// 感触を調整できる。本番はオーバーライドなしで出荷するので全フィールドが定数に等しく、
/// 読み出しは単なる struct ロード（旧来の `const` 参照に対する性能コストはない）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPhysicsTuning {
    pub slop_px: f32,
    pub deceleration_rate: f32,
    pub linear_deceleration: f32,
    pub max_release_velocity: f32,
    pub min_velocity: f32,
    pub sample_window_ms: f64,
    pub rubber_band_c: f32,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
    pub spring_rest_offset: f32,
    pub spring_rest_velocity: f32,
    pub stretch_max: f32,
}

impl Default for ScrollPhysicsTuning {
    fn default() -> Self {
        // 正の定数をミラーする。ここにリテラルを再記述せず、`physics` ブロックを既定値の唯一の出所に保つ。
        Self {
            slop_px: SCROLL_SLOP_PX,
            deceleration_rate: physics::DECELERATION_RATE,
            linear_deceleration: physics::LINEAR_DECELERATION,
            max_release_velocity: physics::MAX_RELEASE_VELOCITY,
            min_velocity: physics::MIN_VELOCITY,
            sample_window_ms: physics::SAMPLE_WINDOW_MS,
            rubber_band_c: physics::RUBBER_BAND_C,
            spring_stiffness: physics::SPRING_STIFFNESS,
            spring_damping: physics::SPRING_DAMPING,
            spring_rest_offset: physics::SPRING_REST_OFFSET,
            spring_rest_velocity: physics::SPRING_REST_VELOCITY,
            stretch_max: physics::STRETCH_MAX,
        }
    }
}

/// 1 軸の iOS 風ラバーバンド抵抗。`raw` は 1:1 の指ドラッグが到達するオフセットで、戻り値は
/// *表示される* オフセット。`[0, max]` 内ではドラッグはそのまま通り、エッジを越えて引くと
/// コンテンツは遅れ、外へ行くほど強まる抵抗（追加 1px ごとにコンテンツの動きが減る）で
/// `dimension` のオーバースクロールへ漸近する——エッジは「重い」が画面から剥がれない。
/// 両エッジで対称。`dimension` が非正だとオーバースクロールを無効化する
/// （エッジを越えても raw をそのまま返す——ラバーバンドするものがない）。
pub fn rubber_band_offset(raw: f32, max: f32, dimension: f32, t: &ScrollPhysicsTuning) -> f32 {
    let max = max.max(0.0);
    if raw >= 0.0 && raw <= max {
        raw
    } else if raw < 0.0 {
        -overscroll_curve(-raw, dimension, t)
    } else {
        max + overscroll_curve(raw - max, dimension, t)
    }
}

/// エッジを越えた raw な引き `x` px に対する、抵抗込みのオーバースクロール距離：
/// `(1 − 1/(x·c/d + 1))·d`。エッジで 0、そこでの傾きは `c`（[`physics::RUBBER_BAND_C`]）、
/// 上に凸で、`x → ∞` で `dimension` に有界。
fn overscroll_curve(x: f32, dimension: f32, t: &ScrollPhysicsTuning) -> f32 {
    if dimension <= 0.0 || x <= 0.0 {
        return x.max(0.0);
    }
    (1.0 - 1.0 / (x * t.rubber_band_c / dimension + 1.0)) * dimension
}

/// Android 風 stretch overscroll の一様スケール係数（ADR-0131）。`displacement` は符号付きの
/// 越境変位（エッジからの overscroll 距離。iOS profile の rubber-band と同じ既存の越境変位を
/// read し替えるだけで、新しい物理状態は持たない）、`dimension` は当該軸のビューポート寸法。
/// 写像 `1 + clamp(|displacement| / dimension, 0, 1) * stretch_max`：範囲内（変位 0）は 1.0、
/// 越えるほど単調に増え、変位が `dimension` に達すると `1 + stretch_max` で頭打ち（有界）、
/// 両端で対称。`dimension` が非正なら 1.0（伸ばすものがない——ゼロ除算回避）。
pub fn overscroll_stretch_scale(displacement: f32, dimension: f32, t: &ScrollPhysicsTuning) -> f32 {
    if dimension <= 0.0 {
        return 1.0;
    }
    let ratio = (displacement.abs() / dimension).clamp(0.0, 1.0);
    1.0 + ratio * t.stretch_max
}

/// 速度(px/ms)を対称な解放上限にクランプする。
fn cap_release_velocity(v: f32, t: &ScrollPhysicsTuning) -> f32 {
    v.clamp(-t.max_release_velocity, t.max_release_velocity)
}

/// 到着順の指サンプル列 `(x, y, timestamp_ms)` から、解放(fling)速度を **オフセット空間**
/// (px/ms) で推定する。オフセット空間はスクロールオフセットの符号規約——コンテンツが指に追従——
/// なので、指が上へ滑ると正の `vy` を返す（ドラッグ差分がオフセットを動かす向きと同じ）。
///
/// 最新サンプルから [`physics::SAMPLE_WINDOW_MS`] 内のサンプルのみが寄与するので、
/// 離す前に止めた指は静止状態で解放される。推定値はそのウィンドウ全体（最初→最後）の平均速度で、
/// 軸ごとに [`physics::MAX_RELEASE_VELOCITY`] で上限を切る。ウィンドウ内サンプルが 2 未満、
/// または所要時間が 0 の場合は fling なし `(0.0, 0.0)`。
pub fn estimate_release_velocity(
    samples: &[(f32, f32, f64)],
    t: &ScrollPhysicsTuning,
) -> (f32, f32) {
    let Some(&(last_x, last_y, last_t)) = samples.last() else {
        return (0.0, 0.0);
    };
    let window_start = last_t - t.sample_window_ms;
    let Some(&(first_x, first_y, first_t)) = samples.iter().find(|&&(_, _, ts)| ts >= window_start)
    else {
        return (0.0, 0.0);
    };
    let dt = (last_t - first_t) as f32;
    if dt <= 0.0 {
        return (0.0, 0.0);
    }
    // オフセットは指と逆向きに動く：オフセット差分 = old_pos − new_pos。
    (
        cap_release_velocity((first_x - last_x) / dt, t),
        cap_release_velocity((first_y - last_y) / dt, t),
    )
}

/// 慣性の 1 軸を指数摩擦＋一定減速下で `dt_ms` 進める。今フレーム適用するオフセット差分と、
/// 次へ持ち越す減衰後の速度を返す（ともにオフセット空間、px と px/ms）。指数摩擦を適用後、
/// [`physics::LINEAR_DECELERATION`] × `dt_ms` を速度の絶対値から差し引く。ゼロをまたいで
/// 逆向きに加速することはない。減衰後の速度が
/// [`physics::MIN_VELOCITY`] を下回ると `0.0` へスナップするので、呼び出し側は漸近的な
/// 這いを積分し続けずアニメーションを終えられる。
pub fn momentum_step(velocity: f32, dt_ms: f32, t: &ScrollPhysicsTuning) -> (f32, f32) {
    let delta = velocity * dt_ms;
    let exponentially_decayed = velocity * t.deceleration_rate.powf(dt_ms);
    let linear_loss = t.linear_deceleration.max(0.0) * dt_ms;
    let next =
        exponentially_decayed.signum() * (exponentially_decayed.abs() - linear_loss).max(0.0);
    if next.abs() < t.min_velocity {
        (delta, 0.0)
    } else {
        (delta, next)
    }
}

/// ばね戻しの 1 軸をエッジへ向けて `dt_ms` 進める。`displacement` はエッジからの符号付き
/// オーバースクロール距離（上を越えると負、下を越えると正）、`velocity` はその変化率
/// （px/ms、オフセット空間）。ほぼ臨界減衰のばね——[`physics::SPRING_STIFFNESS`] /
/// [`physics::SPRING_DAMPING`]——が変位を 0 へ引く：オーバースクロール中に解放された指は
/// 緩やかに戻り、エッジを越えてバウンスした fling（外向き速度で進入）は行き過ぎてから振動せず戻る。
/// 次の `(displacement, velocity)` を返し、両者が rest 閾値に入ると `(0.0, 0.0)`（home、アニメ終了）へスナップする。
pub fn spring_step(
    displacement: f32,
    velocity: f32,
    dt_ms: f32,
    t: &ScrollPhysicsTuning,
) -> (f32, f32) {
    // 半陰的（シンプレクティック）オイラー：先に速度、次に位置を積分し、
    // フレームサイズの dt でばねを安定に保つ。
    let accel = -t.spring_stiffness * displacement - t.spring_damping * velocity;
    let next_v = velocity + accel * dt_ms;
    let next_x = displacement + next_v * dt_ms;
    if next_x.abs() < t.spring_rest_offset && next_v.abs() < t.spring_rest_velocity {
        (0.0, 0.0)
    } else {
        (next_x, next_v)
    }
}

/// 解放されたスクロールの 1 軸を `dt_ms` 進め、オフセットの位置に応じて適切な物理を選ぶ。
/// `[0, max]` 内では摩擦で惰走する（[`momentum_step`]）。エッジを越えて走る fling は速度を保って
/// オーバースクロールへ渡り、そこで次フレームの [`spring_step`] が引き戻す——エッジに達した慣性は
/// バウンスして戻る。すでにオーバースクロール中の解放はまっすぐ home へばねで戻る。次の
/// `(offset, velocity)` を返す。オフセットは未クランプ（SCR-02）——オーバースクロールこそが目的だから。
/// 呼び出し側は速度が静止し **かつ** オフセットが `[0, max]` 内に戻った時点でアニメーションを止める。
///
/// バウンスは当たったエッジ **ちょうど** で静止する：ばねがコンテンツをエッジを越えて範囲内へ
/// 戻そうとする時、残った内向き速度を [`momentum_step`] に渡さず、オフセットを速度 0 でエッジへ
/// スナップする。よって fling は境界をちょうど 1 回だけ行き過ぎてそこで静止する——ばねをどう
/// チューニングしても、境界を再度越えて両エッジ間でピンポンすることはない。
pub fn scroll_motion_step(
    offset: f32,
    velocity: f32,
    max: f32,
    dt_ms: f32,
    t: &ScrollPhysicsTuning,
) -> (f32, f32) {
    let max = max.max(0.0);
    if offset < 0.0 {
        // 上のエッジ（edge = 0）を越えた：そこへばねで戻す。非負の結果は
        // ばねがエッジに到達／越えたことを意味する——ちょうどそこで静止させる。
        let (disp, v) = spring_step(offset, velocity, dt_ms, t);
        if disp >= 0.0 {
            (0.0, 0.0)
        } else {
            (disp, v)
        }
    } else if offset > max {
        // 下のエッジ（edge = max）を越えた：対称——非正の変位はエッジへ戻った／越えたことを
        // 意味するので `max` で静止させる。
        let (disp, v) = spring_step(offset - max, velocity, dt_ms, t);
        if disp <= 0.0 {
            (max, 0.0)
        } else {
            (max + disp, v)
        }
    } else {
        let (delta, v) = momentum_step(velocity, dt_ms, t);
        (offset + delta, v)
    }
}

/// スクロール物理の「感触」を選ぶ閉じた語彙（ADR-0113）。`Auto` は実行プラットフォームに
/// 応じて各 OS 相当の感触へ解決する設計（完成形）。iOS 風（指数減衰＋sigmoid rubber-band。
/// 本モジュールの物理）と Android 風（OverScroller spline＋Material stretch）は別アルゴリズムで、
/// いずれも Hayate Core が所有する。
///
/// `Auto` は唯一 platform 解決される感触で、現状は iOS 風プロファイルへ解決する（既定・不変）。
/// `Android` は Material 風 stretch overscroll（一様スケール近似、ADR-0131）を **scene lowering
/// 一箇所**で選ぶ。物理・保存 `scroll_offset`・Scroll Gesture・`scroll` イベント・スクロールバー
/// indicator は iOS と完全パリティで、感触差は overscroll の見せ方（rubber-band translate か
/// 一様スケール stretch か）だけ。明示 `Ios` 選択と Android の spline fling は将来（ADR-0113/0131）。
/// Scroll Gesture（意図分類）とは別軸。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollPhysicsProfile {
    /// 実行プラットフォームに合わせて解決する（現状は iOS 風プロファイル）。
    #[default]
    Auto,
    /// Android 風の「画面が伸びる」stretch overscroll（一様スケール近似、ADR-0131）。dev の
    /// `tuning.json` の `profile: "android"` でのみ選ぶ。`Auto` の UA 自動解決は将来。
    Android,
}

impl ScrollPhysicsProfile {
    /// この感触の既定物理チューニング。`Auto` は iOS 風プロファイルへ解決する。`Android` は
    /// 物理（fling/spring/rubber-band）が iOS とパリティなので同じ既定値を返す——感触差は
    /// scene lowering の overscroll 表現のみ（[`uses_stretch_overscroll`](Self::uses_stretch_overscroll)）。
    /// Platform Adapter は必要なら開発時にこの上へ値をオーバーレイする（例: web の `tuning.json`）。本番は素の既定値。
    pub fn default_tuning(self) -> ScrollPhysicsTuning {
        match self {
            ScrollPhysicsProfile::Auto | ScrollPhysicsProfile::Android => {
                ScrollPhysicsTuning::default()
            }
        }
    }

    /// この感触が overscroll を Android 風の一様スケール stretch で見せるか（ADR-0131）。iOS 風
    /// （`Auto`）は overshoot を content translate の rubber-band で見せるので `false`。scene
    /// lowering の scroll group アフィン合成はこの一点だけで分岐する。
    pub fn uses_stretch_overscroll(self) -> bool {
        matches!(self, ScrollPhysicsProfile::Android)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 物理関数用の既定チューニング——正の定数に等しいので、追加パラメータによって
    /// 以下の挙動アサーションは変わらない。
    fn t() -> ScrollPhysicsTuning {
        ScrollPhysicsTuning::default()
    }

    #[test]
    fn default_tuning_mirrors_the_authoritative_consts() {
        // `ScrollPhysicsTuning::default()` が `physics` ブロックを反映する不変条件を固定する：
        // struct の更新を忘れた将来の const 編集を検出する。
        let d = ScrollPhysicsTuning::default();
        assert_eq!(d.slop_px, SCROLL_SLOP_PX);
        assert_eq!(d.deceleration_rate, physics::DECELERATION_RATE);
        assert_eq!(d.linear_deceleration, physics::LINEAR_DECELERATION);
        assert_eq!(d.max_release_velocity, physics::MAX_RELEASE_VELOCITY);
        assert_eq!(d.min_velocity, physics::MIN_VELOCITY);
        assert_eq!(d.sample_window_ms, physics::SAMPLE_WINDOW_MS);
        assert_eq!(d.rubber_band_c, physics::RUBBER_BAND_C);
        assert_eq!(d.spring_stiffness, physics::SPRING_STIFFNESS);
        assert_eq!(d.spring_damping, physics::SPRING_DAMPING);
        assert_eq!(d.spring_rest_offset, physics::SPRING_REST_OFFSET);
        assert_eq!(d.spring_rest_velocity, physics::SPRING_REST_VELOCITY);
        assert_eq!(d.stretch_max, physics::STRETCH_MAX);
    }

    #[test]
    fn stretch_max_is_a_named_tunable_constant_not_a_magic_number() {
        // Android stretch overscroll の上限伸び率を単一の名前付きノブに保つため固定。
        // placeholder 0.15（実機校正待ち）。マジックナンバーにしない。
        assert_eq!(physics::STRETCH_MAX, 0.15);
        // default チューニングが定数を反映するので、`tuning.json` で再ビルドなしに上書きできる。
        assert_eq!(
            ScrollPhysicsTuning::default().stretch_max,
            physics::STRETCH_MAX
        );
    }

    #[test]
    fn overscroll_stretch_scale_is_unity_inside_range_grows_bounded_and_symmetric() {
        let t = t();
        let dim = 200.0;
        // 範囲内（越境変位 0）は伸びなし。
        assert_eq!(overscroll_stretch_scale(0.0, dim, &t), 1.0);
        // 越境すると 1.0 超で単調増加。
        let near = overscroll_stretch_scale(-20.0, dim, &t);
        let mid = overscroll_stretch_scale(-80.0, dim, &t);
        let far = overscroll_stretch_scale(-160.0, dim, &t);
        assert!(near > 1.0, "past the edge stretches (got {near})");
        assert!(mid > near && far > mid, "monotonic increase outward");
        // STRETCH_MAX で頭打ち（有界）: 変位が dimension を超えても 1 + STRETCH_MAX を超えない。
        let capped = overscroll_stretch_scale(-100_000.0, dim, &t);
        assert!(
            (capped - (1.0 + physics::STRETCH_MAX)).abs() < 1e-6,
            "bounded at 1 + STRETCH_MAX (got {capped})",
        );
        // ちょうど dimension で頭打ちに達する。
        assert!(
            (overscroll_stretch_scale(dim, dim, &t) - (1.0 + physics::STRETCH_MAX)).abs() < 1e-6
        );
        // 両端対称: 同じ大きさの正負の変位は同じスケール。
        assert_eq!(
            overscroll_stretch_scale(-50.0, dim, &t),
            overscroll_stretch_scale(50.0, dim, &t),
        );
    }

    #[test]
    fn overscroll_stretch_scale_has_no_stretch_without_a_dimension() {
        // dimension が非正なら伸ばすものがない（ゼロ除算回避）。
        assert_eq!(overscroll_stretch_scale(-50.0, 0.0, &t()), 1.0);
    }

    #[test]
    fn android_profile_uses_stretch_overscroll_while_auto_stays_ios() {
        // 既定は不変: Auto は iOS 風 profile へ解決し stretch を使わない（rubber-band translate）。
        assert_eq!(ScrollPhysicsProfile::default(), ScrollPhysicsProfile::Auto);
        assert!(!ScrollPhysicsProfile::Auto.uses_stretch_overscroll());
        assert_eq!(
            ScrollPhysicsProfile::Auto.default_tuning(),
            ScrollPhysicsTuning::default(),
        );
        // Android は一様スケール stretch overscroll を使う。物理チューニングは iOS とパリティ。
        assert!(ScrollPhysicsProfile::Android.uses_stretch_overscroll());
        assert_eq!(
            ScrollPhysicsProfile::Android.default_tuning(),
            ScrollPhysicsTuning::default(),
        );
    }

    #[test]
    fn touch_and_pen_drive_scroll_but_mouse_does_not() {
        assert!(is_drag_scroll_pointer(PointerKind::Touch));
        assert!(is_drag_scroll_pointer(PointerKind::Pen));
        assert!(!is_drag_scroll_pointer(PointerKind::Mouse));
    }

    #[test]
    fn slop_is_a_named_tunable_constant_not_a_magic_number() {
        // デッドゾーンを単一の名前付きノブに保つため固定。値は iOS 風。
        assert_eq!(SCROLL_SLOP_PX, 8.0);
    }

    #[test]
    fn movement_within_the_slop_radius_is_not_yet_a_scroll() {
        let start = (100.0, 100.0);
        // 直進 5px と斜め約 7.07px——どちらも 8px 半径の内側。
        assert!(!exceeds_slop(start, (105.0, 100.0), SCROLL_SLOP_PX));
        assert!(!exceeds_slop(start, (105.0, 105.0), SCROLL_SLOP_PX));
    }

    #[test]
    fn movement_past_the_slop_radius_becomes_a_scroll() {
        let start = (100.0, 100.0);
        assert!(exceeds_slop(start, (100.0, 109.0), SCROLL_SLOP_PX));
        assert!(exceeds_slop(start, (108.1, 100.0), SCROLL_SLOP_PX));
    }

    fn sv() -> crate::ElementId {
        crate::ElementId::from_u64(1)
    }

    #[test]
    fn release_velocity_is_the_offset_space_speed_over_the_recent_samples() {
        // 指が 60ms かけて上へ滑る（y: 100 → 40）。コンテンツが指に追従するので
        // オフセットは 60ms で 60px 増え → オフセット空間で +1 px/ms。X は静止。
        let samples = [(0.0, 100.0, 0.0), (0.0, 70.0, 30.0), (0.0, 40.0, 60.0)];
        let (vx, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(vx, 0.0);
        assert!((vy - 1.0).abs() < 1e-6, "vy = {vy}");
    }

    #[test]
    fn release_velocity_needs_two_in_window_samples_with_a_real_time_span() {
        // サンプルなし、または 1 個だけでは速度を測る基準がない。
        assert_eq!(estimate_release_velocity(&[], &t()), (0.0, 0.0));
        assert_eq!(
            estimate_release_velocity(&[(0.0, 0.0, 5.0)], &t()),
            (0.0, 0.0)
        );
        // 同一時刻のサンプル 2 個：経過時間ゼロでの位置ジャンプは測定可能な速度ではない
        // （ゼロ除算を回避）。
        assert_eq!(
            estimate_release_velocity(&[(0.0, 0.0, 5.0), (0.0, 50.0, 5.0)], &t()),
            (0.0, 0.0),
        );
    }

    #[test]
    fn samples_older_than_the_window_are_ignored_so_a_pause_releases_at_rest() {
        // ずっと前の速い滑り、その後指は離す直前の 60ms を y=40 で静止——
        // ウィンドウ内なのは静止サンプルだけ。
        let samples = [
            (0.0, 200.0, 0.0), // 離す前の 100ms ウィンドウの外
            (0.0, 40.0, 500.0),
            (0.0, 40.0, 560.0),
        ];
        let (_, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(
            vy, 0.0,
            "a finger that paused before lifting releases at rest"
        );
    }

    #[test]
    fn release_velocity_is_capped_so_a_violent_flick_cannot_launch_too_far() {
        // 10ms で 1000px = 100 px/ms、上限をはるかに超える。
        let samples = [(0.0, 1000.0, 0.0), (0.0, 0.0, 10.0)];
        let (_, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(vy, physics::MAX_RELEASE_VELOCITY);
    }

    #[test]
    fn momentum_advances_in_its_direction_and_friction_bleeds_the_speed() {
        // 16ms フレーム：オフセットは速度方向へ進み、持ち越す速度は小さくなるが符号は保つ
        // （反転せず減速）。
        let (delta, next) = momentum_step(2.0, 16.0, &t());
        assert!(delta > 0.0, "offset advances in the velocity direction");
        assert!(
            next > 0.0 && next < 2.0,
            "friction bleeds speed, keeps sign (next = {next})"
        );
        // 下向きの fling でも対称。
        let (delta_neg, next_neg) = momentum_step(-2.0, 16.0, &t());
        assert!(delta_neg < 0.0);
        assert!(next_neg < 0.0 && next_neg > -2.0);
    }

    #[test]
    fn momentum_combines_proportional_and_constant_deceleration_symmetrically() {
        let mut tuning = t();
        tuning.deceleration_rate = 0.5;
        tuning.linear_deceleration = 0.1;

        let (_, positive) = momentum_step(4.0, 1.0, &tuning);
        let (_, negative) = momentum_step(-4.0, 1.0, &tuning);
        assert!((positive - 1.9).abs() < 1e-6);
        assert!((negative + 1.9).abs() < 1e-6);
    }

    #[test]
    fn constant_deceleration_stops_at_zero_without_reversing_direction() {
        let mut tuning = t();
        tuning.deceleration_rate = 1.0;
        tuning.linear_deceleration = 0.1;
        tuning.min_velocity = 0.0;

        assert_eq!(momentum_step(0.05, 1.0, &tuning).1, 0.0);
        assert_eq!(momentum_step(-0.05, 1.0, &tuning).1, -0.0);
    }

    #[test]
    fn momentum_snaps_to_rest_once_it_drops_below_the_stop_threshold() {
        // 閾値ちょうどから始め、1 つの長いフレームで MIN_VELOCITY 未満まで減衰させ、
        // 永遠に這わず 0 へスナップする。
        let (_, next) = momentum_step(physics::MIN_VELOCITY, 1000.0, &t());
        assert_eq!(next, 0.0, "below the stop threshold momentum ends");
    }

    #[test]
    fn physics_coefficients_are_named_constants_gathered_in_one_place() {
        // iOS 風ノブを、積分器に散らばるマジックナンバーでなく単一のチューニング可能ブロックに
        // 保つため固定。
        assert_eq!(physics::DECELERATION_RATE, 0.999);
        assert_eq!(physics::LINEAR_DECELERATION, 0.002);
        assert_eq!(physics::MAX_RELEASE_VELOCITY, 40.0);
        assert_eq!(physics::MIN_VELOCITY, 0.02);
        assert_eq!(physics::SAMPLE_WINDOW_MS, 100.0);
        // オーバースクロール／ばね戻しのノブも同じブロックにある。
        assert_eq!(physics::RUBBER_BAND_C, 0.55);
        assert_eq!(physics::SPRING_STIFFNESS, 0.0001);
        assert_eq!(physics::SPRING_DAMPING, 0.015);
        assert_eq!(physics::SPRING_REST_OFFSET, 0.5);
        assert_eq!(physics::SPRING_REST_VELOCITY, 0.10);
        // Android stretch のノブも同じブロックにある。
        assert_eq!(physics::STRETCH_MAX, 0.15);
    }

    #[test]
    fn within_range_the_drag_follows_the_finger_one_to_one() {
        // スクロール可能範囲内ではラバーバンドなし：両端と中間で表示オフセットは素の指オフセットに等しい。
        assert_eq!(rubber_band_offset(0.0, 400.0, 200.0, &t()), 0.0);
        assert_eq!(rubber_band_offset(150.0, 400.0, 200.0, &t()), 150.0);
        assert_eq!(rubber_band_offset(400.0, 400.0, 200.0, &t()), 400.0);
    }

    #[test]
    fn pulling_past_an_edge_resists_so_the_content_lags_the_finger() {
        // 上のエッジを越えた素の引き 100px は 100px 未満のオーバースクロール（抵抗込み）を示し、
        // エッジのオーバースクロール側に留まる。
        let shown = rubber_band_offset(-100.0, 400.0, 200.0, &t());
        assert!(shown < 0.0, "overscroll is past the top edge (got {shown})");
        assert!(
            shown > -100.0,
            "resisted: content lags the finger (got {shown})"
        );
        // 下のエッジ（max = 400）を越えても対称。
        let shown_bottom = rubber_band_offset(500.0, 400.0, 200.0, &t());
        assert!(
            shown_bottom > 400.0 && shown_bottom < 500.0,
            "got {shown_bottom}"
        );
        assert!(
            (shown_bottom - 400.0 + shown).abs() < 1e-3,
            "the curve is symmetric at both edges",
        );
    }

    #[test]
    fn the_further_past_the_edge_the_heavier_each_extra_pixel_moves() {
        // 等しい素の増分がますます小さい表示増分を生む：ラバーバンドの逓減する「重い」感触。
        let near = rubber_band_offset(-50.0, 400.0, 200.0, &t()).abs();
        let mid = rubber_band_offset(-100.0, 400.0, 200.0, &t()).abs();
        let far = rubber_band_offset(-150.0, 400.0, 200.0, &t()).abs();
        let first_step = mid - near;
        let second_step = far - mid;
        assert!(mid > near && far > mid, "still monotonic outward");
        assert!(
            second_step < first_step,
            "each further pull moves the content less ({second_step} !< {first_step})",
        );
    }

    #[test]
    fn overscroll_is_bounded_so_the_content_never_tears_off_screen() {
        // 巨大な引きでも `dimension` を超えるオーバースクロールは現れない。
        let extreme = rubber_band_offset(-100_000.0, 400.0, 200.0, &t());
        assert!(
            extreme > -200.0,
            "overscroll asymptotes to the dimension (got {extreme})"
        );
    }

    #[test]
    fn spring_back_eases_an_overscrolled_edge_toward_home() {
        // 上のエッジを 60px 越えた位置で静止のまま解放：ばねが変位を 0 へ引き戻す
        // （絶対値が小さくなり、内向きに動く）。
        let (x, v) = spring_step(-60.0, 0.0, 16.0, &t());
        assert!(
            x > -60.0 && x < 0.0,
            "displacement shrinks toward the edge (got {x})"
        );
        assert!(v > 0.0, "velocity points back toward the edge (got {v})");
    }

    #[test]
    fn spring_back_converges_to_the_edge_and_ends() {
        // 静止した深いオーバースクロールから、繰り返しのステップは有限フレームで home (0,0) に
        // 到達しなければならない——アニメーションは終了し、這い続けない。
        let mut x = -120.0;
        let mut v = 0.0;
        let mut frames = 0;
        while (x, v) != (0.0, 0.0) {
            let (nx, nv) = spring_step(x, v, 16.0, &t());
            x = nx;
            v = nv;
            frames += 1;
            assert!(frames < 1000, "spring-back must settle, not ring forever");
        }
        assert_eq!((x, v), (0.0, 0.0));
    }

    #[test]
    fn a_fling_bounce_overshoots_past_the_edge_then_returns() {
        // 慣性が外向きに動いたままエッジ（変位 0）に達する：ばねはそれを越えてバウンスさせ、
        // 反対側に渡らず home へ戻す（臨界減衰、振動なし）。エッジで外向き速度で打ち出した軌跡を 1 本追う。
        let mut x = 0.0_f32;
        let mut v = -2.0_f32;
        let mut min_x = 0.0_f32;
        for _ in 0..1000 {
            let (nx, nv) = spring_step(x, v, 16.0, &t());
            x = nx;
            v = nv;
            min_x = min_x.min(x);
            if (x, v) == (0.0, 0.0) {
                break;
            }
        }
        assert!(
            min_x < 0.0,
            "the bounce carried the content past the edge (min {min_x})"
        );
        assert_eq!((x, v), (0.0, 0.0), "and eased back to rest at the edge");
    }

    #[test]
    fn spring_back_snaps_home_once_within_the_rest_thresholds() {
        // ほぼゼロ速度のサブピクセル変位は home——漸近させずエッジへスナップしてアニメーションを止める。
        assert_eq!(spring_step(0.2, 0.0, 16.0, &t()), (0.0, 0.0));
    }

    #[test]
    fn motion_inside_the_range_coasts_under_friction() {
        // [0, 400] の十分内側では、解放された fling は素の慣性のように減速する：
        // オフセットは速度方向へ進み、速度は減衰する。
        let (offset, v) = scroll_motion_step(100.0, 2.0, 400.0, 16.0, &t());
        assert!(offset > 100.0, "coasts forward (got {offset})");
        assert!(v > 0.0 && v < 2.0, "friction bleeds the speed (got {v})");
    }

    #[test]
    fn inertia_reaching_an_edge_carries_past_it_into_overscroll() {
        // 下のエッジを行き過ぎる fling は外向きに動いたままオーバースクロール（offset > max）へ
        // 渡るので、次フレームでバウンスして戻せる。
        let (offset, v) = scroll_motion_step(395.0, 2.0, 400.0, 16.0, &t());
        assert!(
            offset > 400.0,
            "inertia carries past the edge (got {offset})"
        );
        assert!(
            v > 0.0,
            "still moving outward, to be sprung back next frame (got {v})"
        );
    }

    #[test]
    fn motion_in_overscroll_springs_back_toward_the_edge() {
        // 下のエッジを越えた静止状態：ばね戻しがオフセットを max へ引き、速度は内向きを指す。
        let (offset, v) = scroll_motion_step(440.0, 0.0, 400.0, 16.0, &t());
        assert!(
            offset < 440.0 && offset > 400.0,
            "eases back toward the edge (got {offset})"
        );
        assert!(v < 0.0, "velocity points back inward (got {v})");
        // 上のエッジを越えても対称。
        let (top_offset, top_v) = scroll_motion_step(-40.0, 0.0, 400.0, 16.0, &t());
        assert!(top_offset < 0.0 && top_offset > -40.0, "got {top_offset}");
        assert!(top_v > 0.0, "got {top_v}");
    }

    #[test]
    fn a_flick_that_overruns_the_edge_bounces_and_settles_back_at_it() {
        // 純粋層でのエンドツーエンド：強い fling が下のエッジを行き過ぎ、あるフレームで
        // オーバースクロール中に観測され、その後ばね戻しがエッジちょうどで静止へ戻す。
        let max = 400.0;
        let mut offset = 380.0;
        let mut v = 3.0; // エッジまで残り 20px を行き過ぎるのに十分な強さ
        let mut max_seen = offset;
        let mut settled = None;
        for frame in 0..2000 {
            let (no, nv) = scroll_motion_step(offset, v, max, 16.0, &t());
            offset = no;
            v = nv;
            max_seen = max_seen.max(offset);
            if nv == 0.0 && (0.0..=max).contains(&offset) {
                settled = Some(frame);
                break;
            }
        }
        assert!(
            max_seen > max,
            "the fling bounced into overscroll (peak {max_seen})"
        );
        assert!(settled.is_some(), "and the bounce settled");
        assert!(
            (offset - max).abs() < 1.0,
            "resting at the edge (got {offset})"
        );
    }

    #[test]
    fn a_bounce_settles_at_the_edge_and_never_re_crosses_the_boundary() {
        // 構造的保証：fling が一度オーバースクロールへバウンスすると、ばねは当たったエッジ
        // ちょうどで静止させる——残った内向き速度は決して慣性へ戻されないので、コンテンツが
        // 範囲を横切って撃ち返され両エッジ間でピンポンすることはない。下のエッジをはるかに
        // 越える激しいフリックを駆動し、範囲へ戻るフレームを見る：エッジちょうどで静止して
        // 到着し、余った速度を持って再進入してはならない。
        let max = 400.0;
        let mut offset = 380.0;
        let mut v = 8.0; // エッジまで残り 20px をはるかに行き過ぎる
        let mut re_entered_with_speed = false;
        let mut rest_offset = None;
        for _ in 0..4000 {
            let (no, nv) = scroll_motion_step(offset, v, max, 16.0, &t());
            // オーバースクロール（offset > max）から範囲内への遷移。
            if offset > max && no <= max {
                re_entered_with_speed = nv != 0.0 || (no - max).abs() > 1e-3;
            }
            offset = no;
            v = nv;
            if v == 0.0 && (0.0..=max).contains(&offset) {
                rest_offset = Some(offset);
                break;
            }
        }
        assert!(
            !re_entered_with_speed,
            "the bounce re-crossed the boundary carrying speed"
        );
        assert_eq!(
            rest_offset,
            Some(max),
            "the fling settled exactly at the edge it hit"
        );
    }

    #[test]
    fn a_move_within_slop_keeps_the_gesture_a_pending_tap() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        assert_eq!(
            g.on_move((104.0, 100.0), SCROLL_SLOP_PX),
            MoveOutcome::Pending
        );
        assert!(
            g.is_tap(),
            "an unresolved press is still a tap → click on release"
        );
    }

    #[test]
    fn crossing_slop_takes_over_scrolling_without_applying_a_delta() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        // 上へ 20px は 8px デッドゾーンを越える。
        assert_eq!(
            g.on_move((100.0, 80.0), SCROLL_SLOP_PX),
            MoveOutcome::StartScroll
        );
        assert!(!g.is_tap(), "after takeover a release must not click");
    }

    #[test]
    fn while_scrolling_content_follows_the_finger_one_to_one() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        g.on_move((100.0, 80.0), SCROLL_SLOP_PX); // 移行、last = (100,80)
                                                  // 指が y=60 まで上昇し続ける：コンテンツが追従 → オフセットが 20 増える。
        assert_eq!(
            g.on_move((100.0, 60.0), SCROLL_SLOP_PX),
            MoveOutcome::Scroll { dx: 0.0, dy: 20.0 },
        );
        // 指が y=70 まで下がる：オフセットが 10 減る。差分は起点でなく直前の move から測る。
        assert_eq!(
            g.on_move((100.0, 70.0), SCROLL_SLOP_PX),
            MoveOutcome::Scroll { dx: 0.0, dy: -10.0 },
        );
    }
}
