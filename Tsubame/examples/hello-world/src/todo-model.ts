export type Priority = 1 | 2 | 3; // 1=低, 2=中, 3=高
export type Filter = 'all' | 'active' | 'done';
export type SortMode = 'manual' | 'name' | 'prio';

/** Compile-time guard: `List` must enumerate every member of `Union`. */
type AssertExhaustive<Union, List extends readonly Union[]> = Exclude<
  Union,
  List[number]
> extends never
  ? true
  : never;

/** 表示フィルタの正本。UI のチップ順もこの順に従う。 */
export const FILTER_VALUES = ['all', 'active', 'done'] as const;
const _filterExhaustive: AssertExhaustive<Filter, typeof FILTER_VALUES> = true;

/** 並び順の正本。UI のチップ順もこの順に従う。 */
export const SORT_VALUES = ['manual', 'name', 'prio'] as const;
const _sortExhaustive: AssertExhaustive<SortMode, typeof SORT_VALUES> = true;

/** 優先度の正本。追加フォームのセグメント順（高→低）もこの順に従う。 */
export const PRIORITY_VALUES = [3, 2, 1] as const;
const _priorityExhaustive: AssertExhaustive<Priority, typeof PRIORITY_VALUES> = true;

export interface Todo {
  id: number;
  text: string;
  prio: Priority;
  done: boolean;
}

export interface TodoDraft {
  id: number;
  text: string;
  prio: Priority;
}

/** 新規タスクを先頭に挿入する（未完了で作成）。空文字・空白のみは無視。 */
export function add(todos: readonly Todo[], draft: TodoDraft): Todo[] {
  const text = draft.text.trim();
  if (!text) return [...todos];
  return [{ id: draft.id, text, prio: draft.prio, done: false }, ...todos];
}

/** 指定 id の完了/未完了をトグルする。 */
export function toggleDone(todos: readonly Todo[], id: number): Todo[] {
  return todos.map((todo) => (todo.id === id ? { ...todo, done: !todo.done } : todo));
}

/** 指定 id の文言を編集する（trim 後）。空文字は無視して変更しない。 */
export function editText(todos: readonly Todo[], id: number, text: string): Todo[] {
  const trimmed = text.trim();
  if (!trimmed) return [...todos];
  return todos.map((todo) => (todo.id === id ? { ...todo, text: trimmed } : todo));
}

/** 指定 id のタスクを削除する。 */
export function remove(todos: readonly Todo[], id: number): Todo[] {
  return todos.filter((todo) => todo.id !== id);
}

/** 完了済みタスクを一括削除する。 */
export function clearDone(todos: readonly Todo[]): Todo[] {
  return todos.filter((todo) => !todo.done);
}

/** index i と i+1 を入れ替える。範囲外なら変更しない。 */
function swap(todos: readonly Todo[], i: number): Todo[] {
  if (i < 0 || i + 1 >= todos.length) return [...todos];
  const next = [...todos];
  [next[i], next[i + 1]] = [next[i + 1], next[i]];
  return next;
}

/** 指定 id を一つ上へ移動する（手動並べ替え）。 */
export function moveUp(todos: readonly Todo[], id: number): Todo[] {
  return swap(todos, todos.findIndex((todo) => todo.id === id) - 1);
}

/** 指定 id を一つ下へ移動する（手動並べ替え）。 */
export function moveDown(todos: readonly Todo[], id: number): Todo[] {
  return swap(todos, todos.findIndex((todo) => todo.id === id));
}

/**
 * 手動並べ替え（moveUp/moveDown）が意味を持つ並び順かを返す。
 * name/prio は表示順が導出されるため、上/下ボタンは manual のときだけ有効。
 */
export function canReorder(sort: SortMode): boolean {
  return sort === 'manual';
}

/** 表示フィルタを適用する（all / active=未完了 / done=完了）。 */
export function filterTodos(todos: readonly Todo[], filter: Filter): Todo[] {
  if (filter === 'active') return todos.filter((todo) => !todo.done);
  if (filter === 'done') return todos.filter((todo) => todo.done);
  return [...todos];
}

