//! skia raster フレームの ANativeWindow への CPU present 用ピクセル変換
//! （issue #802・ADR-0146 §3）。
//!
//! desktop の `skia_present.rs`（softbuffer 0RGB 変換）と同型の retained layer 経路。ANativeWindow
//! は RGBX_8888（4byte/px・アルファチャンネルは無視されるが書く必要はある）を present 形式に
//! 使う——clear color が不透明（`STAGE_A_CLEAR_COLOR` は alpha 1.0）なので合成結果も実質不透明で
//! あり、alpha は固定 0xff で書けばよい（un-premultiply 不要、desktop の `copy_rgba_to_xrgb` と
//! 同じ理由）。ここは `ndk::native_window` に一切触れない純関数だけを置き、ホストでテストする。
//! ANativeWindow への実書き込み（lock/lines/unlockAndPost）は device 専用の `skia_window.rs`。

/// RGBA8888（premultiplied、skia raster surface の読み戻し形式）を ANativeWindow の
/// RGBX_8888（4byte/px、tightly packed、stride 非依存）へ変換して `out` に書く。
pub fn copy_rgba_to_rgbx(rgba: &[u8], out: &mut [u8]) {
    debug_assert_eq!(rgba.len(), out.len());
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = 0xff;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_bytes_map_to_rgbx_with_alpha_forced_opaque() {
        let rgba = [0x12, 0x34, 0x56, 0x80, 0xff, 0x00, 0x00, 0x00];
        let mut out = [0u8; 8];
        copy_rgba_to_rgbx(&rgba, &mut out);
        assert_eq!(out, [0x12, 0x34, 0x56, 0xff, 0xff, 0x00, 0x00, 0xff]);
    }
}
