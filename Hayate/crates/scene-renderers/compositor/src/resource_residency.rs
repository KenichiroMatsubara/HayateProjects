use std::cmp::Ordering;
use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::{FontInstanceId, TextRunId};

pub const CPU_BUDGET_CONSTRAINED_BYTES: u64 = 16 * 1024 * 1024;
pub const CPU_BUDGET_BALANCED_BYTES: u64 = 32 * 1024 * 1024;
pub const CPU_BUDGET_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
pub const GPU_VIEWPORTS_CONSTRAINED: u64 = 3;
pub const GPU_VIEWPORTS_BALANCED: u64 = 6;
pub const GPU_VIEWPORTS_EXPANDED: u64 = 10;
pub const RESOURCE_HIGH_WATERMARK_PERCENT: u64 = 90;
pub const RESOURCE_LOW_WATERMARK_PERCENT: u64 = 75;
pub const RESOURCE_EVICTION_BATCH: usize = 4;
const PERCENT_DENOMINATOR: u64 = 100;
const RGBA8_BYTES_PER_PIXEL: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMemoryClass {
    Constrained,
    Balanced,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudgetInputs {
    pub memory_class: DeviceMemoryClass,
    pub surface_width: u32,
    pub surface_height: u32,
}

impl ResourceBudgetInputs {
    pub const fn new(
        memory_class: DeviceMemoryClass,
        surface_width: u32,
        surface_height: u32,
    ) -> Self {
        Self {
            memory_class,
            surface_width,
            surface_height,
        }
    }
}

/// CPU-backed and GPU-backed resources have independent byte semantics and lifecycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceDomain {
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyEvent {
    SurfaceLost,
    ContextLost,
    MemoryPressure(MemoryPressure),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Moderate,
}

/// Fixed-size identity for a Core image source and its raster interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageResourceId {
    pub blob_id: u64,
    pub width: u32,
    pub height: u32,
    pub format: u8,
    pub alpha_type: u8,
}

impl ImageResourceId {
    pub const fn new(blob_id: u64, width: u32, height: u32, format: u8, alpha_type: u8) -> Self {
        Self {
            blob_id,
            width,
            height,
            format,
            alpha_type,
        }
    }
}

/// Fixed-size identity for one Core font blob and face index, independent of variation,
/// synthesis, and other [`FontInstanceId`] attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFaceResourceId {
    pub blob_id: u64,
    pub face_index: u32,
}

impl FontFaceResourceId {
    pub const fn new(blob_id: u64, face_index: u32) -> Self {
        Self {
            blob_id,
            face_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayerResourcePlane {
    Content,
    ScrollChrome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerResourceId {
    pub layer: ElementId,
    pub plane: LayerResourcePlane,
}

impl LayerResourceId {
    pub const fn new(layer: ElementId, plane: LayerResourcePlane) -> Self {
        Self { layer, plane }
    }
}

/// Constant-sized lookup key used by renderer adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderResourceKey {
    FontFace(FontFaceResourceId),
    Font(FontInstanceId),
    Text(TextRunId),
    Image(ImageResourceId),
    Layer(LayerResourceId),
}

impl RenderResourceKey {
    fn order_parts(self) -> (u8, u64, u64, u64) {
        match self {
            Self::FontFace(id) => (0, id.blob_id, u64::from(id.face_index), 0),
            Self::Font(id) => {
                let (arena, slot) = id.to_raw_parts();
                (1, arena, slot, 0)
            }
            Self::Text(id) => {
                let (arena, slot) = id.to_raw_parts();
                (2, arena, slot, 0)
            }
            Self::Image(id) => {
                let dimensions = u64::from(id.width) << 32 | u64::from(id.height);
                let interpretation = u64::from(id.format) << 8 | u64::from(id.alpha_type);
                (3, id.blob_id, dimensions, interpretation)
            }
            Self::Layer(id) => (4, id.layer.to_u64(), id.plane as u64, 0),
        }
    }
}

impl Ord for RenderResourceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_parts().cmp(&other.order_parts())
    }
}

impl PartialOrd for RenderResourceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Typed byte budget for one memory domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolBudgetPolicy {
    pub max_bytes: u64,
    pub high_watermark_bytes: u64,
    pub low_watermark_bytes: u64,
    pub eviction_batch: usize,
}

impl PoolBudgetPolicy {
    pub const fn fixed(max_bytes: u64) -> Self {
        Self {
            max_bytes,
            high_watermark_bytes: max_bytes,
            low_watermark_bytes: max_bytes,
            eviction_batch: 1,
        }
    }

