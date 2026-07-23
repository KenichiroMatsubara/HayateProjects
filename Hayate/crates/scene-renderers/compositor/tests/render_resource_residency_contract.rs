use hayate_core::{element::id::ElementId, FontInstanceId, TextRunId};
use hayate_layer_compositor::{
    FontFaceResourceId, ImageResourceId, LayerResourceId, LayerResourcePlane, MemoryPressure,
    PoolBudgetPolicy, RenderResourceBudgetPolicy, RenderResourceKey, RenderResourceResidency,
    ResidencyEvent, ResourceDomain,
};

fn image(blob_id: u64) -> RenderResourceKey {
    RenderResourceKey::Image(ImageResourceId::new(blob_id, 1, 1, 0, 0))
}

#[test]
fn surface_and_context_loss_release_only_gpu_resources() {
    for event in [ResidencyEvent::SurfaceLost, ResidencyEvent::ContextLost] {
        let pool = PoolBudgetPolicy::fixed(64);
        let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
            cpu: pool,
            gpu: pool,
        });
        residency.insert(ResourceDomain::Cpu, image(10), "decoded image", 4, 1);
        residency.insert(ResourceDomain::Gpu, image(11), "uploaded image", 4, 1);

        let mutation = residency.handle_lifecycle(event);

        assert_eq!(mutation.evicted, vec![image(11)]);
        assert!(residency.contains(ResourceDomain::Cpu, image(10)));
        assert!(!residency.contains(ResourceDomain::Gpu, image(11)));
    }
}

#[test]
fn renderer_can_explicitly_clear_one_domain_without_entering_shutdown() {
    let pool = PoolBudgetPolicy::fixed(64);
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });
    residency.insert(ResourceDomain::Cpu, image(10), "cpu", 4, 1);
    residency.insert(ResourceDomain::Gpu, image(11), "gpu", 4, 1);

    let mutation = residency.clear_domain(ResourceDomain::Cpu);
    residency.insert(ResourceDomain::Cpu, image(12), "replacement", 4, 1);

    assert_eq!(mutation.evicted, vec![image(10)]);
    assert!(!residency.is_shutdown());
    assert!(residency.contains(ResourceDomain::Gpu, image(11)));
    assert!(residency.contains(ResourceDomain::Cpu, image(12)));
}

#[test]
fn core_resource_ids_are_constant_sized_renderer_lookup_keys() {
    let keys = [
        RenderResourceKey::FontFace(FontFaceResourceId::new(1, 0)),
        RenderResourceKey::Font(FontInstanceId::from_raw_parts(1, 2)),
        RenderResourceKey::Text(TextRunId::from_raw_parts(3, 4)),
        RenderResourceKey::Image(ImageResourceId::new(5, 6, 7, 0, 1)),
        RenderResourceKey::Layer(LayerResourceId::new(
            ElementId::from_u64(8),
            LayerResourcePlane::Content,
        )),
    ];

    assert_eq!(
        std::mem::size_of_val(&keys),
        5 * std::mem::size_of::<RenderResourceKey>()
    );
    assert!(std::mem::size_of::<RenderResourceKey>() <= 32);
}

#[test]
fn normal_sweep_uses_named_watermarks_and_an_eviction_batch() {
    let pool = PoolBudgetPolicy::new(16, 12, 8, 2).expect("valid watermark policy");
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });
    for blob_id in 1..=3 {
        assert!(residency
            .insert(ResourceDomain::Cpu, image(blob_id), blob_id, 4, 1)
            .evicted
            .is_empty());
    }

    let mutation = residency.insert(ResourceDomain::Cpu, image(4), 4, 4, 1);

    assert_eq!(mutation.evicted, vec![image(1), image(2)]);
    assert_eq!(residency.stats().cpu.resident_bytes, 8);
}

#[test]
fn moderate_memory_pressure_trims_each_pool_to_its_low_watermark() {
    let pool = PoolBudgetPolicy::new(64, 48, 8, 1).expect("valid watermark policy");
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });
    for blob_id in 1..=2 {
        residency.insert(ResourceDomain::Cpu, image(blob_id), blob_id, 8, 1);
    }
    for blob_id in 3..=4 {
        residency.insert(ResourceDomain::Gpu, image(blob_id), blob_id, 8, 1);
    }

    let mutation =
        residency.handle_lifecycle(ResidencyEvent::MemoryPressure(MemoryPressure::Moderate));

    assert_eq!(mutation.evicted, vec![image(1), image(3)]);
    assert_eq!(residency.stats().cpu.resident_bytes, 8);
    assert_eq!(residency.stats().gpu.resident_bytes, 8);
}

