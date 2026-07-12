//! 色が淡い不具合（issue #509 の目視検証で発覚）の修正を数値で証明する。
//!
//! 原因: vello は offscreen target（`Rgba8Unorm`・非 sRGB）へ **sRGB エンコード済み**のバイトを
//! 書く。それを blit する宛先 surface が `*Srgb` 形式（Windows の `get_default_config` 既定 =
//! `Rgba8UnormSrgb`）だと、blit の書き込みで linear→sRGB エンコードが**二重**にかかり、色が
//! 淡く（washed out）見える。修正は blit 先 surface を非 sRGB 形式へ揃える 1 行
//! （`surface_config.format.remove_srgb_suffix()`）。
//!
//! 本テストは「vello が offscreen に書いたバイト」（= 画面に出るべき真値。修正は vello 出力を
//! 一切変えない）を既知の値として用意し、**旧経路（sRGB 宛先）と新経路（非 sRGB 宛先）の同一
//! blit** に通して読み戻す。読み戻したバイトはそのまま画面表示値に対応する。
//!
//! - 新経路（非 sRGB）: 読み戻し == 入力（色は忠実に保たれる）。
//! - 旧経路（sRGB）   : 読み戻し != 入力（中間調が明るく＝淡くなる、二重エンコード）。

// vello/wgpu 経路のテスト — `backend-vello`（default on）を外したビルドでは対象外。
#![cfg(feature = "backend-vello")]

use hayate_scene_renderer_vello::create_blitter;

fn try_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all()),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .ok()?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("srgb_blit_proof"),
                ..Default::default()
            })
            .await
            .ok()?;
        Some((device, queue))
    })
}

/// vello の offscreen 出力を模した `Rgba8Unorm` ソーステクスチャを既知バイトで作る。
fn make_source(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[[u8; 4]]) -> wgpu::Texture {
    let width = pixels.len() as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("src_rgba8unorm"),
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let bytes: Vec<u8> = pixels.iter().flatten().copied().collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    texture
}

/// ソースを `dst_format` の宛先へ実際の `TextureBlitter` で blit し、宛先の格納バイトを
/// 読み戻す（格納バイト == 画面に出る値）。
fn blit_and_readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &wgpu::Texture,
    dst_format: wgpu::TextureFormat,
    width: u32,
) -> Vec<u8> {
    let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
    let dst = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dst"),
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: dst_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let dst_view = dst.create_view(&wgpu::TextureViewDescriptor::default());

    // desktop の present 経路と同じ `create_blitter(device, surface_format)` を使う。
    let blitter = create_blitter(device, dst_format);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("blit"),
    });
    blitter.copy(device, &mut encoder, &src_view, &dst_view);
    queue.submit(Some(encoder.finish()));

    // 宛先の格納バイトを読み戻す。
    let bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = bytes_per_row.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: padded as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &dst,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).ok();
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv().unwrap().unwrap();
    let mapped = slice.get_mapped_range();
    let out = mapped[..(bytes_per_row as usize)].to_vec();
    drop(mapped);
    buffer.unmap();
    out
}

#[test]
fn srgb_surface_double_encodes_nonsrgb_preserves() {
    let Some((device, queue)) = try_device() else {
        eprintln!("[skip] wgpu アダプタなし（GPU のある実機で実行すること）");
        return;
    };

    // vello が offscreen に書いた既知バイト = 画面に出るべき真値。
    // 中間調グレー（二重エンコードが顕著）＋ fixture の実色（背景/アクセント/ピンク）。
    let pixels: [[u8; 4]; 4] = [
        [128, 128, 128, 255], // mid gray
        [241, 237, 227, 255], // 背景 #f1ede3
        [20, 184, 166, 255],  // accent teal #14b8a6
        [232, 77, 138, 255],  // pink #e84d8a
    ];
    let names = ["gray", "bg #f1ede3", "teal #14b8a6", "pink #e84d8a"];
    let width = pixels.len() as u32;

    let src = make_source(&device, &queue, &pixels);

    // 新経路（修正後）: 非 sRGB 宛先。
    let fixed = blit_and_readback(
        &device,
        &queue,
        &src,
        wgpu::TextureFormat::Rgba8Unorm,
        width,
    );
    // 旧経路（修正前）: sRGB 宛先 = Windows 既定。
    let buggy = blit_and_readback(
        &device,
        &queue,
        &src,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        width,
    );

    eprintln!("\n  色          入力(=真値)        旧 sRGB宛先(淡化)    新 非sRGB宛先(修正)");
    for (i, name) in names.iter().enumerate() {
        let o = i * 4;
        let inp = &pixels[i][..3];
        let bug = &buggy[o..o + 3];
        let fix = &fixed[o..o + 3];
        eprintln!("  {name:<14} {inp:>3?}   {bug:>3?}   {fix:>3?}");
    }

    // 証明 1: 新経路は入力を忠実に保つ（±1 の量子化誤差）。
    for (i, px) in pixels.iter().enumerate() {
        let o = i * 4;
        for c in 0..3 {
            let diff = (fixed[o + c] as i32 - px[c] as i32).abs();
            assert!(
                diff <= 1,
                "新経路(非sRGB)は色を保つはず: {} ch{c} 入力{} 出力{}",
                names[i],
                px[c],
                fixed[o + c]
            );
        }
    }

    // 証明 2: 旧経路は中間調グレーを明るく（淡く）する＝二重 sRGB エンコード。
    // 128 → ~188 になるはず（理論値: srgb_encode(128/255)*255 ≈ 188）。
    let gray_out = buggy[0];
    assert!(
        gray_out >= 180 && gray_out <= 196,
        "旧経路(sRGB)はグレー128を~188へ淡化するはず、実測 {gray_out}"
    );
    assert!(
        gray_out > pixels[0][0] + 30,
        "旧経路は明確に明るく（淡く）なるはず: 128 -> {gray_out}"
    );
}