    pub fn new(
        max_bytes: u64,
        high_watermark_bytes: u64,
        low_watermark_bytes: u64,
        eviction_batch: usize,
    ) -> Result<Self, &'static str> {
        if low_watermark_bytes > high_watermark_bytes {
            return Err("low watermark must not exceed high watermark");
        }
        if high_watermark_bytes > max_bytes {
            return Err("high watermark must not exceed maximum bytes");
        }
        if eviction_batch == 0 {
            return Err("eviction batch must be non-zero");
        }
        Ok(Self {
            max_bytes,
            high_watermark_bytes,
            low_watermark_bytes,
            eviction_batch,
        })
    }
}

/// One policy selected by Render Host and applied to the selected renderer instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourceBudgetPolicy {
    pub cpu: PoolBudgetPolicy,
    pub gpu: PoolBudgetPolicy,
}

impl RenderResourceBudgetPolicy {
    pub fn for_device(inputs: ResourceBudgetInputs) -> Self {
        let (cpu_max, gpu_viewports) = match inputs.memory_class {
            DeviceMemoryClass::Constrained => {
                (CPU_BUDGET_CONSTRAINED_BYTES, GPU_VIEWPORTS_CONSTRAINED)
            }
            DeviceMemoryClass::Balanced => (CPU_BUDGET_BALANCED_BYTES, GPU_VIEWPORTS_BALANCED),
            DeviceMemoryClass::Expanded => (CPU_BUDGET_EXPANDED_BYTES, GPU_VIEWPORTS_EXPANDED),
        };
        let surface_bytes = u64::from(inputs.surface_width.max(1))
            * u64::from(inputs.surface_height.max(1))
            * RGBA8_BYTES_PER_PIXEL;
        Self {
            cpu: watermarked_policy(cpu_max),
            gpu: watermarked_policy(surface_bytes.saturating_mul(gpu_viewports)),
        }
    }
}

