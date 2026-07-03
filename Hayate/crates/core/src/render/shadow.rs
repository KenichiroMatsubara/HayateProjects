//! box-shadow のガウスぼかしを、解析パスを持たない painter 向けに近似する共有ロジック。
//!
//! ぼかし角丸矩形プリミティブ（[`crate::node::NodeKind::BlurredRoundedRect`]）の
//! default フォールバック（[`crate::render::ScenePainter::fill_blurred_rounded_rect`]）と、
//! inset 影の帯フィル（`element::scene_build`）が同じ erf 減衰式を共有する。tunable は
//! すべて名前付き定数で、後続の人力チューニング（別 issue）でビルド不要に詰められる。

/// 影のガウスぼかしを近似する同心半透明シェルの枚数（ADR-0095）。解析パスを持たない
/// painter（`fill_blurred_rounded_rect` の default 実装）が使うフォールバック。blur ≈
/// 重なる半透明角丸矩形。
pub const SHADOW_BLUR_FALLBACK_LAYERS: usize = 10;

/// シェルを積む外向き距離（reach）を σ の何倍まで伸ばすか。σ の数倍で falloff は
/// 無視できる（~0.4% base）ため、それ以遠のシェルは描かない。
pub const SHADOW_REACH_SIGMA_FACTOR: f32 = 2.7;

/// reach の上限を blur 全幅の何倍にクランプするか。σ = blur/2 の関係下では
/// [`SHADOW_REACH_SIGMA_FACTOR`] が常に効くが、両係数を名前付きで残し後続チューニングを容易にする。
pub const SHADOW_REACH_BLUR_FACTOR: f32 = 1.5;

/// これ以下の blur は「ぼかしなし」のハードシャドウとして単色 1 枚で塗る閾値。
pub const HARD_SHADOW_BLUR_THRESHOLD: f32 = 0.5;

/// CSS の blur 半径 `b` は標準偏差 σ = b/2 のガウスぼかしに対応する（CSS Backgrounds & Borders）。
pub fn shadow_sigma(blur: f32) -> f32 {
    (blur * 0.5).max(f32::MIN_POSITIVE)
}

/// 相補誤差関数 erfc(x)（Abramowitz & Stegun 7.1.26、最大誤差 ~1.5e-7）。
/// ぼかしたシャドウ縁のアルファは、ステップ関数をガウスで畳み込んだ
/// `base/2 · erfc(d / (σ√2))` で与えられる（d はシャドウ外形からの外向き距離）。
fn erfc(x: f32) -> f32 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * z);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erf = 1.0 - poly * (-z * z).exp();
    let erf = if x < 0.0 { -erf } else { erf };
    1.0 - erf
}

/// シャドウ外形の縁から外側へ距離 `d` の点における、ぼかしたシャドウのアルファ係数
/// （ピーク `base_a` に対する比 0..0.5）。
pub fn shadow_falloff(d: f32, sigma: f32) -> f32 {
    0.5 * erfc(d / (sigma * std::f32::consts::SQRT_2))
}

/// ぼかし角丸矩形の影を同心角丸シェルへ分解し、可視シェルごとに `paint` を呼ぶ。
///
/// `(x, y, width, height)` は影外形（オフセット・spread 適用済みの矩形）、`corner_radius` は
/// その角丸半径、`std_dev` はガウス σ、`color` は影色（straight RGBA・不透明度適用済み）。
/// `paint(x, y, width, height, color, corner_radius)` は 1 シェル分の角丸矩形塗り。
///
/// 解析プリミティブを持たない painter の default フォールバック（erf シェル近似）。CSS の
/// blur は σ = blur/2 のガウスなので、外形の縁から外向き距離 d でのアルファは
/// `base · shadow_falloff(d)`（縁で base/2、σ の数倍で実質 0）。各シェルのアルファを falloff の
/// 差分にすると加算的に重なってこの減衰を再現する。i=0 の核（grow=0, α=base/2）と外向きシェル
/// （telescope）の合計は外形内部で base に達し、外側はガウス減衰する。外側（薄い大きな矩形）から
/// 内側へ重ねる。
pub fn for_each_shadow_shell(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    std_dev: f32,
    color: [f32; 4],
    mut paint: impl FnMut(f32, f32, f32, f32, [f32; 4], f32),
) {
    let sigma = std_dev;
    // σ = blur/2 なので blur = 2σ。reach クランプは両係数を名前付きで保持する。
    let blur = std_dev * 2.0;
    let reach = (sigma * SHADOW_REACH_SIGMA_FACTOR).min(blur * SHADOW_REACH_BLUR_FACTOR);
    let n = SHADOW_BLUR_FALLBACK_LAYERS;
    for i in (0..=n).rev() {
        let grow = reach * (i as f32) / (n as f32);
        let cover = if i == 0 {
            // 核：外形内部を base まで満たすぶん（境界での値 base/2）。
            0.5_f32
        } else {
            let d_inner = reach * ((i - 1) as f32) / (n as f32);
            shadow_falloff(d_inner, sigma) - shadow_falloff(grow, sigma)
        };
        // アルファ変調は f64 で行い（`Color::a` は f64）、旧 `emit_drop_shadow` と
        // ビット等価に保つ——既存 box-shadow ゴールデンのピクセル不変を守る。
        let shell_a = f64::from(color[3]) * f64::from(cover);
        if shell_a <= 0.0 {
            continue;
        }
        let shell_color = [color[0], color[1], color[2], shell_a as f32];
        paint(
            x - grow,
            y - grow,
            width + 2.0 * grow,
            height + 2.0 * grow,
            shell_color,
            (corner_radius + grow).max(0.0),
        );
    }
}
