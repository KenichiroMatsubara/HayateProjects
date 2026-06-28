import type { RawHayate } from './raw-hayate.js';

export type { RawHayate } from './raw-hayate.js';

/**
 * {@link attachAccessibilityMirror} の後始末関数。ミラー root を DOM から除去し rAF ループを
 * 止める。host のライフサイクル teardown（full reload）から呼ぶ（ADR-0124）。
 */
export type DetachAccessibilityMirror = () => void;

/**
 * Web Canvas Accessibility Mirror（ADR-0124）の attach 点。`<canvas>` 兄弟に読み取り専用の
 * 不可視 ARIA DOM を建て、自前 rAF ループで `raw.poll_accessibility()`（AccessKit `TreeUpdate`
 * の JSON）を投影する。返り値は detach（root 除去・rAF 停止）で、full reload で呼ばれる。
 *
 * このシームは `createHayateWebHost` が canvas boot のたびに 1 箇所で呼ぶ。標準アプリ（`main.tsx`
 * の直 boot）も Miharashi dev ホスト（`bootMiharashiHost`）も `createHayateWebHost` を通るため、
 * **全 Canvas アプリがここを 1 回通る**。投影器をこの関数本体に実装すれば、host-boot 毎の配線
 * なしに全アプリへ自動で効く（ADR-0124「全 Canvas アプリが host boot 経由で自動的に」）。
 *
 * **#591（prefactor）時点では本体は no-op**：attach/detach の seam を canvas boot 経路に据える
 * だけで、ARIA 投影ロジック（rAF・`TreeUpdate` → ARIA DOM の 1:1 投影・不変スキップ）は後続
 * スライス #592 がこの関数本体に挿す。seam の据え付けと投影実装を分けることで、配線の回帰を
 * 投影ロジックと独立に確かめられる。
 */
export function attachAccessibilityMirror(
  raw: RawHayate,
  canvas: HTMLCanvasElement,
): DetachAccessibilityMirror {
  // #592 がここに rAF ループ + `TreeUpdate` → 不可視 ARIA DOM の投影を実装する。
  // prefactor では何もせず、no-op の detach を返すだけ（既存 e2e / unit を緑のまま保つ）。
  void raw;
  void canvas;
  return () => {};
}
