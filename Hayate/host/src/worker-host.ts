/**
 * Web の OffscreenCanvas＋単一 Worker ホスト（ADR-0128 の Web 近似形）。
 *
 * native の UI/Raster 内部二分割（ADR-0128）を Web で再現するには SharedArrayBuffer（COOP/COEP）が
 * 必須で、ADR-0003 が却下した非実践パス。Web ではこれを真似ず、**エンジン丸ごと**（WASM core＋
 * Render Host＋selected Scene Renderer＋common frame pipeline）を **OffscreenCanvas＋単一 Worker** に
 * 載せ、main スレッドは transport shim にする。COOP/COEP 不要
 * （SharedArrayBuffer 非依存）。Web では SceneGraph はスレッドを跨がない（Worker 内に core も raster も
 * 同居）。**IME(EditContext) は main 結合（ADR-0069）なので main↔Worker の IME ブリッジ**が Web 固有税。
 *
 * 本モジュールは main↔Worker のメッセージ契約・main shim・Worker dispatcher・IME ブリッジを純粋に
 * 定義し、transport（postMessage）を注入 seam にしてホストでテストする。実 OffscreenCanvas/WASM/GPU は
 * ブラウザ実行時に差さる。
 */

/** OffscreenCanvas のハンドル。実環境では transfer される `OffscreenCanvas`、テストではトークン。 */
export type CanvasHandle = unknown;
export type WorkerPointerKind = 'mouse' | 'touch' | 'pen';

/** Native と同じ意味で共通 Pipeline が公開する overload / terminal-state counters. */
export interface FramePipelineObservation {
  accepted: number;
  coalesced: number;
  dropped: number;
  active: boolean;
  pending: number;
  failure: boolean;
}

/** Worker 内の WASM core に適用する structured-clone-safe mutation command。 */
export type WorkerMutation =
  | { kind: 'element-create'; id: number; elementKind: number }
  | { kind: 'set-root'; id: number }
  | { kind: 'append-child'; parent: number; child: number }
  | { kind: 'insert-before'; parent: number; child: number; before: number }
  | { kind: 'remove'; id: number }
  | {
      kind: 'apply-mutations';
      ops: Float64Array;
      styles: Float32Array;
      texts: string[];
      draws: Float32Array;
    }
  | { kind: 'background'; r: number; g: number; b: number };

/** main → Worker メッセージ（DOM/pointer/IME 入力の橋渡し）。 */
export type MainToWorker =
  | { kind: 'init'; canvas: CanvasHandle; width: number; height: number; dpr: number }
  | { kind: 'resize'; width: number; height: number; dpr: number }
  | {
      kind: 'pointer';
      action: 'down' | 'move' | 'up';
      pointerKind: WorkerPointerKind;
      x: number;
      y: number;
    }
  | { kind: 'wheel'; x: number; y: number; deltaX: number; deltaY: number }
  | { kind: 'key'; key: string; modifiers: number }
  | { kind: 'edit-intent'; targetId: number; intent: number[] }
  | { kind: 'composition'; targetId: number; text: string }
  | { kind: 'mutation'; command: WorkerMutation }
  | { kind: 'frame'; timestampMs: number }
  | { kind: 'observe-pipeline'; requestId: number }
  | { kind: 'detach' };

/** レイアウト後の IME presentation（ADR-0069）。Worker が決め、main の EditContext へ橋渡しする。 */
export interface ImePresentation {
  /** ソフトキーボードの表示可否。 */
  keyboardVisible: boolean;
  /** 候補ウィンドウ境界（CSS px）。`null` は更新なし。 */
  caretRect: { x: number; y: number; width: number; height: number } | null;
}

