//! skia raster フレームの CPU present 用ピクセル変換（issue #801・ADR-0146 §3）。
//!
//! desktop の skia 経路は wgpu 非依存 — retained layer surface（RGBA8888 premul）を
//! softbuffer の 0RGB（`0x00RRGGBB`）へ変換して winit window に software blit する。ここは window
//! にも softbuffer にも触れない純関数だけを置き、headless（CI）でそのままテストする。

/// RGBA8888（premultiplied、skia raster surface の読み戻し形式）を softbuffer の
/// 0RGB（上位 8bit 未使用、`R<<16 | G<<8 | B`）へ変換して `out` に書く。
///
/// clear color が不透明（desktop の `CLEAR_COLOR` は alpha 1.0）なので合成結果も実質
/// 不透明であり、alpha は落とすだけでよい（un-premultiply 不要）。
pub fn copy_rgba_to_xrgb(rgba: &[u8], out: &mut [u32]) {
    debug_assert_eq!(rgba.len(), out.len() * 4);
    for (px, chunk) in out.iter_mut().zip(rgba.chunks_exact(4)) {
        let [r, g, b, _a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        *px = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_bytes_map_to_softbuffer_0rgb_words() {
        // softbuffer の契約: ピクセルは 32bit、上位 8bit 未使用、R<<16 | G<<8 | B。
        let rgba = [0x12, 0x34, 0x56, 0xff, 0xff, 0x00, 0x00, 0xff];
        let mut out = [0u32; 2];
        copy_rgba_to_xrgb(&rgba, &mut out);
        assert_eq!(out, [0x0012_3456, 0x00ff_0000]);
    }

    #[test]
    fn opaque_alpha_is_dropped_not_shifted() {
        let rgba = [0x00, 0x00, 0x00, 0xff];
        let mut out = [0u32; 1];
        copy_rgba_to_xrgb(&rgba, &mut out);
        assert_eq!(
            out,
            [0x0000_0000],
            "alpha must not leak into the unused high byte"
        );
    }
}
