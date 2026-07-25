import { describe, expect, it, vi } from 'vitest';
import { WasmWorkerEngine, type WorkerRawHayate } from './worker-engine.js';

const TOUCH_SCROLL_STRESS_FRAMES = 50;

describe('WasmWorkerEngine production path', () => {
  it('preserves Worker WASM listener ids and event delivery rows', async () => {
    const raw = {
      register_listener: (elementId: number, eventKind: number) => {
        expect({ elementId, eventKind }).toEqual({ elementId: 7, eventKind: 0 });
        return 73;
      },
      poll_events: () => [[73, 0, 7, 12, 18]],
    } as unknown as WorkerRawHayate;
    const engine = new WasmWorkerEngine(async () => raw);
    await engine.init({}, 100, 100, 1);

    expect(engine.registerListener(7, 0)).toBe(73);
    expect(engine.pollEvents()).toEqual([[73, 0, 7, 12, 18]]);
  });

  it('releases a Worker WASM listener by its opaque id', async () => {
    const unregister = vi.fn();
    const raw = {
      unregister_listener: unregister,
    } as unknown as WorkerRawHayate;
    const engine = new WasmWorkerEngine(async () => raw);
    await engine.init({}, 100, 100, 1);

    engine.unregisterListener(73);

    expect(unregister).toHaveBeenCalledOnce();
    expect(unregister).toHaveBeenCalledWith(73);
  });

  it('presents a committed scene and presents the changed frame after one pointer input', async () => {
    const calls: string[] = [];
    const raw: WorkerRawHayate = {
      element_create: (id, kind) => calls.push(`create(${id},${kind})`),
      set_root: (id) => calls.push(`root(${id})`),
      element_append_child: () => {},
      element_insert_before: () => {},
      element_remove: () => {},
      apply_mutations: () => {},
      set_background_color: () => {},
      set_tuning: () => {},
      register_listener: () => 1,
      unregister_listener: () => {},
      poll_events: () => [],
      render: (timestamp) => {
        calls.push(`present(${timestamp})`);
        return undefined;
      },
      resize_surface: () => {},
      complete_active: () => undefined,
      fail_active: () => {},
      pipeline_observation: () => new Float64Array(),
      is_detached: () => false,
      on_pointer_down: (x, y) => calls.push(`pointer(down,${x},${y})`),
      on_pointer_move: () => {},
      on_pointer_up: () => {},
      on_pointer_down_with_kind: (x, y) => calls.push(`pointer(down,${x},${y})`),
      on_pointer_move_with_kind: () => {},
      on_pointer_up_with_kind: () => {},
      on_wheel: () => {},
      on_key_down: () => {},
      dispatch_edit_intent: () => 0,
      on_text_input: () => {},
      ime_wants_keyboard: () => false,
      ime_character_bounds: () => new Float32Array([0, 0, 0, 0]),
      detach: () => {
        calls.push('detach');
        return undefined;
      },
    };
    const load = vi.fn(async () => raw);
    const engine = new WasmWorkerEngine(load, () => 20);

    await engine.init({ token: 'offscreen' }, 800, 600, 2);
    engine.applyMutation({ kind: 'element-create', id: 1, elementKind: 0 });
    engine.applyMutation({ kind: 'set-root', id: 1 });
    engine.render(10);
    engine.onPointer('down', 4, 5);

    expect(load).toHaveBeenCalledWith({ token: 'offscreen' }, 800, 600, 2);
    expect(calls).toEqual([
      'create(1,0)',
      'root(1)',
      'present(10)',
      'pointer(down,4,5)',
      'present(20)',
    ]);
  });

  it('waits for GPU completion before driving the one coalesced pending touch-scroll frame', async () => {
    let resolveFirst!: () => void;
    let resolveSecond!: () => void;
    const first = new Promise<void>((resolve) => {
      resolveFirst = resolve;
    });
    const second = new Promise<void>((resolve) => {
      resolveSecond = resolve;
    });
    const admitted: number[] = [];
    let started = 0;
    let completions = 0;
    const raw = {
      element_create: () => {},
      set_root: () => {},
      element_append_child: () => {},
      element_insert_before: () => {},
      element_remove: () => {},
      apply_mutations: () => {},
      set_background_color: () => {},
      set_tuning: () => {},
      register_listener: () => 1,
      unregister_listener: () => {},
      poll_events: () => [],
      render: (timestamp: number) => {
        admitted.push(timestamp);
        if (started++ === 0) return first;
        return undefined;
      },
      complete_active: () => {
        completions += 1;
        return completions === 1 ? second : undefined;
      },
      fail_active: () => {},
      pipeline_observation: () => new Float64Array([50, 48, 0, 1, 1, 0]),
      is_detached: () => false,
      resize_surface: () => undefined,
      on_pointer_down: () => {},
      on_pointer_move: () => {},
      on_pointer_up: () => {},
      on_pointer_down_with_kind: () => {},
      on_pointer_move_with_kind: () => {},
      on_pointer_up_with_kind: () => {},
      on_wheel: () => {},
      on_key_down: () => {},
      dispatch_edit_intent: () => 0,
      on_text_input: () => {},
      ime_wants_keyboard: () => false,
      ime_character_bounds: () => new Float32Array(),
      detach: () => undefined,
    } as WorkerRawHayate & {
      complete_active(): Promise<void> | undefined;
      fail_active(message: string): void;
    };
    let now = 0;
    const engine = new WasmWorkerEngine(async () => raw, () => ++now);
    await engine.init({}, 100, 100, 1);

    for (let index = 0; index < TOUCH_SCROLL_STRESS_FRAMES; index += 1) {
      engine.onPointer('move', 0, index, 'touch');
    }
    expect(admitted).toHaveLength(TOUCH_SCROLL_STRESS_FRAMES);
    expect(completions).toBe(0);

    resolveFirst();
    await Promise.resolve();
    await Promise.resolve();
    expect(completions).toBe(1);

    resolveSecond();
    await Promise.resolve();
    await Promise.resolve();
    expect(completions).toBe(2);
  });

  it('latches a rejected renderer completion without retrying or restarting the Worker', async () => {
    const completion = Promise.reject(new Error('WebGPU context lost'));
    let failedWith: string | undefined;
    let restarted = 0;
    const raw = {
      element_create: () => {},
      set_root: () => {},
      element_append_child: () => {},
      element_insert_before: () => {},
      element_remove: () => {},
      apply_mutations: () => {},
      set_background_color: () => {},
      set_tuning: () => {},
      register_listener: () => 1,
      unregister_listener: () => {},
      poll_events: () => [],
      render: () => completion,
      complete_active: () => {
        throw new Error('a failed completion must not advance the pipeline');
      },
      fail_active: (message: string) => {
        failedWith = message;
      },
      pipeline_observation: () => new Float64Array([1, 0, 0, 0, 0, 1]),
      is_detached: () => false,
      resize_surface: () => undefined,
      on_pointer_down: () => {},
      on_pointer_move: () => {},
      on_pointer_up: () => {},
      on_pointer_down_with_kind: () => {},
      on_pointer_move_with_kind: () => {},
      on_pointer_up_with_kind: () => {},
      on_wheel: () => {},
      on_key_down: () => {},
      dispatch_edit_intent: () => 0,
      on_text_input: () => {},
      ime_wants_keyboard: () => false,
      ime_character_bounds: () => new Float32Array(),
      detach: () => undefined,
    } satisfies WorkerRawHayate;
    const engine = new WasmWorkerEngine(async () => {
      restarted += 1;
      return raw;
    });
    await engine.init({}, 100, 100, 1);

    engine.render(1);
    await Promise.resolve();
    await Promise.resolve();

    expect(failedWith).toBe('WebGPU context lost');
    expect(restarted).toBe(1);
    expect(engine.pipelineObservation().failure).toBe(true);
  });
});