/** Worker → main メッセージ。Web 固有税の IME presentation を main へ戻す。 */
export type WorkerToMain =
  | { kind: 'ready' }
  | {
      kind: 'boot-failure';
      failure: { code: 'renderer-init-failed'; message: string };
    }
  | {
      kind: 'runtime-failure';
      failure: { code: 'renderer-runtime-failed'; message: string };
    }
  | { kind: 'ime'; presentation: ImePresentation }
  | {
      kind: 'pipeline-observation';
      requestId: number;
      observation: FramePipelineObservation;
    }
  | { kind: 'detached' };

/**
 * Worker 内のエンジン（WASM コア＋raster＋compositor）。main からのメッセージで駆動される最小面。
 * 描画（`render`）は Worker 上で走り、main/DOM スレッドをブロックしない。
 */
export interface WorkerEngine {
  init(canvas: CanvasHandle, width: number, height: number, dpr: number): void | Promise<void>;
  resize(width: number, height: number, dpr: number): void;
  onPointer(
    action: 'down' | 'move' | 'up',
    x: number,
    y: number,
    pointerKind?: WorkerPointerKind,
  ): void;
  onWheel(x: number, y: number, deltaX: number, deltaY: number): void;
  onKey(key: string, modifiers: number): void;
  dispatchEditIntent?(targetId: number, intent: Float64Array): number;
  onComposition(targetId: number, text: string): void;
  applyMutation?(command: WorkerMutation): void;
  render?(timestampMs: number): void;
  detach?(): void | Promise<void>;
  pipelineObservation(): FramePipelineObservation;
  /** レイアウト後の IME presentation（ADR-0069）。main の EditContext へブリッジする。 */
  imePresentation(): ImePresentation;
}

/**
 * Worker 側ディスパッチャ。main からの [`MainToWorker`] を受けてエンジンを駆動し、IME presentation を
 * main へ戻す。エンジン（core＋raster）は Worker 内に同居し、SceneGraph はスレッドを跨がない。
 */
export class WorkerEngineDispatcher {
  constructor(
    private readonly engine: WorkerEngine,
    private readonly postToMain: (msg: WorkerToMain) => void,
  ) {}

  /** main から届いた 1 メッセージを処理する。 */
  async handle(msg: MainToWorker): Promise<void> {
    try {
      switch (msg.kind) {
        case 'init':
          try {
            await this.engine.init(msg.canvas, msg.width, msg.height, msg.dpr);
            this.postToMain({ kind: 'ready' });
          } catch (error) {
            this.postToMain({
              kind: 'boot-failure',
              failure: {
                code: 'renderer-init-failed',
                message: error instanceof Error ? error.message : String(error),
              },
            });
          }
          break;
        case 'resize':
          this.engine.resize(msg.width, msg.height, msg.dpr);
          break;
        case 'pointer':
          this.engine.onPointer(msg.action, msg.x, msg.y, msg.pointerKind);
          this.emitIme();
          break;
        case 'wheel':
          this.engine.onWheel(msg.x, msg.y, msg.deltaX, msg.deltaY);
          break;
        case 'key':
          this.engine.onKey(msg.key, msg.modifiers);
          this.emitIme();
          break;
        case 'edit-intent':
          this.engine.dispatchEditIntent?.(msg.targetId, new Float64Array(msg.intent));
          this.emitIme();
          break;
        case 'composition':
          this.engine.onComposition(msg.targetId, msg.text);
          this.emitIme();
          break;
        case 'mutation':
          this.engine.applyMutation?.(msg.command);
          break;
        case 'frame':
          this.engine.render?.(msg.timestampMs);
          this.emitIme();
          break;
        case 'observe-pipeline':
          this.postToMain({
            kind: 'pipeline-observation',
            requestId: msg.requestId,
            observation: this.engine.pipelineObservation(),
          });
          break;
        case 'detach':
          await this.engine.detach?.();
          this.postToMain({ kind: 'detached' });
          break;
      }
    } catch (error) {
      this.postToMain({
        kind: 'runtime-failure',
        failure: {
          code: 'renderer-runtime-failed',
          message: error instanceof Error ? error.message : String(error),
        },
      });
    }
  }

