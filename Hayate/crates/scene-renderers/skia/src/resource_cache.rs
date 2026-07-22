use hayate_core::{
    is_notdef, FontInstance, FontInstanceId, RenderImage, RenderImageAlphaType, TextRun, TextRunId,
};
use hayate_layer_compositor::{
    ImageResourceId, PoolBudgetPolicy, RenderResourceBudgetPolicy, RenderResourceKey,
    RenderResourceResidency, ResidencyEvent, ResidencyStats, ResourceDomain,
};
use skia_safe::{
    font_arguments::{variation_position::Coordinate, FontArguments, VariationPosition},
    images, AlphaType, ColorType, Data, Font, FontMgr, FourByteTag, Image, ImageInfo, Point,
    TextBlob, TextBlobBuilder, Typeface,
};
use skrifa::{
    raw::{tables::avar::SegmentMaps, FontRef, TableProvider},
    MetadataProvider,
};

/// SkTypeface, SkTextBlob and CPU-backed SkImage share one renderer-scoped CPU pool.
pub const SKIA_PAINT_RESOURCE_CACHE_BUDGET_BYTES: u64 = 32 * 1024 * 1024;
const TEXT_BLOB_BASE_BYTES: u64 = 256;
const TEXT_BLOB_BYTES_PER_GLYPH: u64 = 16;
const TYPEFACE_BASE_BYTES: u64 = 1024;
const RESOURCE_REBUILD_COST_PER_BYTE: u64 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkiaResourceWorkCounts {
    pub typefaces_created: u64,
    pub text_blobs_created: u64,
    pub image_byte_copies: u64,
    pub sk_images_created: u64,
}

enum SkiaPaintResource {
    Typeface(Option<Typeface>),
    TextBlob(TextBlob),
    Image(Image),
}

pub(crate) struct PaintResourceCache {
    residency: RenderResourceResidency<SkiaPaintResource>,
    work: SkiaResourceWorkCounts,
}

impl PaintResourceCache {
    pub(crate) fn new() -> Self {
        Self::with_budget(SKIA_PAINT_RESOURCE_CACHE_BUDGET_BYTES)
    }

    fn with_budget(budget: u64) -> Self {
        Self {
            residency: RenderResourceResidency::new(RenderResourceBudgetPolicy {
                cpu: PoolBudgetPolicy::fixed(budget),
                gpu: PoolBudgetPolicy::fixed(0),
            }),
            work: SkiaResourceWorkCounts::default(),
        }
    }

    pub(crate) fn configure(&mut self, policy: RenderResourceBudgetPolicy) {
        self.residency.set_policy(policy);
    }

    pub(crate) fn handle_lifecycle(&mut self, event: ResidencyEvent) {
        self.residency.handle_lifecycle(event);
    }

    pub(crate) fn stats(&self) -> ResidencyStats {
        self.residency.stats()
    }

    pub(crate) fn work_counts(&self) -> SkiaResourceWorkCounts {
        self.work
    }

