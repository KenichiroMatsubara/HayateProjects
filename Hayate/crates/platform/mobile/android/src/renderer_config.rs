//! Android のレンダラ強制指定（intent extra）の純 Rust シーム（issue #802、spec §4 REND-15、
//! ADR-0146/0147）。
//!
//! desktop の env/CLI 強制指定（`hayate-platform-desktop::renderer_config`）に対応する
//! Android の口——`adb shell am start -e hayate.renderer skia` で APK を作り直さずに
//! vello / skia を切り替える（#795 の `hayate.backend` / `hayate.aa` と同じ操作感）。値の
//! 解釈・既定値（未指定/未知値は selection policy の既定へ委ねる）はここに置く純関数で、
//! JNI push（Kotlin→Rust）の着地点だけが device 専用（`jni_bridge.rs`）。
//!
//! `hayate_app_host::renderer_selection::native_renderer_selection_policy` が skia→vello の
//! 一方向 fallback・forced-override の却下・vello 不在時の skia 単独起動をすでに決めている
//! （issue #801）。本モジュールは policy を再導出せず、その入力（`forced` / `vello_linked`）を
//! 用意するだけ。

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use hayate_app_host::renderer_selection::SceneRendererKind;

/// レンダラ強制指定の intent extra キー（`adb shell am start -e hayate.renderer skia`）。
pub const RENDERER_INTENT_EXTRA: &str = "hayate.renderer";

/// 強制指定の値語彙。`SceneRendererKind::name()` と同一の安定 ID（desktop の
/// `RENDERER_VALUE_VELLO` / `RENDERER_VALUE_SKIA` と揃える）。
pub const RENDERER_VALUE_VELLO: &str = "vello";
pub const RENDERER_VALUE_SKIA: &str = "skia";

/// このビルドが vello/wgpu をリンクしているか。Android は desktop の `backend-vello`
/// feature に相当する分離をまだ導入していないため常時リンク（将来課題）。
pub const VELLO_LINKED: bool = true;

/// intent extra 由来の文字列から強制指定レンダラを解釈する。Android で選べるのは
/// vello / skia のみで、未知値は `None`（= 既定の selection policy へ委ねる）。
pub fn parse_renderer_name(value: &str) -> Option<SceneRendererKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        RENDERER_VALUE_VELLO => Some(SceneRendererKind::Vello),
        RENDERER_VALUE_SKIA => Some(SceneRendererKind::Skia),
        _ => None,
    }
}

// ── skia 内 surface（raster / GL）の切替（issue #803・ADR-0146 §3） ──────────────────
//
// `hayate.renderer`（vello/skia）とは独立の直交軸——skia が選ばれたとき、その提示面を
// CPU raster（`skia_window.rs`）と Ganesh GL/EGL（`skia_gl_window.rs`）のどちらにするか。
// #795 の `hayate.backend`（wgpu Vulkan/GL）と同じ操作感・同じ resolve 流儀。

/// skia 内 surface 切替の intent extra キー（`adb shell am start -e hayate.skia_surface gl`）。
pub const SKIA_SURFACE_INTENT_EXTRA: &str = "hayate.skia_surface";

/// 切替の値語彙（名前付き定数、`SkiaSurfaceKind::as_str` と往復する）。
pub const SKIA_SURFACE_VALUE_RASTER: &str = "raster";
pub const SKIA_SURFACE_VALUE_GL: &str = "gl";

/// skia Scene Renderer の提示面種別（issue #803）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkiaSurfaceKind {
    /// CPU raster + `ANativeWindow_lock`/`unlockAndPost`（#802、`skia_window.rs`）。
    Raster,
    /// Ganesh GL（EGL window surface + `eglSwapBuffers`、`skia_gl_window.rs`）。
    Gl,
}

/// 既定の skia surface（名前付き定数）。GL は HWUI/Chrome が長年叩いたドライバ成熟経路であり、
/// issue #804 で OPPO A101OP / Nothing Phone (3a) の実アプリ描画が正常だったため既定に据える
/// （ADR-0149）。EGL 初期化に失敗する端末は skia raster へ自動で落ちるため boot は死なない。
pub const DEFAULT_SKIA_SURFACE: SkiaSurfaceKind = SkiaSurfaceKind::Gl;

impl SkiaSurfaceKind {
    /// intent extra 由来の文字列から解釈する。未知値は `None`（呼び元は既定へフォールバック）。
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            SKIA_SURFACE_VALUE_RASTER => Some(Self::Raster),
            SKIA_SURFACE_VALUE_GL => Some(Self::Gl),
            _ => None,
        }
    }

    /// logcat / 実験記録用の安定名。`from_str_opt` と往復する。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raster => SKIA_SURFACE_VALUE_RASTER,
            Self::Gl => SKIA_SURFACE_VALUE_GL,
        }
    }
}

