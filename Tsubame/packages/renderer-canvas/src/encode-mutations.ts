import type {
  ElementId,
  ElementKind,
  PseudoStyleKey,
  StylePatch,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import { PSEUDO_STATE_CODE } from '@tsubame/renderer-protocol';
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

/**
 * One ordered semantic operation queued for the CanvasRenderer → Hayate WASM
 * boundary. The packet buffers these; {@link encodeMutations} turns them into
 * the low-level op/style/text wire buffers (ADR-0052).
 */
export type SemanticMutation =
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

/** The wire buffers `apply_mutations` consumes (ADR-0052). */
export interface EncodedMutations {
  readonly ops: Float64Array;
  readonly styles: Float32Array;
  readonly texts: string[];
}

/** ADR-0081: an unset viewport-condition axis is encoded as -1 on the wire. */
export function viewportAxis(value: number | undefined): number {
  return value === undefined ? -1 : value;
}

/**
 * ADR-0081: OP_SET_STYLE_VARIANT carries exactly one style property, so a
 * multi-property patch is split into one single-property patch per defined key.
 * Undefined entries are dropped; declaration order is preserved.
 */
export function splitStyleVariant(style: StylePatch): StylePatch[] {
  const split: StylePatch[] = [];
  for (const key in style) {
    const k = key as keyof StylePatch;
    if (style[k] === undefined) continue;
    split.push({ [k]: style[k] } as StylePatch);
  }
  return split;
}

/**
 * Pure wire-format producer: a semantic mutation list in, three typed arrays
 * out. This is the single place the op/style/text encoding lives, so it can be
 * exercised directly without crossing the WASM boundary (issue #237).
 */
export function encodeMutations(
  mutations: readonly SemanticMutation[],
): EncodedMutations {
  const ops: number[] = [];
  const styles: number[] = [];
  const texts: string[] = [];

  for (const mutation of mutations) {
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
        appendChild(ops, mutation.parent as number, mutation.child as number);
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
        appendSetDisabled(
          ops,
          mutation.id as number,
          mutation.disabled ? 1 : 0,
        );
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
        for (const single of splitStyleVariant(mutation.style)) {
          const offset = styles.length;
          encodeStylePatch(single, styles);
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

  return {
    ops: new Float64Array(ops),
    styles: new Float32Array(styles),
    texts,
  };
}
