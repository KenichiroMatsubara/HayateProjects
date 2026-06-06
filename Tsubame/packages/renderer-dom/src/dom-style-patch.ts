import type { HayateStyle, StylePatch } from '@tsubame/renderer-protocol';

type DomStylePatchBase = Omit<StylePatch, 'opacity'>;

/**
 * DOM Renderer style patch with IDE warnings for properties that may diverge
 * from Canvas/Hayate Z-Order semantics.
 */
export type DomStylePatch = DomStylePatchBase & {
  /**
   * @deprecated DOM Renderer may diverge from Canvas/Hayate Z-Order semantics
   * because browser CSS creates an implicit stacking context for opacity.
   * See Tsubame/docs/adr/0006-dom-z-order-rn-web-emulation.md.
   */
  opacity?: HayateStyle['opacity'] | null;
};
