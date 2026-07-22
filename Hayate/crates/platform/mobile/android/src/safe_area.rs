//! 安全領域インセット（edge-to-edge / b2, issue #794・ADR-0144）。**純 Rust シーム**。
//!
//! GameActivity の SurfaceView はフルウィンドウ（edge-to-edge）に置かれ、GPU
//! surface/swapchain もフルウィンドウのまま（`window_dimensions`）。ステータスバー/
//! ナビゲーションバー/ディスプレイカットアウトの安全領域は Kotlin（`MainActivity`）が
//! WindowInsets（systemBars + displayCutout、物理px）として取得し、JNI で Rust へ push する
//! （`jni_bridge::store_pushed_safe_area_insets`）。マージン方式（`setMargins` で ANativeWindow
//! 自体を縮める）は Nothing Phone 3a（Android 15 世代）でリスナーが端末依存で不発になり
//! ステータスバー侵食を起こしたため撤去した（ADR-0144）。
//!
//! ここは push 値の格納庫＋「インセット → レイアウトビューポート / シーン平行移動原点 /
//! タッチ座標補正」の純粋計算。android 非依存なのでホストでコンパイル・テストされ、`app.rs`
//! （android のみ）がフレームループから消費する。`AndroidApp::content_rect()` 由来の
//! `safe_window_dimensions` はフルウィンドウを返す端末があり信頼できないため、push 値が
//! あるときはそちらを優先し、content_rect はフォールバックに降格する。
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Kotlin（`MainActivity`）が JNI で push した最新インセット（物理px）。リスナー発火ごと＋
/// `rootWindowInsets` スナップショットのたびに書かれ、フレームループ（`app.rs`）が読む。
/// `content_rect()` はフルウィンドウを返す端末があり信頼できないため、この push 値がある
/// ときは content_rect フォールバックより優先する（`HAS_PUSHED_INSETS` で有無を表す）。
static PUSHED_LEFT: AtomicI32 = AtomicI32::new(0);
static PUSHED_TOP: AtomicI32 = AtomicI32::new(0);
static PUSHED_RIGHT: AtomicI32 = AtomicI32::new(0);
static PUSHED_BOTTOM: AtomicI32 = AtomicI32::new(0);
static HAS_PUSHED_INSETS: AtomicBool = AtomicBool::new(false);

/// Kotlin から push された安全領域インセットを格納する（Kotlin→Rust JNI の着地点）。
/// JNI seam（`jni_bridge`）の native fn がこれを呼ぶ。スレッド間で読まれるので atomic。
pub fn store_pushed_insets(left: i32, top: i32, right: i32, bottom: i32) {
    PUSHED_LEFT.store(left, Ordering::Relaxed);
    PUSHED_TOP.store(top, Ordering::Relaxed);
    PUSHED_RIGHT.store(right, Ordering::Relaxed);
    PUSHED_BOTTOM.store(bottom, Ordering::Relaxed);
    HAS_PUSHED_INSETS.store(true, Ordering::Release);
}

/// これまでに Kotlin から push された最新インセットを返す。一度も push されていなければ
/// `None`（呼び元は content_rect 由来の `safe_window_dimensions` にフォールバックする）。
pub fn pushed_insets() -> Option<SafeAreaInsets> {
    if !HAS_PUSHED_INSETS.load(Ordering::Acquire) {
        return None;
    }
    Some(SafeAreaInsets {
        left: PUSHED_LEFT.load(Ordering::Relaxed),
        top: PUSHED_TOP.load(Ordering::Relaxed),
        right: PUSHED_RIGHT.load(Ordering::Relaxed),
        bottom: PUSHED_BOTTOM.load(Ordering::Relaxed),
    })
}

