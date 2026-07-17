//! Android の描画バックエンド / AA 方式のランタイム選択（issue #795・ADR-0145）。**純 Rust シーム**。
//!
//! Nothing Phone 3a（Adreno 710）で CSS Gallery ページのパス描画が破綻する切り分けのため、wgpu
//! バックエンド（Vulkan / GL）と vello の AA 方式（Area / MSAA8 / MSAA16）を **再ビルドなし**で
//! 切り替える。同じ端末の Chrome（WebGPU / Dawn）では vello が正常なので、容疑は wgpu-native の
//! Vulkan 経路（Naga 生成 SPIR-V ＋ Dawn 内蔵の Qualcomm 回避策の不在）× Adreno 710 ドライバに
//! 絞り込み済み。恒久対策は実機実験で決めるため、本モジュールはその実験スイッチ。
//!
//! ADR-0138/0140 の「常時コンパイル＋ランタイムフラグ」流儀に従い cargo feature や別ビルドは
//! 作らない。実行時上書きは intent extra（`adb shell am start -e hayate.backend gl -e hayate.aa
//! msaa8`）で、値の取得（JNI で Kotlin から push）は device 専用の薄いグルー、解釈・既定値・
//! グローバル格納はこの純粋シーム。既定値（Area・Vulkan）は名前付き定数（マジック値の禁止）で、
//! 後続の完全人力 issue が実験結果で確定させる。
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use hayate_scene_renderer_vello::{VelloAaMethod, DEFAULT_AA_METHOD};

/// Android の wgpu バックエンド（#795）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgpuBackend {
    Vulkan,
    Gl,
}

/// 既定のバックエンド（現行どおり Vulkan）。名前付き定数。後続の完全人力 issue が確定させる。
pub const DEFAULT_WGPU_BACKEND: WgpuBackend = WgpuBackend::Vulkan;

/// バックエンド上書きの intent extra キー（`adb am start -e hayate.backend gl`）。
pub const BACKEND_INTENT_EXTRA: &str = "hayate.backend";
/// AA 方式上書きの intent extra キー（`adb am start -e hayate.aa msaa8`）。
pub const AA_INTENT_EXTRA: &str = "hayate.aa";

impl WgpuBackend {
    /// intent extra 由来の文字列から解釈する。未知値は `None`（呼び元は既定へフォールバック）。
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "vulkan" => Some(Self::Vulkan),
            "gl" => Some(Self::Gl),
            _ => None,
        }
    }

    /// logcat / 実験記録用の安定名。`from_str_opt` と往復する。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Gl => "gl",
        }
    }

    /// wgpu の `Backends` ビットセットへ写す（`wgpu::Instance` 生成用）。
    pub fn to_wgpu(self) -> wgpu::Backends {
        match self {
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Gl => wgpu::Backends::GL,
        }
    }
}

/// intent extra 由来の文字列（`None` / 未知値 = 未指定）から実効バックエンドを解く。
pub fn resolve_backend(override_str: Option<&str>) -> WgpuBackend {
    override_str
        .and_then(WgpuBackend::from_str_opt)
        .unwrap_or(DEFAULT_WGPU_BACKEND)
}

/// intent extra 由来の文字列（`None` / 未知値 = 未指定）から実効 AA 方式を解く。
pub fn resolve_aa(override_str: Option<&str>) -> VelloAaMethod {
    override_str
        .and_then(VelloAaMethod::from_str_opt)
        .unwrap_or(DEFAULT_AA_METHOD)
}

// ── Kotlin（intent extra）から push された実効設定のグローバル格納 ─────────────
//
// MainActivity.onCreate が intent extra を読み、空文字（未指定）も含めて JNI で push する。
// 解決済みの enum を u8 コードで atomic に持ち、`init_gpu_surface`（app.rs）が読む。

static PUSHED_BACKEND: AtomicU8 = AtomicU8::new(0);
static PUSHED_AA: AtomicU8 = AtomicU8::new(0);
static HAS_PUSHED_CONFIG: AtomicBool = AtomicBool::new(false);

fn backend_code(b: WgpuBackend) -> u8 {
    match b {
        WgpuBackend::Vulkan => 0,
        WgpuBackend::Gl => 1,
    }
}

fn backend_from_code(c: u8) -> WgpuBackend {
    match c {
        1 => WgpuBackend::Gl,
        _ => WgpuBackend::Vulkan,
    }
}

