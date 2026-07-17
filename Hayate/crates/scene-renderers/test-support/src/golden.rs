use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::Path;

use png::{BitDepth, ColorType, Decoder, Encoder, Transformations};

fn encode_png(path: &Path, pixels: &[u8], width: u32, height: u32) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create golden directory");
    }
    let file = File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut writer = BufWriter::new(file);
    let mut encoder = Encoder::new(&mut writer, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut png_writer = encoder
        .write_header()
        .unwrap_or_else(|e| panic!("png header {}: {e}", path.display()));
    png_writer
        .write_image_data(pixels)
        .unwrap_or_else(|e| panic!("png pixels {}: {e}", path.display()));
}

fn decode_png(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("png info");
    let mut buf = vec![0; reader.output_buffer_size().expect("png output size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    let width = info.width;
    let height = info.height;
    buf.truncate(info.buffer_size());
    (buf, width, height)
}

/// レンダリング結果を、コミット済みの PNG ゴールデンと比較する。
///
/// `HAYATE_UPDATE_GOLDEN=1` を設定すると、人手レビュー後にベースラインを再生成する。
pub fn assert_pixels_match_golden(golden_path: &Path, pixels: &[u8], width: u32, height: u32) {
    if std::env::var_os("HAYATE_UPDATE_GOLDEN").is_some() {
        encode_png(golden_path, pixels, width, height);
        return;
    }

    let bytes = std::fs::read(golden_path).unwrap_or_else(|e| {
        panic!(
            "missing golden {} ({e}); run with HAYATE_UPDATE_GOLDEN=1 to generate",
            golden_path.display()
        )
    });
    let (expected, exp_w, exp_h) = decode_png(&bytes);
    assert_eq!(
        (exp_w, exp_h),
        (width, height),
        "golden size mismatch for {}",
        golden_path.display()
    );
    assert_eq!(expected, pixels, "pixel diff for {}", golden_path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_png_buffer() {
        let width = 2;
        let height = 2;
        let pixels = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let mut buf = Vec::new();
        let mut encoder = Encoder::new(&mut buf, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let (decoded, w, h) = decode_png(&buf);
        assert_eq!((w, h), (width, height));
        assert_eq!(decoded, pixels);
    }
}