  /** フォーカス/編集を動かし得た入力の後に、最新の IME presentation を main へ橋渡しする。 */
  private emitIme(): void {
    this.postToMain({ kind: 'ime', presentation: this.engine.imePresentation() });
  }
}

/** main の EditContext 面（ADR-0069）。Worker から来た IME presentation を適用する。 */
export interface MainEditContextSink {
  setKeyboardVisible(visible: boolean): void;
  setCaretRect(rect: { x: number; y: number; width: number; height: number } | null): void;
}

/**
 * main スレッドの薄い shim。DOM/pointer/IME を [`MainToWorker`] にして Worker へ postMessage し、
 * Worker からの IME presentation を main の EditContext へ適用するだけ。**エンジン参照を持たない**＝
 * 描画は Worker 上で走り、main/DOM スレッドは描画でブロックされない。SharedArrayBuffer は使わない。
 */
export class MainThreadShim {
  private detached = false;
  private observationRequestId = 0;
  private readonly observationRequests = new Map<
    number,
    (observation: FramePipelineObservation) => void
  >();

  constructor(
    private readonly sendToWorker: (msg: MainToWorker, transfer?: Transferable[]) => void,
    private readonly ime: MainEditContextSink,
  ) {}

  /** OffscreenCanvas を Worker へ transfer して初期化する（COOP/COEP 不要）。 */
  init(canvas: CanvasHandle, width: number, height: number, dpr: number): void {
    // 実環境では canvas（OffscreenCanvas）を transfer リストで渡す。テストでは transport が無視する。
    this.post({ kind: 'init', canvas, width, height, dpr }, [canvas as Transferable]);
  }

  resize(width: number, height: number, dpr: number): void {
    this.post({ kind: 'resize', width, height, dpr });
  }

  pointer(
    action: 'down' | 'move' | 'up',
    x: number,
    y: number,
    pointerKind: WorkerPointerKind = 'mouse',
  ): void {
    this.post({ kind: 'pointer', action, pointerKind, x, y });
  }

  wheel(x: number, y: number, deltaX: number, deltaY: number): void {
    this.post({ kind: 'wheel', x, y, deltaX, deltaY });
  }

  key(key: string, modifiers: number): void {
    this.post({ kind: 'key', key, modifiers });
  }

  editIntent(targetId: number, intent: Float64Array): void {
    this.post({ kind: 'edit-intent', targetId, intent: Array.from(intent) });
  }

  composition(targetId: number, text: string): void {
    this.post({ kind: 'composition', targetId, text });
  }

  mutation(command: WorkerMutation): void {
    this.post({ kind: 'mutation', command });
  }

  frame(timestampMs: number): void {
    this.post({ kind: 'frame', timestampMs });
  }

  pipelineObservation(): Promise<FramePipelineObservation> {
    if (this.detached) {
      return Promise.reject(new Error('worker engine is detached'));
    }
    const requestId = ++this.observationRequestId;
    return new Promise((resolve) => {
      this.observationRequests.set(requestId, resolve);
      this.post({ kind: 'observe-pipeline', requestId });
    });
  }

  detach(): void {
    if (this.detached) return;
    this.sendToWorker({ kind: 'detach' });
    this.detached = true;
  }

  /** Worker からのメッセージを処理する。IME presentation を main の EditContext へ適用する。 */
  handleWorkerMessage(msg: WorkerToMain): void {
    if (msg.kind === 'ime') {
      this.ime.setKeyboardVisible(msg.presentation.keyboardVisible);
      this.ime.setCaretRect(msg.presentation.caretRect);
    } else if (msg.kind === 'pipeline-observation') {
      const resolve = this.observationRequests.get(msg.requestId);
      this.observationRequests.delete(msg.requestId);
      resolve?.(msg.observation);
    }
  }

  private post(msg: MainToWorker, transfer?: Transferable[]): void {
    if (!this.detached) this.sendToWorker(msg, transfer);
  }
}