fn aa_code(a: VelloAaMethod) -> u8 {
    match a {
        VelloAaMethod::Area => 0,
        VelloAaMethod::Msaa8 => 1,
        VelloAaMethod::Msaa16 => 2,
    }
}

fn aa_from_code(c: u8) -> VelloAaMethod {
    match c {
        1 => VelloAaMethod::Msaa8,
        2 => VelloAaMethod::Msaa16,
        _ => VelloAaMethod::Area,
    }
}

/// Kotlin から push された intent extra 文字列（空文字＝未指定）を解決して格納する
/// （Kotlin→Rust JNI の着地点。`jni_bridge` の native fn が呼ぶ）。
pub fn store_pushed_config(backend_override: &str, aa_override: &str) {
    let backend = resolve_backend(Some(backend_override));
    let aa = resolve_aa(Some(aa_override));
    PUSHED_BACKEND.store(backend_code(backend), Ordering::Relaxed);
    PUSHED_AA.store(aa_code(aa), Ordering::Relaxed);
    HAS_PUSHED_CONFIG.store(true, Ordering::Release);
}

/// push 済みの実効バックエンド（未 push なら既定）。
pub fn effective_backend() -> WgpuBackend {
    if HAS_PUSHED_CONFIG.load(Ordering::Acquire) {
        backend_from_code(PUSHED_BACKEND.load(Ordering::Relaxed))
    } else {
        DEFAULT_WGPU_BACKEND
    }
}

/// push 済みの実効 AA 方式（未 push なら既定）。
pub fn effective_aa() -> VelloAaMethod {
    if HAS_PUSHED_CONFIG.load(Ordering::Acquire) {
        aa_from_code(PUSHED_AA.load(Ordering::Relaxed))
    } else {
        DEFAULT_AA_METHOD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_vulkan_and_area() {
        // 既定値は現行どおり（名前付き定数）。web/iOS は本モジュールを使わない。
        assert_eq!(DEFAULT_WGPU_BACKEND, WgpuBackend::Vulkan);
        assert_eq!(DEFAULT_AA_METHOD, VelloAaMethod::Area);
    }

    #[test]
    fn backend_parses_and_round_trips() {
        assert_eq!(
            WgpuBackend::from_str_opt("vulkan"),
            Some(WgpuBackend::Vulkan)
        );
        assert_eq!(WgpuBackend::from_str_opt("gl"), Some(WgpuBackend::Gl));
        assert_eq!(WgpuBackend::from_str_opt("metal"), None);
        for b in [WgpuBackend::Vulkan, WgpuBackend::Gl] {
            assert_eq!(WgpuBackend::from_str_opt(b.as_str()), Some(b));
        }
    }

    #[test]
    fn backend_maps_to_wgpu_bitset() {
        assert_eq!(WgpuBackend::Vulkan.to_wgpu(), wgpu::Backends::VULKAN);
        assert_eq!(WgpuBackend::Gl.to_wgpu(), wgpu::Backends::GL);
    }

    #[test]
    fn resolve_falls_back_to_defaults_on_missing_or_unknown() {
        // 未指定（空文字/None）・未知値は既定へ。3 実験だけが既定を離れる。
        assert_eq!(resolve_backend(None), WgpuBackend::Vulkan);
        assert_eq!(resolve_backend(Some("")), WgpuBackend::Vulkan);
        assert_eq!(resolve_backend(Some("gl")), WgpuBackend::Gl);
        assert_eq!(resolve_aa(None), VelloAaMethod::Area);
        assert_eq!(resolve_aa(Some("bogus")), VelloAaMethod::Area);
        assert_eq!(resolve_aa(Some("msaa16")), VelloAaMethod::Msaa16);
    }

    #[test]
    fn pushed_config_round_trips_through_the_global() {
        // 注: グローバル state を触る唯一のテスト。空文字（未指定）は既定へ落ちる。
        assert_eq!(effective_backend(), WgpuBackend::Vulkan);
        assert_eq!(effective_aa(), VelloAaMethod::Area);
        store_pushed_config("gl", "msaa8");
        assert_eq!(effective_backend(), WgpuBackend::Gl);
        assert_eq!(effective_aa(), VelloAaMethod::Msaa8);
        store_pushed_config("", "");
        assert_eq!(effective_backend(), WgpuBackend::Vulkan);
        assert_eq!(effective_aa(), VelloAaMethod::Area);
    }
}
