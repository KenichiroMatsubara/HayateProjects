// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @torimi/hayate-protocol-spec（element_properties）

/** 閉じた要素プロパティ語彙（ADR-0071）。`aria-*` は専用 API のみを使用する。 */
export const ELEMENT_PROPERTY_NAMES = ["value","placeholder","src","disabled","user-select","multiline"] as const;

export type ElementPropertyName = (typeof ELEMENT_PROPERTY_NAMES)[number];

/**
 * `setProperty(name, value)` 呼び出しを変換した、レンダラー非依存の結果
 *（issue #235）。判別子は論理的な効果を表す。DOM と Canvas の
 * レンダラーは*同一*の変換済みペイロードをそれぞれの媒体に適用するため、
 * 型変換のエッジケース（null の消去、文字列化、`Boolean()`）は
 * ただ一箇所に存在し、2つのレンダラー間でずれることがない。
 */
export type ElementPropertyOp =
  | { kind: 'text-content'; text: string }
  | { kind: 'placeholder'; text: string }
  | { kind: 'src'; text: string }
  | { kind: 'disabled'; disabled: boolean }
  | { kind: 'user-select'; value: 'text' | 'none' | 'contains' }
  | { kind: 'multiline'; multiline: boolean }
  ;

/** 既知の要素プロパティと生の値を、共有された意味論へ変換する。 */
export function coerceElementProperty(
  name: ElementPropertyName,
  value: unknown,
): ElementPropertyOp {
  switch (name) {
    case 'value':
      return { kind: 'text-content', text: value == null ? '' : String(value) };
    case 'placeholder':
      return { kind: 'placeholder', text: typeof value === 'string' ? value : '' };
    case 'src':
      return { kind: 'src', text: typeof value === 'string' ? value : '' };
    case 'disabled':
      return { kind: 'disabled', disabled: Boolean(value) };
    case 'user-select':
      return { kind: 'user-select', value: value === 'none' || value === 'contains' ? value : 'text' };
    case 'multiline':
      return { kind: 'multiline', multiline: Boolean(value) };
  }
}

/**
 * op-kind をキーとする効果ハンドラ。各レンダラーは自身の媒体（DOM 変更 /
 * Canvas のキュー投入）への書き込みでこれらを埋める。アダプターに委ねられる
 * *唯一*の部分（ADR-0008）。新しい op-kind はこのマップを広げるため、各レンダラーは
 * 新しいハンドラを供給しなければ型チェックに失敗する。
 */
export type ElementPropertyEffects<R> = {
  [Op in ElementPropertyOp as Op['kind']]: (op: Op) => R;
};

/**
 * 共有の prop-op ディスパッチ（ADR-0008）。変換済みの op を効果ハンドラへ振り分ける。
 * op-kind の分岐はここに一度だけ存在し、レンダラーが再実装することはない。
 */
export function dispatchElementPropertyOp<R>(
  op: ElementPropertyOp,
  effects: ElementPropertyEffects<R>,
): R {
  const handler = effects[op.kind] as (op: ElementPropertyOp) => R;
  return handler(op);
}
