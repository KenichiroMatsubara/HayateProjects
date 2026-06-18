import type {
  ElementId,
  ElementKind,
  PseudoStyleKey,
  StylePatch,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { encodeMutations, type SemanticMutation } from './encode-mutations.js';

/**
 * Ordered Hayate Mutation Packet queue for the CanvasRenderer → Hayate WASM boundary.
 *
 * This is the B-lite form of the packet: it preserves semantic operation order and
 * emits the low-level op/style/text buffers only at boundary flush time. It deliberately
 * does not merge, prune, coalesce, or otherwise optimise queued semantic mutations.
 *
 * The wire-format encoding lives entirely in the pure {@link encodeMutations}
 * (issue #237); this class is just the ordered buffer plus the single boundary call.
 */
export class HayateMutationPacket {
  private readonly mutations: SemanticMutation[] = [];

  enqueueCreateElement(id: ElementId, kind: ElementKind): void {
    this.mutations.push({ kind: 'createElement', id, elementKind: kind });
  }

  enqueueSetRoot(id: ElementId): void {
    this.mutations.push({ kind: 'setRoot', id });
  }

  enqueueAppendChild(parent: ElementId, child: ElementId): void {
    this.mutations.push({ kind: 'appendChild', parent, child });
  }

  enqueueInsertBefore(
    parent: ElementId,
    child: ElementId,
    before: ElementId,
  ): void {
    this.mutations.push({ kind: 'insertBefore', parent, child, before });
  }

  enqueueRemove(id: ElementId): void {
    this.mutations.push({ kind: 'remove', id });
  }

  enqueueSetStyle(id: ElementId, style: StylePatch): void {
    this.mutations.push({ kind: 'setStyle', id, style: { ...style } });
  }

  enqueueSetText(id: ElementId, text: string): void {
    this.mutations.push({ kind: 'setText', id, text });
  }

  enqueueSetTextContent(id: ElementId, text: string): void {
    this.mutations.push({ kind: 'setTextContent', id, text });
  }

  enqueueSetDisabled(id: ElementId, disabled: boolean): void {
    this.mutations.push({ kind: 'setDisabled', id, disabled });
  }

  enqueueSetSelectable(id: ElementId, selectable: boolean): void {
    this.mutations.push({ kind: 'setSelectable', id, selectable });
  }

  enqueueSetMultiline(id: ElementId, multiline: boolean): void {
    this.mutations.push({ kind: 'setMultiline', id, multiline });
  }

  enqueueSetSrc(id: ElementId, url: string): void {
    this.mutations.push({ kind: 'setSrc', id, url });
  }

  enqueueSetPseudoStyle(
    id: ElementId,
    pseudo: PseudoStyleKey,
    style: StylePatch,
  ): void {
    this.mutations.push({
      kind: 'setPseudoStyle',
      id,
      pseudo,
      style: { ...style },
    });
  }

  enqueueSetStyleVariant(
    id: ElementId,
    condition: ViewportCondition,
    style: StylePatch,
  ): void {
    this.mutations.push({
      kind: 'setStyleVariant',
      id,
      condition,
      style: { ...style },
    });
  }

  flush(raw: RawHayate): void {
    if (this.mutations.length === 0) return;
    const { ops, styles, texts } = encodeMutations(this.mutations);
    if (ops.length > 0) {
      raw.apply_mutations(ops, styles, texts);
    }
    this.mutations.length = 0;
  }
}
