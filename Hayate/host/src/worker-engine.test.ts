import { describe, expect, it, vi } from 'vitest';
import { WasmWorkerEngine, type WorkerRawHayate } from './worker-engine.js';

describe('WasmWorkerEngine production path', () => {
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
      render: (timestamp) => calls.push(`present(${timestamp})`),
      resize_surface: () => {},
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
      detach: () => calls.push('detach'),
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
});
