import type {
  CanvasHandle,
  FramePipelineObservation,
  ImePresentation,
  WorkerEngine,
  WorkerMutation,
  WorkerPointerKind,
} from './worker-host.js';

export interface WorkerRawHayate {
  element_create(id: number, kind: number): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  apply_mutations(
    ops: Float64Array,
    styles: Float32Array,
    texts: string[],
    draws: Float32Array,
  ): void;
  set_background_color(r: number, g: number, b: number): void;
  set_tuning(json: string): void;
  render(timestampMs: number): Promise<void> | undefined;
  resize_surface(width: number, height: number, dpr: number): Promise<void> | undefined;
  complete_active(): Promise<void> | undefined;
  fail_active(message: string): void;
  pipeline_observation(): Float64Array;
  is_detached(): boolean;
  on_pointer_down(x: number, y: number): void;
  on_pointer_move(x: number, y: number): void;
  on_pointer_up(x: number, y: number): void;
  on_pointer_down_with_kind(x: number, y: number, kind: number): void;
  on_pointer_move_with_kind(x: number, y: number, kind: number): void;
  on_pointer_up_with_kind(x: number, y: number, kind: number): void;
  on_wheel(x: number, y: number, deltaX: number, deltaY: number): void;
  on_key_down(key: string, modifiers: number): void;
  dispatch_edit_intent(target: number, intent: Float64Array): number;
  on_text_input(target: number, text: string): void;
  ime_wants_keyboard(): boolean;
  ime_character_bounds(): Float32Array;
  detach(): Promise<void> | undefined;
}

export type LoadWorkerWasm = (
  canvas: CanvasHandle,
  width: number,
  height: number,
  dpr: number,
) => Promise<WorkerRawHayate>;

/** Production WASM loader. The Render Host inside Rust chooses the compiled Scene Renderer. */
export async function loadWorkerWasm(
  canvas: CanvasHandle,
  width: number,
  height: number,
  dpr: number,
): Promise<WorkerRawHayate> {
  const module = (await import('@torimi/hayate-adapter-web')) as unknown as {
    default(): Promise<void>;
    HayateWorkerEngine: {
      init(
        canvas: CanvasHandle,
        width: number,
        height: number,
        dpr: number,
      ): Promise<WorkerRawHayate>;
    };
  };
  await module.default();
  return module.HayateWorkerEngine.init(canvas, width, height, dpr);
}

/**
 * Worker-owned execution adapter. It translates transport commands to one WASM engine; admission,
 * dirty merge, lifecycle ordering, and terminal failure stay inside Rust's common pipeline.
 */
export class WasmWorkerEngine implements WorkerEngine {
  private raw: WorkerRawHayate | undefined;

  constructor(
    private readonly load: LoadWorkerWasm = loadWorkerWasm,
    private readonly now: () => number = () => performance.now(),
  ) {}

  async init(canvas: CanvasHandle, width: number, height: number, dpr: number): Promise<void> {
    this.raw = await this.load(canvas, width, height, dpr);
  }

  resize(width: number, height: number, dpr: number): void {
    const raw = this.engine();
    this.drive(raw, raw.resize_surface(width, height, dpr));
  }

  onPointer(
    action: 'down' | 'move' | 'up',
    x: number,
    y: number,
    pointerKind: WorkerPointerKind = 'mouse',
  ): void {
    const raw = this.engine();
    const kind = pointerKind === 'touch' ? 1 : pointerKind === 'pen' ? 2 : 0;
    if (action === 'down') raw.on_pointer_down_with_kind(x, y, kind);
    else if (action === 'move') raw.on_pointer_move_with_kind(x, y, kind);
    else raw.on_pointer_up_with_kind(x, y, kind);
    this.drive(raw, raw.render(this.now()));
  }

  onWheel(x: number, y: number, deltaX: number, deltaY: number): void {
    const raw = this.engine();
    raw.on_wheel(x, y, deltaX, deltaY);
    this.drive(raw, raw.render(this.now()));
  }

  onKey(key: string, modifiers: number): void {
    const raw = this.engine();
    raw.on_key_down(key, modifiers);
    this.drive(raw, raw.render(this.now()));
  }

  dispatchEditIntent(targetId: number, intent: Float64Array): number {
    const raw = this.engine();
    const outcome = raw.dispatch_edit_intent(targetId, intent);
    this.drive(raw, raw.render(this.now()));
    return outcome;
  }

  onComposition(targetId: number, text: string): void {
    const raw = this.engine();
    raw.on_text_input(targetId, text);
    this.drive(raw, raw.render(this.now()));
  }

  applyMutation(command: WorkerMutation): void {
    const raw = this.engine();
    switch (command.kind) {
      case 'element-create':
        raw.element_create(command.id, command.elementKind);
        break;
      case 'set-root':
        raw.set_root(command.id);
        break;
      case 'append-child':
        raw.element_append_child(command.parent, command.child);
        break;
      case 'insert-before':
        raw.element_insert_before(command.parent, command.child, command.before);
        break;
      case 'remove':
        raw.element_remove(command.id);
        break;
      case 'apply-mutations':
        raw.apply_mutations(
          command.ops,
          command.styles,
          command.texts,
          command.draws,
        );
        break;
      case 'background':
        raw.set_background_color(command.r, command.g, command.b);
        break;
      case 'tuning':
        raw.set_tuning(command.json);
        break;
    }
  }

  render(timestampMs: number): void {
    const raw = this.engine();
    this.drive(raw, raw.render(timestampMs));
  }

  imePresentation(): ImePresentation {
    const raw = this.engine();
    const bounds = raw.ime_character_bounds();
    return {
      keyboardVisible: raw.ime_wants_keyboard(),
      caretRect:
        bounds.length >= 4
          ? {
              x: bounds[0] ?? 0,
              y: bounds[1] ?? 0,
              width: bounds[2] ?? 0,
              height: bounds[3] ?? 0,
            }
          : null,
    };
  }

  pipelineObservation(): FramePipelineObservation {
    const values = this.engine().pipeline_observation();
    return {
      accepted: values[0] ?? 0,
      coalesced: values[1] ?? 0,
      dropped: values[2] ?? 0,
      active: (values[3] ?? 0) !== 0,
      pending: values[4] ?? 0,
      failure: (values[5] ?? 0) !== 0,
    };
  }

  async detach(): Promise<void> {
    const raw = this.raw;
    if (!raw) return;
    this.drive(raw, raw.detach());
    if (!raw.is_detached() || this.completionActive) {
      await new Promise<void>((resolve) => this.detachWaiters.push(resolve));
    }
    if (this.raw === raw) this.raw = undefined;
  }

  private completionActive = false;
  private readonly detachWaiters: Array<() => void> = [];

  private drive(raw: WorkerRawHayate, completion: Promise<void> | undefined): void {
    if (!completion) {
      this.resolveDetached(raw);
      return;
    }
    this.completionActive = true;
    void completion.then(
      () => {
        if (this.raw !== raw) return;
        this.completionActive = false;
        this.drive(raw, raw.complete_active());
      },
      (error: unknown) => {
        if (this.raw !== raw) return;
        this.completionActive = false;
        raw.fail_active(error instanceof Error ? error.message : String(error));
        this.resolveDetached(raw);
      },
    );
  }

  private resolveDetached(raw: WorkerRawHayate): void {
    if (this.completionActive || !raw.is_detached()) return;
    for (const resolve of this.detachWaiters.splice(0)) resolve();
  }

  private engine(): WorkerRawHayate {
    if (!this.raw) throw new Error('worker engine is not initialized');
    return this.raw;
  }
}
