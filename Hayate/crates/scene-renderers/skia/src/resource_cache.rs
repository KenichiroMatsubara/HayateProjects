use std::collections::HashMap;

use hayate_core::{is_notdef, RenderImage, RenderImageAlphaType, TextRun, TextRunId};
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
    text_blobs: HashMap<TextRunId, CacheEntry<TextBlob>>,
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

    pub(crate) fn text_blob_for(
        &mut self,
        text_run: TextRunId,
        data: &TextRun,
        font: &Font,
    ) -> Option<TextBlob> {
        self.clock = self.clock.wrapping_add(1);
        if let Some(cached) = self.text_blobs.get_mut(&text_run) {
            cached.last_used = self.clock;
            return Some(cached.value.clone());
        }
        let glyph_count = data.glyphs.iter().filter(|glyph| !is_notdef(glyph)).count();
        if glyph_count == 0 {
            return None;
        }
        let mut builder = TextBlobBuilder::new();
        let (glyph_ids, positions) = builder.alloc_run_pos(font, glyph_count, None);
        for (i, glyph) in data
            .glyphs
            .iter()
            .filter(|glyph| !is_notdef(glyph))
            .enumerate()
        {
            glyph_ids[i] = glyph.id as u16;
            positions[i] = Point::new(glyph.x, glyph.y);
        }
        let blob = builder.make()?;
        self.work.text_blobs_created += 1;
        let bytes = TEXT_BLOB_BASE_BYTES + glyph_count as u64 * TEXT_BLOB_BYTES_PER_GLYPH;
        self.used = self.used.saturating_add(bytes);
        self.text_blobs.insert(
            text_run,
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
                .map(|(key, entry)| (*key, entry.last_used, entry.bytes))
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