#[test]
fn shutdown_releases_every_pool_and_prevents_new_residency() {
    let pool = PoolBudgetPolicy::fixed(64);
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });
    residency.insert(ResourceDomain::Cpu, image(1), "cpu", 4, 1);
    residency.insert(ResourceDomain::Gpu, image(2), "gpu", 4, 1);

    let mutation = residency.handle_lifecycle(ResidencyEvent::Shutdown);
    residency.insert(ResourceDomain::Cpu, image(3), "late", 4, 1);

    assert_eq!(mutation.evicted, vec![image(1), image(2)]);
    assert!(residency.is_shutdown());
    assert!(!residency.contains(ResourceDomain::Cpu, image(3)));
    assert_eq!(residency.stats().cpu.resident_bytes, 0);
    assert_eq!(residency.stats().gpu.resident_bytes, 0);
}

#[test]
fn retained_layer_resources_wait_for_the_logical_ledger_eviction_decision() {
    let pool = PoolBudgetPolicy::fixed(4);
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });

    residency.insert_retained(ResourceDomain::Gpu, image(1), "first", 4, 1);
    let mutation = residency.insert_retained(ResourceDomain::Gpu, image(2), "second", 4, 1);

    assert!(mutation.evicted.is_empty());
    assert_eq!(residency.stats().gpu.resident_bytes, 8);
    assert_eq!(
        residency.remove(ResourceDomain::Gpu, image(1)),
        Some("first")
    );
    assert_eq!(residency.stats().gpu.resident_bytes, 4);
}

#[test]
fn memory_pressure_does_not_bypass_the_logical_layer_ledger() {
    let pool = PoolBudgetPolicy::new(16, 12, 4, 1).expect("valid watermark policy");
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });
    let layer = RenderResourceKey::Layer(LayerResourceId::new(
        ElementId::from_u64(41),
        LayerResourcePlane::Content,
    ));
    residency.insert_retained(ResourceDomain::Gpu, layer, "texture", 12, 1);

    let mutation =
        residency.handle_lifecycle(ResidencyEvent::MemoryPressure(MemoryPressure::Moderate));

    assert!(mutation.evicted.is_empty());
    assert!(residency.contains(ResourceDomain::Gpu, layer));
}

#[test]
fn cpu_and_gpu_resources_have_independent_deterministic_budgets() {
    let pool = PoolBudgetPolicy::fixed(8);
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });

    residency.insert(ResourceDomain::Cpu, image(1), "cpu-oldest", 4, 1);
    residency.insert(ResourceDomain::Cpu, image(2), "cpu-newest", 4, 1);
    residency.insert(ResourceDomain::Gpu, image(3), "gpu", 8, 1);
    assert_eq!(
        residency.get(ResourceDomain::Cpu, image(1)),
        Some(&"cpu-oldest")
    );

    let mutation = residency.insert(ResourceDomain::Cpu, image(4), "cpu-pressure", 4, 1);

    assert_eq!(mutation.evicted, vec![image(2)]);
    assert_eq!(
        residency.get(ResourceDomain::Cpu, image(1)),
        Some(&"cpu-oldest")
    );
    assert_eq!(
        residency.get(ResourceDomain::Cpu, image(4)),
        Some(&"cpu-pressure")
    );
    assert_eq!(residency.get(ResourceDomain::Gpu, image(3)), Some(&"gpu"));
    assert_eq!(residency.stats().cpu.resident_bytes, 8);
    assert_eq!(residency.stats().gpu.resident_bytes, 8);
}

#[test]
fn memory_stress_reaches_steady_state_and_recovers_with_observable_work() {
    let pool = PoolBudgetPolicy::new(64, 48, 32, 2).expect("valid watermark policy");
    let mut residency = RenderResourceResidency::new(RenderResourceBudgetPolicy {
        cpu: pool,
        gpu: pool,
    });

    for cycle in 0..8 {
        for slot in 0..16 {
            residency.insert(
                ResourceDomain::Cpu,
                image(cycle * 16 + slot),
                vec![slot as u8; 8],
                8,
                8,
            );
        }
        assert!(residency.stats().cpu.resident_bytes <= pool.high_watermark_bytes);
    }
    let stressed = residency.stats();
    assert!(stressed.cpu.evictions > 0);
    assert!(stressed.cpu.rebuild_cost > 0);

    residency.handle_lifecycle(ResidencyEvent::MemoryPressure(MemoryPressure::Moderate));
    let pressured = residency.stats();
    assert!(pressured.cpu.resident_bytes <= pool.low_watermark_bytes);

    residency.insert(ResourceDomain::Cpu, image(10_000), vec![1; 8], 8, 8);
    assert!(residency.contains(ResourceDomain::Cpu, image(10_000)));
    assert!(residency.stats().cpu.rebuild_cost > stressed.cpu.rebuild_cost);
}
