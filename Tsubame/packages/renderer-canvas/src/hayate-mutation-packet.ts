import type {
  ElementId,
  ElementKind,
  StylePatch,
} from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { ELEMENT_KIND, OP } from './protocol.js';
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
      readonly unsetKinds: readonly number[];
    }
  | { readonly kind: 'setText'; readonly id: ElementId; readonly text: string };

/**
 * Ordered Hayate Mutation Packet queue for the CanvasRenderer → Hayate WASM boundary.
 *
 * This is the B-lite form of the packet: it preserves semantic operation order and
 * emits the low-level op/style buffers only at boundary flush time. It deliberately
 * does not merge, prune, coalesce, or otherwise optimise queued semantic mutations.
 */
export class HayateMutationPacket {
  private readonly raw: RawHayate;
  private readonly mutations: SemanticMutation[] = [];
  private readonly ops: number[] = [];
  private readonly styles: number[] = [];

  constructor(raw: RawHayate) {
    this.raw = raw;
  }

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
    const copiedStyle: StylePatch = { ...style };
    const unsetKinds = unsetKindsOf(copiedStyle);
    this.mutations.push({ kind: 'setStyle', id, style: copiedStyle, unsetKinds });
    if (unsetKinds.length > 0) {
      this.flush();
    }
  }

  enqueueSetText(id: ElementId, text: string): void {
    this.mutations.push({ kind: 'setText', id, text });
    this.flush();
  }

  flush(): void {
    if (this.mutations.length === 0) return;
    this.encodePendingMutations();
    this.drainTypedBatch();
  }

  private encodePendingMutations(): void {
    for (const mutation of this.mutations) {
      switch (mutation.kind) {
        case 'createElement':
          this.ops.push(
            OP.CREATE,
            mutation.id as number,
            (ELEMENT_KIND as Record<string, number>)[mutation.elementKind]!,
          );
          break;
        case 'setRoot':
          this.ops.push(OP.SET_ROOT, mutation.id as number);
          break;
        case 'appendChild':
          this.ops.push(
            OP.APPEND_CHILD,
            mutation.parent as number,
            mutation.child as number,
          );
          break;
        case 'insertBefore':
          this.ops.push(
            OP.INSERT_BEFORE,
            mutation.parent as number,
            mutation.child as number,
            mutation.before as number,
          );
          break;
        case 'remove':
          this.ops.push(OP.REMOVE, mutation.id as number);
          break;
        case 'setStyle': {
          const offset = this.styles.length;
          encodeStylePatch(mutation.style, this.styles);
          const len = this.styles.length - offset;
          if (len > 0) {
            this.ops.push(OP.SET_STYLE, mutation.id as number, offset, len);
          }
          if (mutation.unsetKinds.length > 0) {
            this.drainTypedBatch();
            this.raw.element_unset_style(
              mutation.id as number,
              Uint32Array.from(mutation.unsetKinds),
            );
          }
          break;
        }
        case 'setText':
          this.drainTypedBatch();
          this.raw.element_set_text(mutation.id as number, mutation.text);
          break;
      }
    }
    this.mutations.length = 0;
  }

  private drainTypedBatch(): void {
    if (this.ops.length === 0) return;
    this.raw.apply_mutations(new Float64Array(this.ops), new Float32Array(this.styles));
    this.ops.length = 0;
    this.styles.length = 0;
  }
}
