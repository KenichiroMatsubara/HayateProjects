import {
  editDispatchOutcomeFromWire,
  encodeEditIntent,
  type EditDispatchOutcome,
  type EditIntent,
} from '@torimi/tsubame-protocol-generated/edit-intent';
import type { RawHayate } from './hayate.js';

/** mutation packet とは独立した semantic input command（#828）。 */
export function dispatchEditIntent(
  raw: RawHayate,
  target: number,
  intent: EditIntent,
): EditDispatchOutcome {
  return editDispatchOutcomeFromWire(raw.dispatch_edit_intent(target, encodeEditIntent(intent)));
}

/** OS raw-key producer が semantic command 未処理時だけ KeyDown へ落とす契約。 */
export function dispatchEditIntentWithKeyFallback(
  raw: RawHayate,
  target: number,
  intent: EditIntent,
  key: string,
  modifiers: number,
): EditDispatchOutcome {
  const outcome = dispatchEditIntent(raw, target, intent);
  if (outcome === 'unhandled') raw.on_key_down(key, modifiers);
  return outcome;
}

export type { EditDispatchOutcome, EditIntent };
