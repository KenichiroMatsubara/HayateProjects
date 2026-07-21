use std::collections::{HashMap, HashSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use slotmap::{new_key_type, Key, SlotMap};

use crate::node::{
    TextDecorationLine, TextFontAttributes, TextFontSlant, TextRunData, TextSynthesis,
};
use crate::render::{RenderFont, RenderGlyph};

new_key_type! {
    struct FontSlot;
    struct TextRunSlot;
}

static NEXT_RESOURCE_ARENA_ID: AtomicU64 = AtomicU64::new(1);

/// Default number of retired text nodes accumulated before Core prunes resource pins.
pub const DEFAULT_TEXT_RESOURCE_SWEEP_THRESHOLD: usize = 64;

/// Typed tuning policy for Core's text-resource intern table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextResourcePolicy {
    sweep_threshold: NonZeroUsize,
}

impl TextResourcePolicy {
    pub fn new(sweep_threshold: usize) -> Option<Self> {
        NonZeroUsize::new(sweep_threshold).map(|sweep_threshold| Self { sweep_threshold })
    }

    pub fn sweep_threshold(self) -> usize {
        self.sweep_threshold.get()
    }
}

impl Default for TextResourcePolicy {
    fn default() -> Self {
        Self::new(DEFAULT_TEXT_RESOURCE_SWEEP_THRESHOLD)
            .expect("the default text resource sweep threshold is non-zero")
    }
}

/// Stable, fixed-size identity for one immutable Core-owned [`FontInstance`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FontInstanceId {
    arena: u64,
    slot: u64,
}

impl fmt::Debug for FontInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FontInstanceId")
            .field(&self.arena)
            .field(&self.slot)
            .finish()
    }
}

/// Stable, fixed-size identity for one immutable Core-owned [`TextRun`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct TextRunId {
    arena: u64,
    slot: u64,
}

impl fmt::Debug for TextRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TextRunId")
            .field(&self.arena)
            .field(&self.slot)
            .finish()
    }
}

impl FontInstanceId {
    /// Rebuild an ID carried over a fixed-width wire projection. Invalid or stale parts remain
    /// harmless: resource lookup returns [`ResourceLookupError`].
    pub const fn from_raw_parts(arena: u64, slot: u64) -> Self {
        Self { arena, slot }
    }

    pub const fn to_raw_parts(self) -> (u64, u64) {
        (self.arena, self.slot)
    }

    fn new(arena: u64, slot: FontSlot) -> Self {
        Self {
            arena,
            slot: slot.data().as_ffi(),
        }
    }
}

impl TextRunId {
    /// Rebuild an ID carried over a fixed-width wire projection. Invalid or stale parts remain
    /// harmless: resource lookup returns [`ResourceLookupError`].
    pub const fn from_raw_parts(arena: u64, slot: u64) -> Self {
        Self { arena, slot }
    }

    pub const fn to_raw_parts(self) -> (u64, u64) {
        (self.arena, self.slot)
    }

    fn new(arena: u64, slot: TextRunSlot) -> Self {
        Self {
            arena,
            slot: slot.data().as_ffi(),
        }
    }
}

/// Renderer-neutral font source and all values that select its raster instance.
#[derive(Debug)]
pub struct FontInstance {
    pub font: RenderFont,
    pub font_attributes: TextFontAttributes,
    pub synthesis: TextSynthesis,
    pub normalized_coords: Arc<[i16]>,
}

