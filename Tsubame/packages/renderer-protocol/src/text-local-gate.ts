import type { ElementKind } from './element.js';
import type { StylePatch } from './style.js';
import { isTextLocal, carriesTextLocal } from './generated/style-channel.js';

/**
 * Style Channel gate (ADR-0065 / ADR-0002, generated from proto/spec via
 * Tsubame ADR-0008): a channel-1 text-local prop only reaches a Text-Local
 * Carrier kind; every non-text-local prop always applies. This is the single
 * rule both renderers consult — the DOM renderer before writing CSS, the Canvas
 * renderer before encoding the wire — so the two can never silently diverge.
 */
export function shouldApplyTextLocalPatch(kind: ElementKind, patchKey: string): boolean {
  if (!isTextLocal(patchKey)) return true;
  return carriesTextLocal(kind);
}

/**
 * Drop the text-local props a `kind` does not carry, preserving declaration
 * order. Carriers (and patches with no gated key) pass through unchanged.
 */
export function gateTextLocalPatch(kind: ElementKind, patch: StylePatch): StylePatch {
  if (carriesTextLocal(kind)) return patch;

  const gated: Record<string, unknown> = {};
  for (const key in patch) {
    if (!shouldApplyTextLocalPatch(kind, key)) continue;
    gated[key] = patch[key as keyof StylePatch];
  }
  return gated as StylePatch;
}
