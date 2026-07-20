import type {
  ElementId,
  ElementKind,
  PseudoStyleKey,
  StylePatch,
  UserSelect,
  ViewportCondition,
} from '@torimi/tsubame-renderer-protocol';
import { PSEUDO_STATE_CODE } from '@torimi/tsubame-renderer-protocol';
import { ELEMENT_KIND, USER_SELECT } from '@torimi/tsubame-protocol-generated/protocol';
import {
  appendCreate,
  appendSetRoot,
  appendChild,
  insertBefore,
  appendRemove,
  appendSetDraw,
  appendSetStyle,
  appendSetText,
  appendSetTextContent,
  appendSetDisabled,
  appendSetUserSelect,
  appendSetMultiline,
  appendSetSrc,
  appendSetPseudoStyle,
  appendSetStyleVariant,
  appendUnsetStyle,
  encodeStylePatch,
  unsetKindsOf,
} from '@torimi/tsubame-protocol-generated/codec';

/**
 * HayateRenderer → Hayate WASM 境界へ向けて順序付きでキューされる意味操作1件。
 * HayateRenderer がこれらをバッファし、{@link encodeMutations} が低レベルの
 * op/style/text ワイヤバッファへ変換する（ADR-0052）。
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
  | {
      readonly kind: 'setUserSelect';
      readonly id: ElementId;
      readonly value: UserSelect;
    }
  | {
      readonly kind: 'setMultiline';
      readonly id: ElementId;
      readonly multiline: boolean;
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
    }
  | {
      /** 記録済み draw display list（draw_ops.json の op 列・#724 / ADR-0141）。 */
      readonly kind: 'setDraw';
      readonly id: ElementId;
      readonly list: readonly number[];
    };

/** `apply_mutations` が消費するワイヤバッファ（ADR-0052）。 */
export interface EncodedMutations {
  readonly ops: Float64Array;
  readonly styles: Float32Array;
  readonly texts: string[];
  /** draw display list チャネル（texts と同格・#724 / ADR-0141）。`OP_SET_DRAW` が
   * オフセット/長さで参照する f32 フラットバッファ（op 表は draw_ops.json）。 */
  readonly draws: Float32Array;
}

/** ADR-0081: 未設定のビューポート条件軸はワイヤ上で -1 として符号化する。 */
export function viewportAxis(value: number | undefined): number {
  return value === undefined ? -1 : value;
}

/**
 * ADR-0081: OP_SET_STYLE_VARIANT はスタイルプロパティを1つだけ運ぶので、
 * 複数プロパティのパッチは定義済みキーごとに単一プロパティのパッチへ分割する。
 * undefined のエントリは捨て、宣言順は保持する。
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
 * 純粋なワイヤ形式の生成器。意味的ミューテーション列を入力し、3 本の
 * typed array を出力する。op/style/text 符号化が存在する唯一の場所なので、
 * WASM 境界を越えずに直接テストできる。
 */
export function encodeMutations(
  mutations: readonly SemanticMutation[],
): EncodedMutations {
  const ops: number[] = [];
  const styles: number[] = [];
  const texts: string[] = [];
  const draws: number[] = [];

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
      case 'setDraw': {
        // styles と同じオフセット/長さ参照モデル（texts の index 参照とは異なる）。
        const offset = draws.length;
        draws.push(...mutation.list);
        appendSetDraw(ops, mutation.id as number, offset, mutation.list.length);
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
      case 'setUserSelect':
        appendSetUserSelect(
          ops,
          mutation.id as number,
          USER_SELECT[mutation.value],
        );
        break;
      case 'setMultiline':
        appendSetMultiline(
          ops,
          mutation.id as number,
          mutation.multiline ? 1 : 0,
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
    draws: new Float32Array(draws),
  };
}
