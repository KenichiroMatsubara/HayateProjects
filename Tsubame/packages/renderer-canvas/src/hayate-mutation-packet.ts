import type {
  ElementId,
  ElementKind,
  PseudoStyleKey,
  StylePatch,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import { PSEUDO_STATE_CODE } from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { ELEMENT_KIND } from '@tsubame/protocol-generated/protocol';
import {
  appendCreate,
  appendSetRoot,
  appendChild,
  insertBefore,
  appendRemove,
  appendSetStyle,
  appendSetText,
  appendSetTextContent,
  appendSetDisabled,
  appendSetSrc,
  appendSetPseudoStyle,
  appendSetStyleVariant,
  appendUnsetStyle,
  encodeStylePatch,
  unsetKindsOf,
} from '@tsubame/protocol-generated/codec';

/** ADR-0081: an unset viewport-condition axis is encoded as -1 on the wire. */
function viewportAxis(value: number | undefined): number {
  return value === undefined ? -1 : value;
}

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
  | { readonly kind: 'setText'; readonly id: ElementId; readonly text: string }
  | {
      readonly kind: 'setTextContent';
      readonly id: ElementId;
      readonly text: string;
    }
  | {
      readonly kind: 'setDisabled';
      readonly id: ElementId;
      readonly disabled: boolean;
    }
  | { readonly kind: 'setSrc'; readonly id: ElementId; readonly url: string }
  | {
      readonly kind: 'setPseudoStyle';
      readonly id: ElementId;
      readonly pseudo: PseudoStyleKey;
      readonly style: StylePatch;
    }
  | {
      readonly kind: 'setStyleVariant';
      readonly id: ElementId;
      readonly condition: ViewportCondition;
      readonly style: StylePatch;
    };

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

  enqueueSetTextContent(id: ElementId, text: string): void {
    this.mutations.push({ kind: 'setTextContent', id, text });
  }

  enqueueSetDisabled(id: ElementId, disabled: boolean): void {
    this.mutations.push({ kind: 'setDisabled', id, disabled });
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

    const ops: number[] = [];
    const styles: number[] = [];
    const texts: string[] = [];

    for (const mutation of this.mutations) {
      switch (mutation.kind) {
        case 'createElement':
          appendCreate(
            ops,
            mutation.id as number,
            (ELEMENT_KIND as Record<string, number>)[mutation.elementKind]!,
          );
          break;
        case 'setRoot':
          appendSetRoot(ops, mutation.id as number);
          break;
        case 'appendChild':
          appendChild(
            ops,
            mutation.parent as number,
            mutation.child as number,
          );
          break;
        case 'insertBefore':
          insertBefore(
            ops,
            mutation.parent as number,
            mutation.child as number,
            mutation.before as number,
          );
          break;
        case 'remove':
          appendRemove(ops, mutation.id as number);
          break;
        case 'setStyle': {
          const offset = styles.length;
          encodeStylePatch(mutation.style, styles);
          const len = styles.length - offset;
          if (len > 0) {
            appendSetStyle(ops, mutation.id as number, offset, len);
          }
          for (const unsetKind of unsetKindsOf(mutation.style)) {
            appendUnsetStyle(ops, mutation.id as number, unsetKind);
          }
          break;
        }
        case 'setText': {
          const textIndex = texts.length;
          texts.push(mutation.text);
          appendSetText(ops, mutation.id as number, textIndex);
          break;
        }
        case 'setTextContent': {
          const textIndex = texts.length;
          texts.push(mutation.text);
          appendSetTextContent(ops, mutation.id as number, textIndex);
          break;
        }
        case 'setDisabled':
          appendSetDisabled(ops, mutation.id as number, mutation.disabled ? 1 : 0);
          break;
        case 'setSrc': {
          const textIndex = texts.length;
          texts.push(mutation.url);
          appendSetSrc(ops, mutation.id as number, textIndex);
          break;
        }
        case 'setPseudoStyle': {
          const offset = styles.length;
          encodeStylePatch(mutation.style, styles);
          const len = styles.length - offset;
          if (len > 0) {
            appendSetPseudoStyle(
              ops,
              mutation.id as number,
              PSEUDO_STATE_CODE[mutation.pseudo],
              offset,
              len,
            );
          }
          break;
        }
        case 'setStyleVariant': {
          // OP_SET_STYLE_VARIANT carries exactly one style property (ADR-0081),
          // so a multi-property patch is split into one op per property.
          for (const key in mutation.style) {
            const k = key as keyof StylePatch;
            if (mutation.style[k] === undefined) continue;
            const offset = styles.length;
            encodeStylePatch({ [k]: mutation.style[k] } as StylePatch, styles);
            const len = styles.length - offset;
            if (len > 0) {
              appendSetStyleVariant(
                ops,
                mutation.id as number,
                viewportAxis(mutation.condition.minWidth),
                viewportAxis(mutation.condition.maxWidth),
                viewportAxis(mutation.condition.minHeight),
                viewportAxis(mutation.condition.maxHeight),
                offset,
                len,
              );
            }
          }
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
