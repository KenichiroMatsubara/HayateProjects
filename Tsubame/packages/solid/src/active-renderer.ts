import type { IRenderer } from '@torimi/tsubame-renderer-protocol';

/**
 * tsubame-solid のコンパイル済み JSX は固定モジュール（このパッケージ）から
 * `createElement` / `insert` 等を import する。そのため対象の {@link IRenderer}
 * はモジュールレベルの可変ホルダーで保持する。
 *
 * これにより「Renderer だけ差し替える」（DOM ↔ Canvas）を、ホルダーの
 * 差し替え＋再レンダリングで実現できる（T6 デモの訴求点）。
 */
let active: IRenderer | null = null;

/** 以降の描画に使う Renderer を設定する。 */
export function setActiveRenderer(renderer: IRenderer): void {
  active = renderer;
}

/** 現在の Renderer を取得する。未設定なら例外。 */
export function activeRenderer(): IRenderer {
  if (active === null) {
    throw new Error(
      'tsubame-solid: アクティブな Renderer が未設定です。renderTsubame() を使うか setActiveRenderer() を先に呼んでください。',
    );
  }
  return active;
}