fn watermarked_policy(max_bytes: u64) -> PoolBudgetPolicy {
    PoolBudgetPolicy::new(
        max_bytes,
        max_bytes.saturating_mul(RESOURCE_HIGH_WATERMARK_PERCENT) / PERCENT_DENOMINATOR,
        max_bytes.saturating_mul(RESOURCE_LOW_WATERMARK_PERCENT) / PERCENT_DENOMINATOR,
        RESOURCE_EVICTION_BATCH,
    )
    .expect("named residency watermark constants form a valid policy")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolResidencyStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub resident_bytes: u64,
    pub rebuild_cost: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResidencyStats {
    pub cpu: PoolResidencyStats,
    pub gpu: PoolResidencyStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidencyMutation {
    pub evicted: Vec<RenderResourceKey>,
}

struct Entry<R> {
    resource: R,
    bytes: u64,
    last_used: u64,
    logical_ledger_retained: bool,
}

struct ResourcePool<R> {
    entries: HashMap<RenderResourceKey, Entry<R>>,
    policy: PoolBudgetPolicy,
    stats: PoolResidencyStats,
    clock: u64,
}

impl<R> ResourcePool<R> {
    fn new(policy: PoolBudgetPolicy) -> Self {
        Self {
            entries: HashMap::new(),
            policy,
            stats: PoolResidencyStats::default(),
            clock: 0,
        }
    }

    fn get(&mut self, key: RenderResourceKey) -> Option<&R> {
        self.clock = self.clock.wrapping_add(1);
        match self.entries.get_mut(&key) {
            Some(entry) => {
                entry.last_used = self.clock;
                self.stats.hits += 1;
                Some(&entry.resource)
            }
            None => {
                self.stats.misses += 1;
                None
            }
        }
    }

    fn insert(
        &mut self,
        key: RenderResourceKey,
        resource: R,
        bytes: u64,
        rebuild_cost: u64,
    ) -> ResidencyMutation {
        self.insert_with_sweep(key, resource, bytes, rebuild_cost, true)
    }

    fn insert_retained(
        &mut self,
        key: RenderResourceKey,
        resource: R,
        bytes: u64,
        rebuild_cost: u64,
    ) -> ResidencyMutation {
        self.insert_with_sweep(key, resource, bytes, rebuild_cost, false)
    }

    fn insert_with_sweep(
        &mut self,
        key: RenderResourceKey,
        resource: R,
        bytes: u64,
        rebuild_cost: u64,
        sweep: bool,
    ) -> ResidencyMutation {
        self.clock = self.clock.wrapping_add(1);
        if let Some(replaced) = self.entries.insert(
            key,
            Entry {
                resource,
                bytes,
                last_used: self.clock,
                logical_ledger_retained: !sweep,
            },
        ) {
            self.stats.resident_bytes = self.stats.resident_bytes.saturating_sub(replaced.bytes);
        }
        self.stats.resident_bytes = self.stats.resident_bytes.saturating_add(bytes);
        self.stats.rebuild_cost = self.stats.rebuild_cost.saturating_add(rebuild_cost);

        let mut evicted = Vec::new();
        let should_sweep = sweep && self.stats.resident_bytes > self.policy.high_watermark_bytes;
        while should_sweep
            && (self.stats.resident_bytes > self.policy.low_watermark_bytes
                || evicted.len() < self.policy.eviction_batch)
        {
            let Some(victim) = self
                .entries
                .iter()
                .filter(|(_, entry)| !entry.logical_ledger_retained)
                .min_by_key(|(key, entry)| (entry.last_used, **key))
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.stats.resident_bytes = self.stats.resident_bytes.saturating_sub(entry.bytes);
                self.stats.evictions += 1;
                evicted.push(victim);
            }
        }
        ResidencyMutation { evicted }
    }

    fn peek(&self, key: RenderResourceKey) -> Option<&R> {
        self.entries.get(&key).map(|entry| &entry.resource)
    }

    fn remove(&mut self, key: RenderResourceKey) -> Option<R> {
        let entry = self.entries.remove(&key)?;
        self.stats.resident_bytes = self.stats.resident_bytes.saturating_sub(entry.bytes);
        self.stats.evictions += 1;
        Some(entry.resource)
    }

    fn contains(&self, key: RenderResourceKey) -> bool {
        self.entries.contains_key(&key)
    }

    fn set_policy(&mut self, policy: PoolBudgetPolicy) -> ResidencyMutation {
        self.policy = policy;
        self.trim_to(policy.low_watermark_bytes.min(policy.max_bytes))
    }

    fn evict_all(&mut self) -> ResidencyMutation {
        let mut ordered = self
            .entries
            .iter()
            .map(|(key, entry)| (entry.last_used, *key))
            .collect::<Vec<_>>();
        ordered.sort_unstable();
        let evicted = ordered.into_iter().map(|(_, key)| key).collect::<Vec<_>>();
        self.entries.clear();
        self.stats.evictions = self.stats.evictions.saturating_add(evicted.len() as u64);
        self.stats.resident_bytes = 0;
        ResidencyMutation { evicted }
    }

    fn trim_to(&mut self, target_bytes: u64) -> ResidencyMutation {
        let mut evicted = Vec::new();
        while self.stats.resident_bytes > target_bytes {
            let Some(victim) = self
                .entries
                .iter()
                .filter(|(_, entry)| !entry.logical_ledger_retained)
                .min_by_key(|(key, entry)| (entry.last_used, **key))
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.stats.resident_bytes = self.stats.resident_bytes.saturating_sub(entry.bytes);
                self.stats.evictions += 1;
                evicted.push(victim);
            }
        }
        ResidencyMutation { evicted }
    }
}

/// Renderer-scoped owner of concrete CPU and GPU resources.
pub struct RenderResourceResidency<R> {
    cpu: ResourcePool<R>,
    gpu: ResourcePool<R>,
    shutdown: bool,
}

