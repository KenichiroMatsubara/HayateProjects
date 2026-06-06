import type {
  ElementId,
  ElementKind,
  StylePatch,
} from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { ELEMENT_KIND, OP } from '@tsubame/protocol-generated/protocol';
import { encodeStylePatch, unsetKindsOf } from './style-encoder.js';

type SemanticMutation =
  | {
      readonly kind: 'createElement';
      readonly id: ElementId;
      readonly elementKind: ElementKind;
    }
  | { readonly kind: 'setRoot'; readonly id: ElementId }
  | {
      readonly kind: 'appendChild';
      readonly parent: ElementId;
      readonly child: ElementId;
    }
  | {
      readonly kind: 'insertBefore';
      readonly parent: ElementId;
      readonly child: ElementId;
      readonly before: ElementId;
    }
  | { readonly kind: 'remove'; readonly id: ElementId }
  | {
      readonly kind: 'setStyle';
      readonly id: ElementId;
      readonly style: StylePatch;
    }
  | { readonly kind: 'setText'; readonly id: ElementId; readonly text: string };

/**
 * Ordered Hayate Mutation Packet queue for the CanvasRenderer → Hayate WASM boundary.
 *
 * This is the B-lite form of the packet: it preserves semantic operation order and
 * emits the low-level op/style/text buffers only at boundary flush time. It deliberately
 * does not merge, prune, coalesce, or otherwise optimise queued semantic mutations.
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

  flush(raw: RawHayate): void {
    if (this.mutations.length === 0) return;

    const ops: number[] = [];
    const styles: number[] = [];
    const texts: string[] = [];

    for (const mutation of this.mutations) {
      switch (mutation.kind) {
        case 'createElement':
          ops.push(
            OP.CREATE,
            mutation.id as number,
            (ELEMENT_KIND as Record<string, number>)[mutation.elementKind]!,
          );
          break;
        case 'setRoot':
          ops.push(OP.SET_ROOT, mutation.id as number);
          break;
        case 'appendChild':
          ops.push(
            OP.APPEND_CHILD,
            mutation.parent as number,
            mutation.child as number,
          );
          break;
        case 'insertBefore':
          ops.push(
            OP.INSERT_BEFORE,
            mutation.parent as number,
            mutation.child as number,
            mutation.before as number,
          );
          break;
        case 'remove':
          ops.push(OP.REMOVE, mutation.id as number);
          break;
        case 'setStyle': {
          const offset = styles.length;
          encodeStylePatch(mutation.style, styles);
          const len = styles.length - offset;
          if (len > 0) {
            ops.push(OP.SET_STYLE, mutation.id as number, offset, len);
          }
          for (const unsetKind of unsetKindsOf(mutation.style)) {
            ops.push(OP.UNSET_STYLE, mutation.id as number, unsetKind);
          }
          break;
        }
        case 'setText': {
          const textIndex = texts.length;
          texts.push(mutation.text);
          ops.push(OP.SET_TEXT, mutation.id as number, textIndex);
          break;
        }
      }
    }

    if (ops.length > 0) {
      raw.apply_mutations(new Float64Array(ops), new Float32Array(styles), texts);
    }
    this.mutations.length = 0;
  }
}
