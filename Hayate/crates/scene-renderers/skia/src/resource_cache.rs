use hayate_core::{
    is_notdef, FontInstance, FontInstanceId, RenderImage, RenderImageAlphaType, TextRun, TextRunId,
};
use hayate_layer_compositor::{
    FontFaceResourceId, ImageResourceId, PoolBudgetPolicy, RenderResourceBudgetPolicy,
    RenderResourceKey, RenderResourceResidency, ResidencyEvent, ResidencyStats, ResourceDomain,
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
const FONT_FACE_BASE_BYTES: u64 = 1024;
const TYPEFACE_INSTANCE_BYTES: u64 = 1024;
const RESOURCE_REBUILD_COST_PER_BYTE: u64 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkiaResourceWorkCounts {
    pub font_managers_created: u64,
    pub font_data_copies: u64,
    pub typefaces_created: u64,
    pub text_blobs_created: u64,
    pub image_byte_copies: u64,
    pub sk_images_created: u64,
}

enum SkiaPaintResource {
    FontFace(Option<Typeface>),
    Typeface(Option<Typeface>),
    TextBlob(TextBlob),
    Image(Image),
}

/// The renderer-owned factory for Skia typefaces.
///
/// `skia-safe` does not mark `FontMgr` as `Send`, while native
/// [`hayate_layer_compositor::LayerRasterizer`] implementations must be movable into the Raster
/// Thread. The manager is therefore created lazily on the first raster that needs text and is
/// never shared: every operation requires `&mut PaintResourceCache`, and the manager is dropped
/// with that renderer.
struct RendererFontMgr {
    inner: Option<FontMgr>,
}

impl RendererFontMgr {
    fn new() -> Self {
        Self { inner: None }
    }

    fn get_or_init(&mut self) -> (&FontMgr, bool) {
        let created = self.inner.is_none();
        (self.inner.get_or_insert_with(FontMgr::default), created)
    }
}

// SAFETY: `RendererFontMgr` is owned exclusively by a `PaintResourceCache`; it is never exposed,
// cloned, or shared, and all access is serialized through `&mut PaintResourceCache`. SkFontMgr is
// atomically reference-counted and the only retained operation used here (`makeFromStream`) is
// safe before or after moving the owning renderer to its Raster Thread. The lazy `Option` also
// keeps Android's normal UI-thread -> Raster-thread handoff from constructing platform font state
// before that move.
unsafe impl Send for RendererFontMgr {}

pub(crate) struct PaintResourceCache {
    // Keep resident typefaces before the manager so they are dropped first.
    residency: RenderResourceResidency<SkiaPaintResource>,
    font_manager: RendererFontMgr,
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
            font_manager: RendererFontMgr::new(),
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
        let face_key =
            RenderResourceKey::FontFace(FontFaceResourceId::new(font.data.id(), font.index));
        let base = match self.residency.get(ResourceDomain::Cpu, face_key) {
            Some(SkiaPaintResource::FontFace(cached)) => cached.clone(),
            _ => {
                // `FontMgr::default()` performs Android system-font discovery. Keep that factory
                // for the renderer lifetime, and copy a Core font Blob into Skia only once even
                // when many variation/synthesis instances use the same face.
                let (font_manager, font_manager_created) = self.font_manager.get_or_init();
                self.work.font_managers_created += u64::from(font_manager_created);
                self.work.font_data_copies += 1;
                let base = font_manager.new_from_data(bytes, font.index as usize);
                let resident_bytes = FONT_FACE_BASE_BYTES.saturating_add(bytes.len() as u64);
                self.residency.insert(
                    ResourceDomain::Cpu,
                    face_key,
                    SkiaPaintResource::FontFace(base.clone()),
                    resident_bytes,
                    resident_bytes.saturating_mul(RESOURCE_REBUILD_COST_PER_BYTE),
                );
                base
            }
        };
        let typeface = base.and_then(|base| {
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
        self.residency.insert(
            ResourceDomain::Cpu,
            key,
            SkiaPaintResource::Typeface(typeface.clone()),
            TYPEFACE_INSTANCE_BYTES,
            TYPEFACE_INSTANCE_BYTES.saturating_mul(RESOURCE_REBUILD_COST_PER_BYTE),
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

    #[test]
    fn font_manager_is_built_once_for_distinct_font_instances() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/assets/twemoji_smiley_sbix.ttf"
        ))
        .expect("test font asset");
        let mut scene = hayate_core::SceneGraph::new();
        let first = scene.intern_text_run(hayate_core::TextRunData {
            font: hayate_core::RenderFont::new(
                hayate_core::Blob::new(std::sync::Arc::new(bytes.clone())),
                0,
            ),
            font_size: 16.0,
            font_attributes: hayate_core::TextFontAttributes::default(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            text: std::sync::Arc::from("first font instance"),
            synthesis: hayate_core::TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let second = scene.intern_text_run(hayate_core::TextRunData {
            font: hayate_core::RenderFont::new(
                hayate_core::Blob::new(std::sync::Arc::new(bytes)),
                0,
            ),
            font_size: 16.0,
            font_attributes: hayate_core::TextFontAttributes::default(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            text: std::sync::Arc::from("second font instance"),
            synthesis: hayate_core::TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let first = scene.resources().text_run(first).expect("first text run");
        let second = scene.resources().text_run(second).expect("second text run");
        assert_ne!(first.font_instance, second.font_instance);
        let mut cache = PaintResourceCache::new();

        cache
            .typeface_for(
                first.font_instance,
                scene
                    .resources()
                    .font_instance(first.font_instance)
                    .expect("first font instance"),
            )
            .expect("first typeface");
        cache
            .typeface_for(
                second.font_instance,
                scene
                    .resources()
                    .font_instance(second.font_instance)
                    .expect("second font instance"),
            )
            .expect("second typeface");

        assert_eq!(cache.work_counts().typefaces_created, 2);
        assert_eq!(
            cache.work_counts().font_managers_created,
            1,
            "system font discovery must not repeat for each FontInstanceId"
        );
    }

    #[test]
    fn font_data_is_copied_once_for_instances_of_the_same_face() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/assets/twemoji_smiley_sbix.ttf"
        ))
        .expect("test font asset");
        let font =
            hayate_core::RenderFont::new(hayate_core::Blob::new(std::sync::Arc::new(bytes)), 0);
        let mut scene = hayate_core::SceneGraph::new();
        let first = scene.intern_text_run(hayate_core::TextRunData {
            font: font.clone(),
            font_size: 16.0,
            font_attributes: hayate_core::TextFontAttributes::default(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            text: std::sync::Arc::from("base instance"),
            synthesis: hayate_core::TextSynthesis::default(),
            normalized_coords: Vec::new(),
        });
        let second = scene.intern_text_run(hayate_core::TextRunData {
            font,
            font_size: 16.0,
            font_attributes: hayate_core::TextFontAttributes::default(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            text: std::sync::Arc::from("varied instance"),
            synthesis: hayate_core::TextSynthesis::default(),
            normalized_coords: vec![8_192],
        });
        let first = scene.resources().text_run(first).expect("first text run");
        let second = scene.resources().text_run(second).expect("second text run");
        assert_ne!(first.font_instance, second.font_instance);
        let mut cache = PaintResourceCache::new();

        for id in [first.font_instance, second.font_instance] {
            cache
                .typeface_for(
                    id,
                    scene.resources().font_instance(id).expect("font instance"),
                )
                .expect("typeface");
        }

        assert_eq!(cache.work_counts().typefaces_created, 2);
        assert_eq!(
            cache.work_counts().font_data_copies,
            1,
            "font bytes shared by variation instances must be copied into Skia once"
        );
    }
}
