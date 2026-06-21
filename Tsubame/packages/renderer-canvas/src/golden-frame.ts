import type { HayateEffectiveVisual, RawHayate } from './hayate.js';

/** 単一要素の構造・スタイル・レイアウト状態(ADR-0079)。 */
export interface GoldenFrameElement {
  id: number;
  text: string;
  textContent: string;
  /** `layout_cache` 由来の `[x, y, width, height]`。 */
  bounds: number[];
  /** 解決済みの `Visual`(継承 + pseudo、ADR-0067)、または `null`。 */
  visual: HayateEffectiveVisual | null;
}

/** DOM 空間の矩形(ADR-0069 IME 文字バウンディング)、非フォーカス時は `null`。 */
export interface GoldenFrameImeBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * ドキュメント状態の JSON シリアライズ可能な構造化スナップショット。Shadow Tree →
 * Mutation Packet → `ElementTree` → IME/AccessKit の継ぎ目をまたぐ(ADR-0079)。
 * golden ファイルと `toMatchSnapshot()` で比較する。
 */
export interface GoldenFrame {
  elements: GoldenFrameElement[];
  accessibility: unknown;
  imeBounds: GoldenFrameImeBounds | null;
}

/**
 * `rootId` とその子孫の golden frame を取得する(順序は `element_subtree_ids`、
 * Hayate の `ElementTree` がドキュメント順で返す)。
 */
export function captureGoldenFrame(
  raw: RawHayate,
  rootId: number,
  imeBounds: GoldenFrameImeBounds | null,
): GoldenFrame {
  const ids = Array.from(raw.element_subtree_ids(rootId), Number);

  const elements = ids.map((id) => ({
    id,
    text: raw.element_get_text(id),
    textContent: raw.element_get_text_content(id),
    bounds: Array.from(raw.element_get_bounds(id)),
    visual: raw.element_effective_visual(id),
  }));

  const accessibilityJson = raw.poll_accessibility();
  const accessibility =
    accessibilityJson === null ? null : (JSON.parse(accessibilityJson) as unknown);

  return { elements, accessibility, imeBounds };
}
