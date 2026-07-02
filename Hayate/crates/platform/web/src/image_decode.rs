//! Canvas Mode の画像デコード共通経路（#643）。
//!
//! ブラウザの `createImageBitmap`（オフスレッドデコード）を主経路とし、非対応・失敗時は
//! `image` クレートの WASM 内 CPU 同期デコードへフォールバックする（`canvas.rs::fetch_image`）。
//! どちらの経路もストレート αRGBA8 の中間表現 [`DecodedRgba`] に落ち、単一の
//! [`render_image_from_rgba`] で `RenderImage` を組む。これで「どちらのデコーダでも RenderImage の
//! 組み立て（フォーマット・アルファ扱い・寸法・Blob 化）は一致する」を 1 seam で保証する。

use hayate_core::{RenderImage, RenderImageAlphaType, RenderImageFormat};

/// デコード済みのストレート αRGBA8 ピクセルと寸法。`createImageBitmap` 経路（canvas の
/// `getImageData` 読み戻し）と `image` クレート経路の共通中間表現。
pub(crate) struct DecodedRgba {
    /// 行優先・4 バイト/画素（R,G,B,A）のストレート α ピクセル。長さは `width * height * 4`。
    pub raw: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// `image` クレート（WASM 内 CPU デコード）で PNG/JPEG/WebP をストレート αRGBA8 に復号する。
///
/// `createImageBitmap` が使えない・失敗したときのフォールバックであり、ブラウザを介さず
/// ホストで直接テストできる参照経路でもある。`into_rgba8()` はストレート α を返すので、
/// `getImageData`（同じくストレート α）と同じアルファ扱いになる。
pub(crate) fn decode_image_bytes(bytes: &[u8]) -> Result<DecodedRgba, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Ok(DecodedRgba {
        raw: rgba.into_raw(),
        width,
        height,
    })
}

/// ストレート αRGBA8 ピクセルから `RenderImage` を組む（#643）。両デコード経路の唯一の合流点。
///
/// Blob 化はここで一度だけ行う。以後この画像が生きている間は Blob id が安定し、vello の
/// 画像アトラスに毎フレームヒットする（issue #630）。
pub(crate) fn render_image_from_rgba(decoded: DecodedRgba) -> RenderImage {
    RenderImage {
        data: hayate_core::Blob::from(decoded.raw),
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        width: decoded.width,
        height: decoded.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};

    /// 既知の 2×2 RGBA 画像（半透明画素を含む）を PNG にエンコードして返す。
    fn sample_png() -> (Vec<u8>, Vec<u8>) {
        // R,G,B,A を明示。半透明画素（alpha=128）を含め、ストレート α が保存されることを確かめる。
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, // 不透明赤
            0, 255, 0, 255, // 不透明緑
            0, 0, 255, 128, // 半透明青
            10, 20, 30, 40, // 任意の半透明色
        ];
        let img = RgbaImage::from_raw(2, 2, pixels.clone()).expect("2x2 image");
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, ImageFormat::Png).expect("encode png");
        (bytes.into_inner(), pixels)
    }

    #[test]
    fn decode_png_preserves_dimensions_and_straight_alpha_pixels() {
        let (png, expected) = sample_png();
        let decoded = decode_image_bytes(&png).expect("decode");
        assert_eq!((decoded.width, decoded.height), (2, 2), "dimensions must match");
        // PNG はロスレスなので、ストレート α のピクセルがそのまま戻る（半透明も premultiply されない）。
        assert_eq!(decoded.raw, expected, "RGBA pixels must round-trip unchanged");
    }

    #[test]
    fn render_image_from_rgba_builds_rgba8_straight_alpha_contract() {
        let decoded = DecodedRgba {
            raw: vec![1, 2, 3, 4, 5, 6, 7, 8],
            width: 2,
            height: 1,
        };
        let expected_raw = decoded.raw.clone();
        let image = render_image_from_rgba(decoded);
        // どちらのデコード経路が作った DecodedRgba でも、この構築規約は同一（parity の核）。
        assert_eq!(image.format, RenderImageFormat::Rgba8);
        assert_eq!(image.alpha_type, RenderImageAlphaType::Alpha);
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.data.data(), expected_raw.as_slice(), "pixels must transfer verbatim");
    }
}