/// Immutable shaped text owned by Core and referenced from retained scene nodes by ID.
#[derive(Debug)]
pub struct TextRun {
    pub font_instance: FontInstanceId,
    pub font_size: f32,
    pub glyphs: Arc<[RenderGlyph]>,
    pub decorations: Arc<[TextDecorationLine]>,
    pub text: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLookupError {
    StaleFontInstance(FontInstanceId),
    StaleTextRun(TextRunId),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceSweepStats {
    pub font_instances: usize,
    pub text_runs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
    blob_id: u64,
    face_index: u32,
    weight: u32,
    width: u32,
    slant: u8,
    skew_tangent: Option<u32>,
    embolden: Option<u32>,
    normalized_coords: Vec<i16>,
}

impl FontKey {
    fn new(
        font: &RenderFont,
        attributes: TextFontAttributes,
        synthesis: TextSynthesis,
        normalized_coords: &[i16],
    ) -> Self {
        Self {
            blob_id: font.data.id(),
            face_index: font.index,
            weight: attributes.weight.to_bits(),
            width: attributes.width.to_bits(),
            slant: match attributes.slant {
                TextFontSlant::Upright => 0,
                TextFontSlant::Italic => 1,
                TextFontSlant::Oblique => 2,
            },
            skew_tangent: synthesis.skew_tangent.map(f32::to_bits),
            embolden: synthesis.embolden.map(f32::to_bits),
            normalized_coords: normalized_coords.to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextRunKey {
    font_instance: FontInstanceId,
    font_size: u32,
    glyphs: Vec<(u32, u32, u32)>,
    decorations: Vec<(u32, u32, u32, u32)>,
    text: Arc<str>,
}

impl TextRunKey {
    fn new(
        font_instance: FontInstanceId,
        font_size: f32,
        glyphs: &[RenderGlyph],
        decorations: &[TextDecorationLine],
        text: Arc<str>,
    ) -> Self {
        Self {
            font_instance,
            font_size: font_size.to_bits(),
            glyphs: glyphs
                .iter()
                .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                .collect(),
            decorations: decorations
                .iter()
                .map(|line| {
                    (
                        line.x0.to_bits(),
                        line.x1.to_bits(),
                        line.y.to_bits(),
                        line.thickness.to_bits(),
                    )
                })
                .collect(),
            text,
        }
    }
}

#[derive(Debug)]
struct FontEntry {
    key: FontKey,
    value: Arc<FontInstance>,
}

#[derive(Debug)]
struct TextRunEntry {
    key: TextRunKey,
    value: Arc<TextRun>,
}

#[derive(Debug)]
struct InternerState {
    arena: u64,
    fonts: SlotMap<FontSlot, FontEntry>,
    font_ids: HashMap<FontKey, FontSlot>,
    text_runs: SlotMap<TextRunSlot, TextRunEntry>,
    text_run_ids: HashMap<TextRunKey, TextRunSlot>,
}

impl InternerState {
    fn new() -> Self {
        Self {
            arena: NEXT_RESOURCE_ARENA_ID.fetch_add(1, Ordering::Relaxed),
            fonts: SlotMap::with_key(),
            font_ids: HashMap::new(),
            text_runs: SlotMap::with_key(),
            text_run_ids: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct TextResourceInterner {
    state: Arc<Mutex<InternerState>>,
}

impl fmt::Debug for TextResourceInterner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextResourceInterner")
            .finish_non_exhaustive()
    }
}

pub(crate) struct InternedTextRun {
    pub id: TextRunId,
    pub run: Arc<TextRun>,
    pub font_id: FontInstanceId,
    pub font: Arc<FontInstance>,
}

impl TextResourceInterner {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(InternerState::new())),
        }
    }

    pub fn intern_text_run(&self, data: TextRunData) -> InternedTextRun {
        let TextRunData {
            font,
            font_size,
            font_attributes,
            glyphs,
            decorations,
            text,
            synthesis,
            normalized_coords,
        } = data;
        let mut state = self.state.lock().expect("text resource interner poisoned");
        let arena = state.arena;
        let font_key = FontKey::new(&font, font_attributes, synthesis, &normalized_coords);
        let (font_id, font_value) = match state.font_ids.get(&font_key).copied() {
            Some(slot) => {
                let entry = state
                    .fonts
                    .get(slot)
                    .expect("font intern index must be valid");
                (FontInstanceId::new(arena, slot), Arc::clone(&entry.value))
            }
            None => {
                let value = Arc::new(FontInstance {
                    font,
                    font_attributes,
                    synthesis,
                    normalized_coords: normalized_coords.into(),
                });
                let slot = state.fonts.insert(FontEntry {
                    key: font_key.clone(),
                    value: Arc::clone(&value),
                });
                state.font_ids.insert(font_key, slot);
                (FontInstanceId::new(arena, slot), value)
            }
        };

        let text_key =
            TextRunKey::new(font_id, font_size, &glyphs, &decorations, Arc::clone(&text));
        let (id, run) = match state.text_run_ids.get(&text_key).copied() {
            Some(slot) => {
                let entry = state
                    .text_runs
                    .get(slot)
                    .expect("text run intern index must be valid");
                (TextRunId::new(arena, slot), Arc::clone(&entry.value))
            }
            None => {
                let value = Arc::new(TextRun {
                    font_instance: font_id,
                    font_size,
                    glyphs: glyphs.into(),
                    decorations: decorations.into(),
                    text,
                });
                let slot = state.text_runs.insert(TextRunEntry {
                    key: text_key.clone(),
                    value: Arc::clone(&value),
                });
                state.text_run_ids.insert(text_key, slot);
                (TextRunId::new(arena, slot), value)
            }
        };

        InternedTextRun {
            id,
            run,
            font_id,
            font: font_value,
        }
    }

    pub fn sweep(&self) -> ResourceSweepStats {
        let mut state = self.state.lock().expect("text resource interner poisoned");
        let retired_text_runs: Vec<TextRunSlot> = state
            .text_runs
            .iter()
            .filter_map(|(slot, entry)| (Arc::strong_count(&entry.value) == 1).then_some(slot))
            .collect();
        for slot in &retired_text_runs {
            if let Some(entry) = state.text_runs.remove(*slot) {
                state.text_run_ids.remove(&entry.key);
            }
        }
        let retired_fonts: Vec<FontSlot> = state
            .fonts
            .iter()
            .filter_map(|(slot, entry)| (Arc::strong_count(&entry.value) == 1).then_some(slot))
            .collect();
        for slot in &retired_fonts {
            if let Some(entry) = state.fonts.remove(*slot) {
                state.font_ids.remove(&entry.key);
            }
        }
        ResourceSweepStats {
            font_instances: retired_fonts.len(),
            text_runs: retired_text_runs.len(),
        }
    }
}

/// Immutable lookup view carried by a SceneGraph snapshot.
#[derive(Debug, Clone, Default)]
pub struct SceneResources {
    fonts: Arc<HashMap<FontInstanceId, Arc<FontInstance>>>,
    text_runs: Arc<HashMap<TextRunId, Arc<TextRun>>>,
}

impl SceneResources {
    pub fn font_instance(&self, id: FontInstanceId) -> Result<&FontInstance, ResourceLookupError> {
        self.fonts
            .get(&id)
            .map(Arc::as_ref)
            .ok_or(ResourceLookupError::StaleFontInstance(id))
    }

    pub fn text_run(&self, id: TextRunId) -> Result<&TextRun, ResourceLookupError> {
        self.text_runs
            .get(&id)
            .map(Arc::as_ref)
            .ok_or(ResourceLookupError::StaleTextRun(id))
    }

    pub(crate) fn pin(&mut self, interned: InternedTextRun) {
        if !self.fonts.contains_key(&interned.font_id) {
            Arc::make_mut(&mut self.fonts).insert(interned.font_id, interned.font);
        }
        if !self.text_runs.contains_key(&interned.id) {
            Arc::make_mut(&mut self.text_runs).insert(interned.id, interned.run);
        }
    }

    pub(crate) fn retain_text_runs(&mut self, live: &HashSet<TextRunId>) {
        let text_runs = Arc::make_mut(&mut self.text_runs);
        text_runs.retain(|id, _| live.contains(id));
        let live_fonts: HashSet<FontInstanceId> =
            text_runs.values().map(|run| run.font_instance).collect();
        Arc::make_mut(&mut self.fonts).retain(|id, _| live_fonts.contains(id));
    }
}
