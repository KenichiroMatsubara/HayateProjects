// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @torimi/hayate-protocol-spec edit_intents.json

export type EditDirection = 'backward' | 'forward' | 'up' | 'down';
export type EditGranularity = 'grapheme' | 'word' | 'lineBoundary' | 'docBoundary';
export type EditDispatchOutcome = 'consumed' | 'unhandled' | 'deferred';
export type EditIntent =
  | { kind: 'move' | 'extend' | 'delete'; granularity: EditGranularity; direction: EditDirection }
  | { kind: 'insertLineBreak' | 'selectAll' | 'copy' | 'cut' | 'paste' };

export const EDIT_INTENT_TAG = {
  "move": 0,
  "extend": 1,
  "delete": 2,
  "insert_line_break": 3,
  "select_all": 4,
  "copy": 5,
  "cut": 6,
  "paste": 7
} as const;
const GRANULARITY = { grapheme: 0, word: 1, lineBoundary: 2, docBoundary: 3 } as const;
const DIRECTION = { backward: 0, forward: 1, up: 2, down: 3 } as const;

export function encodeEditIntent(intent: EditIntent): Float64Array {
  switch (intent.kind) {
    case 'move': case 'extend': case 'delete':
      return new Float64Array([EDIT_INTENT_TAG[intent.kind], GRANULARITY[intent.granularity], DIRECTION[intent.direction]]);
    case 'insertLineBreak': return new Float64Array([EDIT_INTENT_TAG.insert_line_break]);
    case 'selectAll': return new Float64Array([EDIT_INTENT_TAG.select_all]);
    case 'copy': return new Float64Array([EDIT_INTENT_TAG.copy]);
    case 'cut': return new Float64Array([EDIT_INTENT_TAG.cut]);
    case 'paste': return new Float64Array([EDIT_INTENT_TAG.paste]);
  }
}

export function editDispatchOutcomeFromWire(value: number): EditDispatchOutcome {
  if (value === 0) return 'consumed';
  if (value === 1) return 'unhandled';
  if (value === 2) return 'deferred';
  throw new RangeError(`unknown EditDispatchOutcome ${value}`);
}