impl<R> RenderResourceResidency<R> {
    pub fn new(policy: RenderResourceBudgetPolicy) -> Self {
        Self {
            cpu: ResourcePool::new(policy.cpu),
            gpu: ResourcePool::new(policy.gpu),
            shutdown: false,
        }
    }

    pub fn get(&mut self, domain: ResourceDomain, key: RenderResourceKey) -> Option<&R> {
        match domain {
            ResourceDomain::Cpu => self.cpu.get(key),
            ResourceDomain::Gpu => self.gpu.get(key),
        }
    }

    pub fn contains(&self, domain: ResourceDomain, key: RenderResourceKey) -> bool {
        match domain {
            ResourceDomain::Cpu => self.cpu.contains(key),
            ResourceDomain::Gpu => self.gpu.contains(key),
        }
    }

    pub fn set_policy(&mut self, policy: RenderResourceBudgetPolicy) -> ResidencyMutation {
        let mut mutation = self.cpu.set_policy(policy.cpu);
        mutation
            .evicted
            .extend(self.gpu.set_policy(policy.gpu).evicted);
        mutation
    }

    pub fn insert(
        &mut self,
        domain: ResourceDomain,
        key: RenderResourceKey,
        resource: R,
        bytes: u64,
        rebuild_cost: u64,
    ) -> ResidencyMutation {
        if self.shutdown {
            return ResidencyMutation {
                evicted: Vec::new(),
            };
        }
        match domain {
            ResourceDomain::Cpu => self.cpu.insert(key, resource, bytes, rebuild_cost),
            ResourceDomain::Gpu => self.gpu.insert(key, resource, bytes, rebuild_cost),
        }
    }

    pub fn insert_retained(
        &mut self,
        domain: ResourceDomain,
        key: RenderResourceKey,
        resource: R,
        bytes: u64,
        rebuild_cost: u64,
    ) -> ResidencyMutation {
        if self.shutdown {
            return ResidencyMutation {
                evicted: Vec::new(),
            };
        }
        match domain {
            ResourceDomain::Cpu => self.cpu.insert_retained(key, resource, bytes, rebuild_cost),
            ResourceDomain::Gpu => self.gpu.insert_retained(key, resource, bytes, rebuild_cost),
        }
    }

    pub fn peek(&self, domain: ResourceDomain, key: RenderResourceKey) -> Option<&R> {
        match domain {
            ResourceDomain::Cpu => self.cpu.peek(key),
            ResourceDomain::Gpu => self.gpu.peek(key),
        }
    }

    pub fn remove(&mut self, domain: ResourceDomain, key: RenderResourceKey) -> Option<R> {
        match domain {
            ResourceDomain::Cpu => self.cpu.remove(key),
            ResourceDomain::Gpu => self.gpu.remove(key),
        }
    }

    /// Release every resource in one memory domain without disabling future inserts.
    /// Renderer resize paths use this for resources whose dimensions are no longer valid;
    /// lifecycle loss remains narrower and clears only the GPU domain.
    pub fn clear_domain(&mut self, domain: ResourceDomain) -> ResidencyMutation {
        match domain {
            ResourceDomain::Cpu => self.cpu.evict_all(),
            ResourceDomain::Gpu => self.gpu.evict_all(),
        }
    }

    pub fn stats(&self) -> ResidencyStats {
        ResidencyStats {
            cpu: self.cpu.stats,
            gpu: self.gpu.stats,
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    pub fn handle_lifecycle(&mut self, event: ResidencyEvent) -> ResidencyMutation {
        match event {
            ResidencyEvent::SurfaceLost | ResidencyEvent::ContextLost => self.gpu.evict_all(),
            ResidencyEvent::MemoryPressure(MemoryPressure::Moderate) => {
                let mut mutation = self.cpu.trim_to(self.cpu.policy.low_watermark_bytes);
                mutation.evicted.extend(
                    self.gpu
                        .trim_to(self.gpu.policy.low_watermark_bytes)
                        .evicted,
                );
                mutation
            }
            ResidencyEvent::Shutdown => {
                self.shutdown = true;
                let mut mutation = self.cpu.evict_all();
                mutation.evicted.extend(self.gpu.evict_all().evicted);
                mutation
            }
        }
    }
}
