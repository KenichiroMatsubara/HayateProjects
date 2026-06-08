use hayate_core::SceneGraph;
use hayate_scene_renderer_vello::{VelloRenderTarget, VelloSceneRenderer};
use crate::pixel::{CANVAS_H, CANVAS_W, CLEAR_COLOR};

pub struct VelloHarness {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub renderer: VelloSceneRenderer,
}

/// Returns `None` when no wgpu adapter/device is available (caller should skip).
pub fn try_vello_harness() -> Option<VelloHarness> {
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
                label: Some("hayate_css_pixel_test"),
                ..Default::default()
            })
            .await
            .ok()?;
        let renderer = VelloSceneRenderer::new(&device).ok()?;
        Some(VelloHarness {
            device,
            queue,
            renderer,
        })
    })
}

pub fn render_scene_to_pixels(harness: &mut VelloHarness, graph: &SceneGraph) -> Option<Vec<u8>> {
    let texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hayate_css_pixel_test"),
        size: wgpu::Extent3d {
            width: CANVAS_W,
            height: CANVAS_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    harness
        .renderer
        .render_scene(
            graph,
            &VelloRenderTarget {
                device: &harness.device,
                queue: &harness.queue,
                target_view: &view,
                width: CANVAS_W,
                height: CANVAS_H,
            },
            CLEAR_COLOR,
        )
        .ok()?;

    readback_texture_rgba8(&harness.device, &harness.queue, &texture, CANVAS_W, CANVAS_H)
}

fn readback_texture_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bpr = bytes_per_row.div_ceil(align) * align;
    let buffer_size = padded_bpr * height;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hayate_pixel_readback"),
        size: buffer_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("hayate_pixel_readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
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
    rx.recv().ok()?.ok()?;

    let mapped = slice.get_mapped_range();
    let mut out = vec![0u8; (width * height * 4) as usize];
    for row in 0..height {
        let src = (row * padded_bpr) as usize;
        let dst = (row * bytes_per_row) as usize;
        out[dst..dst + bytes_per_row as usize]
            .copy_from_slice(&mapped[src..src + bytes_per_row as usize]);
    }
    drop(mapped);
    buffer.unmap();

    // Vello writes premultiplied RGBA; tests use channel thresholds tolerant of both.
    Some(out)
}
