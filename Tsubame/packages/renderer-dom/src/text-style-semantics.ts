import type { ElementKind } from '@tsubame/renderer-protocol';
import { isTextLocal, carriesTextLocal } from '@tsubame/renderer-protocol';

/**
 * Style Channel gating (ADR-0065 / ADR-0002), generated from proto/spec:
 * channel-1 text-local props only reach Text-Local Carrier kinds. Non-text-local
 * props always apply.
 */
export function shouldApplyTextLocalPatch(kind: ElementKind, patchKey: string): boolean {
  if (!isTextLocal(patchKey)) return true;
  return carriesTextLocal(kind);
}