/// Kotlin から JNI で push された最新の安全領域インセット（物理px）。systemBars +
/// displayCutout の和（IME は含めない — GameTextInput が別途処理する）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SafeAreaInsets {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SafeAreaInsets {
    /// フルウィンドウの物理サイズと content scale から、レイアウトに渡す論理ビューポートを導く。
    ///
    /// GPU surface はフルウィンドウのままだが、レイアウトが使うビューポートは「ウィンドウ −
    /// インセット」に縮める。これで bottom-anchored 要素がナビゲーションバーの裏に潜らず、
    /// 上端がステータスバーの裏に潜らない。論理px（Web の CSS px 相当）なので content_scale で割る。
    pub fn layout_viewport(
        self,
        window_width: u32,
        window_height: u32,
        content_scale: f32,
    ) -> (f32, f32) {
        let visible_w = (window_width as i32 - self.left.max(0) - self.right.max(0)).max(1) as f32;
        let visible_h = (window_height as i32 - self.top.max(0) - self.bottom.max(0)).max(1) as f32;
        let scale = content_scale.max(1.0);
        (visible_w / scale, visible_h / scale)
    }

    /// 描画時にシーンを安全領域内へ落とし込む平行移動原点（論理px、content_scale 適用前）。
    ///
    /// レイアウトは (0,0) 起点のビューポートでシーンを組むので、フルウィンドウの GPU ターゲット
    /// 上ではそのままだとステータスバー/カットアウトの裏から始まってしまう。左インセット分右へ・
    /// 上インセット分下へずらす（Layer Presentation の placement adapter に渡す）。
    /// レンダラは content_scale を後段で掛けるため、ここは物理インセットを scale で割った論理px。
    pub fn scene_origin(self, content_scale: f32) -> (f32, f32) {
        let scale = content_scale.max(1.0);
        (
            self.left.max(0) as f32 / scale,
            self.top.max(0) as f32 / scale,
        )
    }

    /// ウィンドウ座標（物理px）のタッチ点を、シーンと同じ安全領域原点へ平行移動する。
    ///
    /// GameActivity はタッチを Activity 全体（フルウィンドウ）の座標で native へ流す。描画は
    /// `scene_origin` 分だけ内側へずれているので、タッチも左/上インセット分を差し引いてから
    /// ヒットテスト（`process_touch_input` が続けて content_scale で論理px化する）に渡さないと、
    /// 着弾点が systemBars.top 分だけ下へずれる。Kotlin 側のタッチ補正（旧 `offsetLocation`）を
    /// 撤去し、補正を Rust 側へ一本化する（b2）。
    pub fn correct_touch(self, x_physical: f32, y_physical: f32) -> (f32, f32) {
        (
            x_physical - self.left.max(0) as f32,
            y_physical - self.top.max(0) as f32,
        )
    }

    /// `AndroidApp::content_rect()`（ウィンドウ内の可視領域、物理px）からインセットを導く。
    ///
    /// JNI push（`pushed_insets`）が未着（初回レイアウト前・リスナー不発端末）なときのフォールバック。
    /// content_rect はフルウィンドウを返す端末があり信頼できないため、あくまで push 値が無いときの
    /// 保険で、push 値があればそちらを優先する（ADR-0144）。空/不正な rect（右≤左 等）は全ゼロに丸める。
    pub fn from_content_rect(
        window_width: u32,
        window_height: u32,
        content_left: i32,
        content_top: i32,
        content_right: i32,
        content_bottom: i32,
    ) -> SafeAreaInsets {
        if content_right <= content_left || content_bottom <= content_top {
            return SafeAreaInsets::default();
        }
        SafeAreaInsets {
            left: content_left.max(0),
            top: content_top.max(0),
            right: (window_width as i32 - content_right).max(0),
            bottom: (window_height as i32 - content_bottom).max(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_viewport_shrinks_by_the_system_bar_insets() {
        // Nothing Phone 3a 相当: 1080x2400、ステータスバー上 110px・ナビバー下 132px。
        // レイアウトビューポートはその分縮む（等倍）。
        let insets = SafeAreaInsets {
            left: 0,
            top: 110,
            right: 0,
            bottom: 132,
        };
        assert_eq!(insets.layout_viewport(1080, 2400, 1.0), (1080.0, 2158.0));
    }

    #[test]
    fn scene_origin_shifts_content_below_the_top_inset() {
        // 上 110px・左 40px（横向きカットアウト相当）を安全領域の原点に。等倍なら物理=論理。
        let insets = SafeAreaInsets {
            left: 40,
            top: 110,
            right: 0,
            bottom: 132,
        };
        assert_eq!(insets.scene_origin(1.0), (40.0, 110.0));
    }

    #[test]
    fn scene_origin_is_logical_px_after_dividing_by_content_scale() {
        // 3x 密度: 物理 330px の上インセットは論理 110px（レンダラが後段で 3x 掛け直す）。
        let insets = SafeAreaInsets {
            left: 0,
            top: 330,
            right: 0,
            bottom: 0,
        };
        assert_eq!(insets.scene_origin(3.0), (0.0, 110.0));
    }

    #[test]
    fn correct_touch_subtracts_the_top_left_insets_in_physical_px() {
        // 上 110px 下げた描画に合わせ、ウィンドウ座標 y=200 のタッチは安全領域 y=90 に着弾。
        let insets = SafeAreaInsets {
            left: 40,
            top: 110,
            right: 0,
            bottom: 132,
        };
        assert_eq!(insets.correct_touch(100.0, 200.0), (60.0, 90.0));
    }

    #[test]
    fn pushed_insets_round_trip_and_take_priority_once_set() {
        // push 前は None（呼び元は content_rect フォールバック）。push 後は最新値を返す。
        // 注: グローバル state を触る唯一のテスト（他テストは純粋計算のみ）。
        assert_eq!(pushed_insets(), None);
        store_pushed_insets(0, 110, 0, 132);
        assert_eq!(
            pushed_insets(),
            Some(SafeAreaInsets {
                left: 0,
                top: 110,
                right: 0,
                bottom: 132
            })
        );
        // 後続の push は最新値で上書きする（リスナー発火のたび更新）。
        store_pushed_insets(5, 111, 6, 133);
        assert_eq!(
            pushed_insets(),
            Some(SafeAreaInsets {
                left: 5,
                top: 111,
                right: 6,
                bottom: 133
            })
        );
    }

    #[test]
    fn from_content_rect_derives_insets_as_the_fallback() {
        // ウィンドウ 1080x2400、可視 (0,110)-(1080,2268) → 上 110・下 132・左右 0。
        let insets = SafeAreaInsets::from_content_rect(1080, 2400, 0, 110, 1080, 2268);
        assert_eq!(
            insets,
            SafeAreaInsets {
                left: 0,
                top: 110,
                right: 0,
                bottom: 132
            }
        );
    }

    #[test]
    fn from_content_rect_is_zero_when_the_rect_is_empty_or_full_window() {
        // 空 rect（初回レイアウト前）はゼロインセット＝フルウィンドウ扱い（安全側）。
        assert_eq!(
            SafeAreaInsets::from_content_rect(1080, 2400, 0, 0, 0, 0),
            SafeAreaInsets::default()
        );
        // フルウィンドウを返す端末（content_rect が信頼できない例）もゼロインセット。
        assert_eq!(
            SafeAreaInsets::from_content_rect(1080, 2400, 0, 0, 1080, 2400),
            SafeAreaInsets::default()
        );
    }
}
