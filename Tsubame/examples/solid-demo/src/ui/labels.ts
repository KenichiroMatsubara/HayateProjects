import {
  FILTER_VALUES,
  PRIORITY_VALUES,
  SORT_VALUES,
  type Filter,
  type Priority,
  type SortMode,
} from '../todo-model';

/** 優先度の表示ラベル（追加フォーム・行で共有）。 */
export const PRIORITY_LABEL: Record<Priority, string> = {
  3: '高',
  2: '中',
  1: '低',
};

const FILTER_LABEL: Record<Filter, string> = {
  all: 'すべて',
  active: '未完了',
  done: '完了済み',
};

/** ツールバーのフィルタ chip。モデルの正本 `FILTER_VALUES` から導出する。 */
export const FILTERS: { value: Filter; label: string }[] = FILTER_VALUES.map((value) => ({
  value,
  label: FILTER_LABEL[value],
}));

const SORT_LABEL: Record<SortMode, string> = {
  manual: '手動',
  name: '名前',
  prio: '優先度',
};

/** ツールバーのソート chip。モデルの正本 `SORT_VALUES` から導出する。 */
export const SORTS: { value: SortMode; label: string }[] = SORT_VALUES.map((value) => ({
  value,
  label: SORT_LABEL[value],
}));

/** 追加フォームの優先度セグメント。モデルの正本 `PRIORITY_VALUES` から導出する。 */
export const PRIORITIES: Priority[] = [...PRIORITY_VALUES];

/** インライン編集中の keydown が表す操作。`dblclick` は語彙に無いため keydown のみ。 */
export type EditKeyAction = 'commit' | 'cancel' | 'none';

/** インライン編集の確定/取消キーを判定する（Enter=確定 / Escape=取消 / それ以外=無視）。 */
export function editKeyAction(key: string): EditKeyAction {
  if (key === 'Enter') return 'commit';
  if (key === 'Escape') return 'cancel';
  return 'none';
}