/// intent extra 由来の文字列（`None` / 未知値 = 未指定）から実効 skia surface を解く。
pub fn resolve_skia_surface(override_str: Option<&str>) -> SkiaSurfaceKind {
    override_str
        .and_then(SkiaSurfaceKind::from_str_opt)
        .unwrap_or(DEFAULT_SKIA_SURFACE)
}

// ── Kotlin（intent extra）から push された強制指定のグローバル格納 ─────────────────
//
// MainActivity.onCreate が intent extra を読み、空文字（未指定）も含めて JNI で push する。
// CreateSurface（GPU surface 初期化）より前に必ず 1 度 push される（#795 の
// render_config::store_pushed_config と同じ着地パターン）。

static PUSHED_RENDERER: AtomicU8 = AtomicU8::new(0);
static HAS_PUSHED_RENDERER: AtomicBool = AtomicBool::new(false);

fn renderer_code(kind: SceneRendererKind) -> u8 {
    match kind {
        SceneRendererKind::Skia => 1,
        _ => 0, // Vello（既定コード）。他 kind はネイティブの強制指定候補ではない。
    }
}

fn renderer_from_code(code: u8) -> SceneRendererKind {
    match code {
        1 => SceneRendererKind::Skia,
        _ => SceneRendererKind::Vello,
    }
}

/// Kotlin から push された intent extra 文字列（空文字/未知値＝未指定）を解決して格納する
/// （Kotlin→Rust JNI の着地点。`jni_bridge` の native fn が呼ぶ）。
pub fn store_pushed_renderer(value: &str) {
    match parse_renderer_name(value) {
        Some(kind) => {
            PUSHED_RENDERER.store(renderer_code(kind), Ordering::Relaxed);
            HAS_PUSHED_RENDERER.store(true, Ordering::Release);
        }
        None => HAS_PUSHED_RENDERER.store(false, Ordering::Release),
    }
}

/// push 済みの強制指定レンダラ（未 push / 未指定 / 未知値なら `None` = 既定 policy に委ねる）。
/// `native_renderer_selection_policy` の `forced` 引数へそのまま渡す。
pub fn forced_renderer() -> Option<SceneRendererKind> {
    if HAS_PUSHED_RENDERER.load(Ordering::Acquire) {
        Some(renderer_from_code(PUSHED_RENDERER.load(Ordering::Relaxed)))
    } else {
        None
    }
}

// skia surface（raster/GL）も同じ着地パターン（issue #803）。解決済み enum を u8 コードで持つ。
static PUSHED_SKIA_SURFACE: AtomicU8 = AtomicU8::new(0);
static HAS_PUSHED_SKIA_SURFACE: AtomicBool = AtomicBool::new(false);

fn skia_surface_code(kind: SkiaSurfaceKind) -> u8 {
    match kind {
        SkiaSurfaceKind::Raster => 0,
        SkiaSurfaceKind::Gl => 1,
    }
}

fn skia_surface_from_code(code: u8) -> SkiaSurfaceKind {
    match code {
        1 => SkiaSurfaceKind::Gl,
        _ => SkiaSurfaceKind::Raster,
    }
}

/// Kotlin から push された intent extra 文字列（空文字/未知値＝未指定）を解決して格納する
/// （Kotlin→Rust JNI の着地点。`jni_bridge` の native fn が呼ぶ）。
pub fn store_pushed_skia_surface(value: &str) {
    match SkiaSurfaceKind::from_str_opt(value) {
        Some(kind) => {
            PUSHED_SKIA_SURFACE.store(skia_surface_code(kind), Ordering::Relaxed);
            HAS_PUSHED_SKIA_SURFACE.store(true, Ordering::Release);
        }
        None => HAS_PUSHED_SKIA_SURFACE.store(false, Ordering::Release),
    }
}

