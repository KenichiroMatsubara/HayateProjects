import initWasm, { HayateElementRenderer } from 'hayate-adapter-web';
import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import type { HayateWasm } from './hayate.js';

/**
 * Hayate WASM を初期化し CanvasRenderer を返す。
 * アプリ側はこの関数を呼ぶだけでよく、WASM ロードや HayateElementRenderer の
 * ライフサイクルを意識する必要がない。
 */
export async function initCanvasRenderer(
  canvas: HTMLCanvasElement,
  options?: CanvasRendererOptions,
): Promise<CanvasRenderer> {
  await initWasm();
  const raw = await HayateElementRenderer.init(canvas) as unknown as Record<string, unknown>;
  const hayate: HayateWasm = {
    element_create: (id, kind) => (raw.element_create as (id: number, kind: string) => void)(id, kind),
    set_root: (id) => (raw.set_root as (id: number) => void)(id),
    element_append_child: (parent, child) =>
      (raw.element_append_child as (parent: number, child: number) => void)(parent, child),
    element_insert_before: (parent, child, before) =>
      (raw.element_insert_before as (
        parent: number,
        child: number,
        before: number,
      ) => void)(parent, child, before),
    element_remove: (id) => (raw.element_remove as (id: number) => void)(id),
    element_set_style: (id, props) =>
      (raw.element_set_style as (id: number, props: unknown[]) => void)(id, props),
    element_unset_style: (id, kinds) =>
      (raw.element_unset_style as (id: number, kinds: string[]) => void)(id, kinds),
    element_set_text: (id, text) =>
      (raw.element_set_text as (id: number, text: string) => void)(id, text),
    on_resize: (width, height) =>
      (raw.on_resize as (width: number, height: number) => void)(width, height),
    render: (timestampMs) =>
      (raw.render as (timestampMs: number) => void)(timestampMs),
    poll_events: () =>
      (raw.poll_events as () => ReturnType<HayateWasm['poll_events']>)(),
  };
  return new CanvasRenderer(hayate, options);
}