    pub(crate) fn image_for(&mut self, image: &RenderImage) -> Option<Image> {
        if image.width == 0 || image.height == 0 {
            return None;
        }
        let key = RenderResourceKey::Image(image_resource_id(image));
        if let Some(SkiaPaintResource::Image(cached)) = self.residency.get(ResourceDomain::Cpu, key)
        {
            return Some(cached.clone());
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
        self.residency.insert(
            ResourceDomain::Cpu,
            key,
            SkiaPaintResource::Image(sk_image.clone()),
            bytes,
            bytes.saturating_mul(RESOURCE_REBUILD_COST_PER_BYTE),
        );
        Some(sk_image)
    }

    pub(crate) fn typeface_for(
        &mut self,
        font_id: FontInstanceId,
        instance: &FontInstance,
    ) -> Option<Typeface> {
        let key = RenderResourceKey::Font(font_id);
        if let Some(SkiaPaintResource::Typeface(cached)) =
            self.residency.get(ResourceDomain::Cpu, key)
        {
            return cached.clone();
        }
        let font = &instance.font;
        let bytes: &[u8] = font.data.as_ref();
        // FontMgr itself is not Send in skia-safe. Construct it only on the renderer's raster
        // thread when a new FontInstanceId misses; the resulting Typeface is Send and lives in
        // the renderer-scoped residency. Steady-state lookup never reconstructs FontMgr.
        let typeface = FontMgr::default()
            .new_from_data(bytes, font.index as usize)
            .and_then(|base| {
                if instance.normalized_coords.is_empty() {
                    return Some(base);
                }
                let coords = design_coords_from_normalized(font, &instance.normalized_coords);
                if coords.is_empty() {
                    return Some(base);
                }
                let args = FontArguments::new().set_variation_design_position(VariationPosition {
                    coordinates: &coords,
                });
                base.clone_with_arguments(&args).or(Some(base))
            });
        self.work.typefaces_created += 1;
        let resident_bytes = TYPEFACE_BASE_BYTES.saturating_add(bytes.len() as u64);
        self.residency.insert(
            ResourceDomain::Cpu,
            key,
            SkiaPaintResource::Typeface(typeface.clone()),
            resident_bytes,
            resident_bytes.saturating_mul(RESOURCE_REBUILD_COST_PER_BYTE),
        );
        typeface
    }

    pub(crate) fn text_blob_for(
        &mut self,
        text_run: TextRunId,
        data: &TextRun,
        font: &Font,
    ) -> Option<TextBlob> {
        let key = RenderResourceKey::Text(text_run);
        if let Some(SkiaPaintResource::TextBlob(cached)) =
            self.residency.get(ResourceDomain::Cpu, key)
        {
            return Some(cached.clone());
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
        self.residency.insert(
            ResourceDomain::Cpu,
            key,
            SkiaPaintResource::TextBlob(blob.clone()),
            bytes,
            bytes.saturating_mul(RESOURCE_REBUILD_COST_PER_BYTE),
        );
        Some(blob)
    }
}

fn image_resource_id(image: &RenderImage) -> ImageResourceId {
    let format = match image.format {
        hayate_core::RenderImageFormat::Rgba8 => 0,
    };
    let alpha_type = match image.alpha_type {
        RenderImageAlphaType::Opaque => 0,
        RenderImageAlphaType::Alpha => 1,
        RenderImageAlphaType::Premultiplied => 2,
    };
    ImageResourceId::new(
        image.data.id(),
        image.width,
        image.height,
        format,
        alpha_type,
    )
}

fn inverse_avar_segment(map: &SegmentMaps, post: f32) -> f32 {
    let maps = map.axis_value_maps();
    let from = |i: usize| maps[i].from_coordinate().to_f32();
    let to = |i: usize| maps[i].to_coordinate().to_f32();
    match maps.len() {
        0 => return post,
        1 => return post - to(0) + from(0),
        _ => {}
    }
    if post <= to(0) {
        return from(0);
    }
    for i in 1..maps.len() {
        if post <= to(i) {
            let span = to(i) - to(i - 1);
            if span <= 0.0 {
                return from(i);
            }
            return from(i - 1) + (post - to(i - 1)) * (from(i) - from(i - 1)) / span;
        }
    }
    from(maps.len() - 1)
}

fn design_coords_from_normalized(
    font: &hayate_core::RenderFont,
    normalized: &[i16],
) -> Vec<Coordinate> {
    let bytes: &[u8] = font.data.as_ref();
    let Ok(font_ref) = FontRef::from_index(bytes, font.index) else {
        return Vec::new();
    };
    let axes = font_ref.axes();
    let avar = font_ref.avar().ok();
    let mut out = Vec::with_capacity(axes.len());
    for (i, axis) in axes.iter().enumerate() {
        let Some(&raw) = normalized.get(i) else {
            break;
        };
        let post = f32::from(raw) / 16384.0;
        let pre = match avar
            .as_ref()
            .and_then(|a| a.axis_segment_maps().iter().nth(i))
        {
            Some(Ok(map)) => inverse_avar_segment(&map, post),
            _ => post,
        };
        let (min, def, max) = (axis.min_value(), axis.default_value(), axis.max_value());
        let design = if pre >= 0.0 {
            def + pre * (max - def)
        } else {
            def + pre * (def - min)
        };
        let tag = FourByteTag::from(u32::from_be_bytes(axis.tag().to_be_bytes()));
        out.push(Coordinate {
            axis: tag,
            value: design,
        });
    }
    out
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

    #[test]
    fn typeface_is_built_once_per_stable_font_instance_id() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/assets/twemoji_smiley_sbix.ttf"
        ))
        .expect("test font asset");
        let font =
            hayate_core::RenderFont::new(hayate_core::Blob::new(std::sync::Arc::new(bytes)), 0);
        let mut scene = hayate_core::SceneGraph::new();
        let text_run = scene.intern_text_run(hayate_core::TextRunData {
            font,
            font_size: 16.0,
            font_attributes: hayate_core::TextFontAttributes::default(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            text: std::sync::Arc::from("cache probe"),
            synthesis: hayate_core::TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let run = scene.resources().text_run(text_run).expect("text run");
        let instance = scene
            .resources()
            .font_instance(run.font_instance)
            .expect("font instance");
        let mut cache = PaintResourceCache::new();

        cache
            .typeface_for(run.font_instance, instance)
            .expect("first typeface");
        cache
            .typeface_for(run.font_instance, instance)
            .expect("cached typeface");

        assert_eq!(cache.work_counts().typefaces_created, 1);
        assert_eq!(cache.stats().cpu.hits, 1);
    }
}