/// push 済みの実効 skia surface（未 push / 未指定 / 未知値なら既定 = `DEFAULT_SKIA_SURFACE`）。
/// `init_and_spawn_raster` の skia 分岐が読む。
pub fn effective_skia_surface() -> SkiaSurfaceKind {
    if HAS_PUSHED_SKIA_SURFACE.load(Ordering::Acquire) {
        skia_surface_from_code(PUSHED_SKIA_SURFACE.load(Ordering::Relaxed))
    } else {
        DEFAULT_SKIA_SURFACE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_key_and_default_values_are_named_constants() {
        // issue #802 受け入れ条件: 切替キー名・値語彙が名前付き定数であること。
        assert_eq!(RENDERER_INTENT_EXTRA, "hayate.renderer");
        assert_eq!(RENDERER_VALUE_VELLO, SceneRendererKind::Vello.name());
        assert_eq!(RENDERER_VALUE_SKIA, SceneRendererKind::Skia.name());
    }

    #[test]
    fn parses_known_values_and_round_trips() {
        assert_eq!(parse_renderer_name("vello"), Some(SceneRendererKind::Vello));
        assert_eq!(parse_renderer_name("skia"), Some(SceneRendererKind::Skia));
        for kind in [SceneRendererKind::Vello, SceneRendererKind::Skia] {
            assert_eq!(parse_renderer_name(kind.name()), Some(kind));
        }
    }

    #[test]
    fn unknown_or_empty_values_fall_to_the_default_policy() {
        assert_eq!(parse_renderer_name(""), None);
        assert_eq!(parse_renderer_name("dawn"), None);
        assert_eq!(parse_renderer_name("tiny-skia"), None);
    }

    #[test]
    fn values_are_trimmed_and_case_insensitive() {
        assert_eq!(parse_renderer_name(" Skia "), Some(SceneRendererKind::Skia));
        assert_eq!(parse_renderer_name("VELLO"), Some(SceneRendererKind::Vello));
    }

    #[test]
    fn skia_surface_switch_key_values_and_default_are_named_constants() {
        // issue #803 受け入れ条件: skia 内 raster/GL 切替キー・値語彙・既定値が名前付き定数で
        // あること（確定既定値は issue #804 / ADR-0149）。
        assert_eq!(SKIA_SURFACE_INTENT_EXTRA, "hayate.skia_surface");
        assert_eq!(SKIA_SURFACE_VALUE_RASTER, SkiaSurfaceKind::Raster.as_str());
        assert_eq!(SKIA_SURFACE_VALUE_GL, SkiaSurfaceKind::Gl.as_str());
        assert_eq!(DEFAULT_SKIA_SURFACE, SkiaSurfaceKind::Gl);
    }

    #[test]
    fn skia_surface_parses_known_values_and_falls_to_the_default_otherwise() {
        assert_eq!(SkiaSurfaceKind::from_str_opt("raster"), Some(SkiaSurfaceKind::Raster));
        assert_eq!(SkiaSurfaceKind::from_str_opt("gl"), Some(SkiaSurfaceKind::Gl));
        assert_eq!(SkiaSurfaceKind::from_str_opt(" GL "), Some(SkiaSurfaceKind::Gl));
        assert_eq!(SkiaSurfaceKind::from_str_opt(""), None);
        assert_eq!(SkiaSurfaceKind::from_str_opt("vulkan"), None);
        // 未指定/未知値は既定（名前付き定数）へ（#795 の resolve_backend と同じ流儀）。
        assert_eq!(resolve_skia_surface(None), DEFAULT_SKIA_SURFACE);
        assert_eq!(resolve_skia_surface(Some("bogus")), DEFAULT_SKIA_SURFACE);
        assert_eq!(resolve_skia_surface(Some("raster")), SkiaSurfaceKind::Raster);
    }

    #[test]
    fn pushed_skia_surface_round_trips_through_the_global() {
        // 注: グローバル state を触るテスト（`pushed_renderer_...` と同じ流儀）。未 push・
        // 空文字/未知値は既定（DEFAULT_SKIA_SURFACE）へ落ちる。
        assert_eq!(effective_skia_surface(), DEFAULT_SKIA_SURFACE);
        store_pushed_skia_surface("raster");
        assert_eq!(effective_skia_surface(), SkiaSurfaceKind::Raster);
        store_pushed_skia_surface("gl");
        assert_eq!(effective_skia_surface(), SkiaSurfaceKind::Gl);
        store_pushed_skia_surface("");
        assert_eq!(effective_skia_surface(), DEFAULT_SKIA_SURFACE);
    }

    #[test]
    fn pushed_renderer_round_trips_through_the_global() {
        // 注: グローバル state を触る唯一のテスト（`render_config` の同種テストと同じ流儀）。
        assert_eq!(forced_renderer(), None);
        store_pushed_renderer("skia");
        assert_eq!(forced_renderer(), Some(SceneRendererKind::Skia));
        store_pushed_renderer("vello");
        assert_eq!(forced_renderer(), Some(SceneRendererKind::Vello));
        // 空文字/未知値の再 push は「未指定」へ戻す（#795 の resolve と同じ流儀）。
        store_pushed_renderer("");
        assert_eq!(forced_renderer(), None);
    }
}
