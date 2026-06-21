import type {
  ElementId,
  ElementKind,
  PseudoStyleKey,
  StylePatch,
  UserSelect,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { encodeMutations, type SemanticMutation } from './encode-mutations.js';

/**
 * CanvasRenderer → Hayate WASM 境界向けの順序付き Hayate Mutation Packet キュー。
 *
 * パケットの B-lite 形式: セマンティックな操作順序を保ち、低レベルの op/style/text
 * バッファは境界フラッシュ時にのみ出力する。キュー済みのセマンティック変更を
 * マージ・刈り込み・結合・最適化することは意図的に行わない。
 *
 * ワイヤフォーマットのエンコードは純粋な {@link encodeMutations} に閉じている。
 * 本クラスは順序付きバッファと境界呼び出し一回ぶんに過ぎない。
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

  enqueueSetUserSelect(id: ElementId, value: UserSelect): void {
    this.mutations.push({ kind: 'setUserSelect', id, value });
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
