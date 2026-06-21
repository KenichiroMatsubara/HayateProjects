import { createElement, insertNode, renderTsubame, setProp, type TsubameNode } from '@tsubame/solid';
import { CanvasRenderer } from '../canvas-renderer.js';
import { captureGoldenFrame, type GoldenFrame } from '../golden-frame.js';
import { createNullHayate, type WasmHayateFixture } from './wasm-hayate.js';
import { manualScheduler } from './manual-scheduler.js';

export interface GoldenFrameParityHarness {
  readonly fixture: WasmHayateFixture;
  readonly renderer: CanvasRenderer;
  readonly tick: (ms?: number) => void;
  capture(): GoldenFrame;
  dispose(): void;
}

/** パリティサンプルを tsubame-solid → CanvasRenderer → WASM 経由でマウントする（ADR-0079）。 */
export async function mountGoldenFrameParity(
  build: (tools: {
    createElement: typeof createElement;
    insertNode: typeof insertNode;
    setProp: typeof setProp;
    setText: (node: TsubameNode, value: string) => void;
  }) => TsubameNode,
  options?: { width?: number; height?: number },
): Promise<GoldenFrameParityHarness> {
  const fixture = await createNullHayate(options?.width ?? 200, options?.height ?? 100);
  const sched = manualScheduler();
  const renderer = new CanvasRenderer(fixture.raw, {
    ...sched,
    canvas: fixture.canvas,
  });

  const setText = (node: TsubameNode, value: string): void => {
    renderer.setText(node.id, value);
  };

  const disposeRender = renderTsubame(
    () => build({ createElement, insertNode, setProp, setText }),
    renderer,
  );

  sched.tick(16);

  return {
    fixture,
    renderer,
    tick: (ms = 16) => sched.tick(ms),
    capture: () => captureGoldenFrame(fixture.raw, 1, null),
    dispose: () => {
      disposeRender();
      fixture.dispose();
    },
  };
}

/** ローカルまたは合成テキストが `snippet` を含む最初の要素を探す。 */
export function findElementByText(frame: GoldenFrame, snippet: string) {
  return frame.elements.find(
    (el) => el.text.includes(snippet) || el.textContent.includes(snippet),
  );
}
