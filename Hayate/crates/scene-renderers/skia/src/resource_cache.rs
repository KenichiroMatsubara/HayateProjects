use std::collections::HashMap;

use hayate_core::{is_notdef, RenderImage, RenderImageAlphaType, TextFontSlant, TextRunData};
use skia_safe::{
    images, AlphaType, ColorType, Data, Font, Image, ImageInfo, Point, TextBlob, TextBlobBuilder,
};

/// SkTextBlob と CPU-backed SkImage が共有する renderer-local byte budget。
pub const SKIA_PAINT_RESOURCE_CACHE_BUDGET_BYTES: u64 = 32 * 1024 * 1024;
const TEXT_BLOB_BASE_BYTES: u64 = 256;
const TEXT_BLOB_BYTES_PER_GLYPH: u64 = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkiaResourceWorkCounts {
    pub text_blobs_created: u64,
    pub image_byte_copies: u64,
    pub sk_images_created: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ImageKey {
    blob_id: u64,
    width: u32,
    height: u32,
    format: u8,
    alpha_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextBlobKey {
    font_blob_id: u64,
    font_index: u32,
    font_size: u32,
    weight: u32,
    width: u32,
    slant: u8,
    skew_tangent: Option<u32>,
    embolden: Option<u32>,
    normalized_coords: Vec<i16>,
    glyphs: Vec<(u32, u32, u32)>,
}

impl TextBlobKey {
    fn from_run(data: &TextRunData) -> Self {
        let slant = match data.font_attributes.slant {
            TextFontSlant::Upright => 0,
            TextFontSlant::Italic => 1,
            TextFontSlant::Oblique => 2,
        };
        Self {
            font_blob_id: data.font.data.id(),
            font_index: data.font.index,
            font_size: data.font_size.to_bits(),
            weight: data.font_attributes.weight.to_bits(),
            width: data.font_attributes.width.to_bits(),
            slant,
            skew_tangent: data.synthesis.skew_tangent.map(f32::to_bits),
            embolden: data.synthesis.embolden.map(f32::to_bits),
            normalized_coords: data.normalized_coords.clone(),
            glyphs: data
                .glyphs
                .iter()
                .filter(|glyph| !is_notdef(glyph))
                .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                .collect(),
        }
    }
}

impl ImageKey {
    fn from_image(image: &RenderImage) -> Self {
        let format = match image.format {
            hayate_core::RenderImageFormat::Rgba8 => 0,
        };
        let alpha_type = match image.alpha_type {
            RenderImageAlphaType::Opaque => 0,
            RenderImageAlphaType::Alpha => 1,
            RenderImageAlphaType::Premultiplied => 2,
        };
        Self {
            blob_id: image.data.id(),
            width: image.width,
            height: image.height,
            format,
            alpha_type,
        }
    }
}

pub(crate) struct PaintResourceCache {
    images: HashMap<ImageKey, CacheEntry<Image>>,
    text_blobs: HashMap<TextBlobKey, CacheEntry<TextBlob>>,
    work: SkiaResourceWorkCounts,
    budget: u64,
    used: u64,
    clock: u64,
}

struct CacheEntry<T> {
    value: T,
    bytes: u64,
    last_used: u64,
}

impl PaintResourceCache {
    pub(crate) fn new() -> Self {
        Self::with_budget(SKIA_PAINT_RESOURCE_CACHE_BUDGET_BYTES)
    }

    fn with_budget(budget: u64) -> Self {
        Self {
            images: HashMap::new(),
            text_blobs: HashMap::new(),
            work: SkiaResourceWorkCounts::default(),
            budget,
            used: 0,
            clock: 0,
        }
    }

    pub(crate) fn work_counts(&self) -> SkiaResourceWorkCounts {
        self.work
    }

    pub(crate) fn image_for(&mut self, image: &RenderImage) -> Option<Image> {
        if image.width == 0 || image.height == 0 {
            return None;
        }
        let key = ImageKey::from_image(image);
        self.clock = self.clock.wrapping_add(1);
        if let Some(cached) = self.images.get_mut(&key) {
            cached.last_used = self.clock;
            return Some(cached.value.clone());
        }
        let alpha_type = match image.alpha_type {
            RenderImageAlphaType::Opaque => AlphaType::Opaque,
            RenderImageAlphaType::Alpha => AlphaType::Unpremul,
            RenderImageAlphaType::Premultiplied => AlphaType::Premul,
        };
        let info = ImageInfo::new(
            (image.width as i32, image.height as i32),
            ColorType::RGBA8888,
            alpha_type,
            None,
        );
        let row_bytes = info.min_row_bytes();
        let data = Data::new_copy(image.data.data());
        self.work.image_byte_copies += 1;
        let sk_image = images::raster_from_data(&info, data, row_bytes)?;
        self.work.sk_images_created += 1;
        let bytes = image.data.data().len() as u64;
        self.used = self.used.saturating_add(bytes);
        self.images.insert(
            key,
            CacheEntry {
                value: sk_image.clone(),
                bytes,
                last_used: self.clock,
            },
        );
        self.evict_to_budget();
        Some(sk_image)
    }

    pub(crate) fn text_blob_for(&mut self, data: &TextRunData, font: &Font) -> Option<TextBlob> {
        let key = TextBlobKey::from_run(data);
        if key.glyphs.is_empty() {
            return None;
        }
        self.clock = self.clock.wrapping_add(1);
        if let Some(cached) = self.text_blobs.get_mut(&key) {
            cached.last_used = self.clock;
            return Some(cached.value.clone());
        }
        let mut builder = TextBlobBuilder::new();
        let (glyph_ids, positions) = builder.alloc_run_pos(font, key.glyphs.len(), None);
        for (i, &(glyph_id, x, y)) in key.glyphs.iter().enumerate() {
            glyph_ids[i] = glyph_id as u16;
            positions[i] = Point::new(f32::from_bits(x), f32::from_bits(y));
        }
        let blob = builder.make()?;
        self.work.text_blobs_created += 1;
        let bytes = TEXT_BLOB_BASE_BYTES + key.glyphs.len() as u64 * TEXT_BLOB_BYTES_PER_GLYPH;
        self.used = self.used.saturating_add(bytes);
        self.text_blobs.insert(
            key,
            CacheEntry {
                value: blob.clone(),
                bytes,
                last_used: self.clock,
            },
        );
        self.evict_to_budget();
        Some(blob)
    }

    fn evict_to_budget(&mut self) {
        while self.used > self.budget {
            let oldest_image = self
                .images
                .iter()
                .map(|(key, entry)| (*key, entry.last_used, entry.bytes))
                .min_by_key(|(_, last_used, _)| *last_used);
            let oldest_text = self
                .text_blobs
                .iter()
                .map(|(key, entry)| (key.clone(), entry.last_used, entry.bytes))
                .min_by_key(|(_, last_used, _)| *last_used);
            match (oldest_image, oldest_text) {
                (Some((key, image_used, bytes)), Some((text_key, text_used, text_bytes))) => {
                    if image_used <= text_used {
                        self.images.remove(&key);
                        self.used = self.used.saturating_sub(bytes);
                    } else {
                        self.text_blobs.remove(&text_key);
                        self.used = self.used.saturating_sub(text_bytes);
                    }
                }
                (Some((key, _, bytes)), None) => {
                    self.images.remove(&key);
                    self.used = self.used.saturating_sub(bytes);
                }
                (None, Some((key, _, bytes))) => {
                    self.text_blobs.remove(&key);
                    self.used = self.used.saturating_sub(bytes);
                }
                (None, None) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{Blob, RenderImageFormat};

    fn image(byte: u8) -> RenderImage {
        RenderImage {
            width: 1,
            height: 1,
            format: RenderImageFormat::Rgba8,
            alpha_type: RenderImageAlphaType::Alpha,
            data: Blob::from(vec![byte; 4]),
        }
    }

    #[test]
    fn least_recently_used_resources_are_evicted_at_the_byte_budget() {
        let mut cache = PaintResourceCache::with_budget(8);
        let first = image(1);
        let second = image(2);
        let third = image(3);

        cache.image_for(&first).expect("first image");
        cache.image_for(&second).expect("second image");
        cache.image_for(&first).expect("refresh first image");
        cache.image_for(&third).expect("third image evicts second");
        cache
            .image_for(&second)
            .expect("second image must be rebuilt");

        assert_eq!(cache.work_counts().image_byte_copies, 4);
    }
}