/** 並び順を適用する（manual=手動 / name=名前(ja) / prio=優先度降順）。常に新配列を返す。 */
export function sortTodos(todos: readonly Todo[], sort: SortMode): Todo[] {
  const next = [...todos];
  if (sort === 'name') return next.sort((a, b) => a.text.localeCompare(b.text, 'ja'));
  if (sort === 'prio') return next.sort((a, b) => b.prio - a.prio);
  return next;
}

/**
 * 単カードのリストに表示する Todo を導出する。
 * フィルタ → ソートの順で適用する（gomi の単カードと同じ可視化規則）。常に新配列。
 */
export function visibleTodos(todos: readonly Todo[], filter: Filter, sort: SortMode): Todo[] {
  return sortTodos(filterTodos(todos, filter), sort);
}

export interface Completion {
  total: number;
  remaining: number;
  percent: number;
}

/** 完了状況を集計する（残り件数 / 全件 / 完了率%）。 */
export function completion(todos: readonly Todo[]): Completion {
  const total = todos.length;
  const remaining = todos.filter((todo) => !todo.done).length;
  const percent = total === 0 ? 0 : Math.round(((total - remaining) / total) * 100);
  return { total, remaining, percent };
}

/** localStorage に書き込む既定のキー。 */
export const STORAGE_KEY = 'pop-todo-items-v1';

/** 永続化が空・破損していたときに使う初期データ。 */
export const SEED: readonly Todo[] = [
  { id: 1, text: 'レイアウトエンジンに flex-wrap を実装', prio: 3, done: false },
  { id: 2, text: 'box-shadow の描画を確認する', prio: 2, done: true },
  { id: 3, text: 'ドラッグで並べ替えできるかテスト', prio: 2, done: false },
  { id: 4, text: 'ダークモードの配色を調整', prio: 1, done: false },
  { id: 5, text: 'sticky ヘッダーの挙動チェック', prio: 3, done: true },
];

/** seed を毎回新しい配列・要素として複製する（共有ミュータブル状態を避ける）。 */
function seedClone(): Todo[] {
  return SEED.map((todo) => ({ ...todo }));
}

function isTodo(value: unknown): value is Todo {
  if (typeof value !== 'object' || value === null) return false;
  const todo = value as Record<string, unknown>;
  return (
    typeof todo.id === 'number' &&
    typeof todo.text === 'string' &&
    (todo.prio === 1 || todo.prio === 2 || todo.prio === 3) &&
    typeof todo.done === 'boolean'
  );
}

/** Todo 配列を localStorage 保存用の文字列へ変換する。 */
export function serialize(todos: readonly Todo[]): string {
  return JSON.stringify(todos);
}

/**
 * 保存文字列を Todo 配列へ復元する。
 * null・不正 JSON・配列でない・要素の形が壊れている場合は seed のコピーへフォールバック。
 * 空配列は「すべて削除済み」の正常状態として尊重する。
 */
export function deserialize(raw: string | null): Todo[] {
  if (raw === null) return seedClone();
  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed) && parsed.every(isTodo)) {
      return parsed.map((todo) => ({ id: todo.id, text: todo.text, prio: todo.prio, done: todo.done }));
    }
  } catch {
    // 壊れた JSON は seed フォールバックへ落とす
  }
  return seedClone();
}

/** `localStorage` 互換の最小インターフェース（テストでは差し替え可能）。 */
export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** ストレージから Todo を読み込む（無い/壊れていれば seed）。 */
export function loadTodos(storage: StorageLike, key: string = STORAGE_KEY): Todo[] {
  return deserialize(storage.getItem(key));
}

/** Todo をストレージへ保存する。 */
export function saveTodos(storage: StorageLike, todos: readonly Todo[], key: string = STORAGE_KEY): void {
  storage.setItem(key, serialize(todos));
}
