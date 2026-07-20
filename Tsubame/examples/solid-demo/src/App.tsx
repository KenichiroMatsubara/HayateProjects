import { createEffect, createMemo, createSignal } from 'solid-js';
import { CssGallery } from './CssGallery';
import { AppBar } from './components/AppBar';
import { FreeDrawPrototype } from './prototype/FreeDrawPrototype';
import {
  loadTheme,
  palette,
  saveTheme,
  type AccentKey,
  type Theme,
} from './theme';
import {
  add,
  clearDone,
  completion,
  editText,
  moveDown,
  moveUp,
  remove,
  SEED,
  toggleDone,
  visibleTodos,
  type Filter,
  type Priority,
  type SortMode,
  type Todo,
} from './todo-model';

/** タスク画面とギャラリー画面のどちらを表示するか。 */
export type Page = 'tasks' | 'gallery';

function seedTodos(): Todo[] {
  return SEED.map((todo) => ({ ...todo }));
}

export function TodoApp() {
  const initialPage: Page =
    new URLSearchParams(window.location.search).get('page') === 'gallery' ? 'gallery' : 'tasks';
  const [page, setPage] = createSignal<Page>(initialPage);
  const [todos, setTodos] = createSignal<Todo[]>(seedTodos());
  const [filter, setFilter] = createSignal<Filter>('all');
  const [sort, setSort] = createSignal<SortMode>('manual');
  const [draftPrio, setDraftPrio] = createSignal<Priority>(2);
  const [draft, setDraft] = createSignal('');
  // インライン編集の対象行（null=非編集）と、その編集中テキスト。
  const [editingId, setEditingId] = createSignal<number | null>(null);
  const [editDraft, setEditDraft] = createSignal('');
  let nextId = 1000;

  // テーマ・アクセントは localStorage で永続化（#247 の方針）。既定はライト。
  const initialPrefs = loadTheme(window.localStorage);
  const [theme, setTheme] = createSignal<Theme>(initialPrefs.theme);
  const [accent, setAccent] = createSignal<AccentKey>(initialPrefs.accent);
  const colors = createMemo(() => palette(theme(), accent()));
  createEffect(() => saveTheme(window.localStorage, { theme: theme(), accent: accent() }));

  // Hayate アプリ外の素 HTML オーバーレイ（#renderer-switch）へ現在パレットを橋渡しする。
  // Hayate CSS には custom properties が無いため、document の CSS 変数経由で同期し、
  // テーマ/アクセント変更にライブ追従させる（正本はここ・index.html は boot 近似）。
  createEffect(() => {
    const p = colors();
    const root = document.documentElement.style;
    root.setProperty('--rsw-bg', p.rail);
    root.setProperty('--rsw-line', p.line);
    root.setProperty('--rsw-text', p.muted);
    root.setProperty('--rsw-ink', p.ink);
    root.setProperty('--rsw-hover', p.panel3);
    root.setProperty('--rsw-on-accent', p.black);
    root.setProperty('--rsw-accent', p.accent);
  });

  const toggleTheme = () => setTheme((current) => (current === 'dark' ? 'light' : 'dark'));

  const visible = createMemo(() => visibleTodos(todos(), filter(), sort()));
  const summary = createMemo(() => completion(todos()));

  const addTask = () => {
    const text = draft();
    if (!text.trim()) return;
    setTodos(add(todos(), { id: nextId++, text, prio: draftPrio() }));
    setDraft('');
  };

  const toggle = (id: number) => setTodos(toggleDone(todos(), id));
  const removeTask = (id: number) => setTodos(remove(todos(), id));
  const clearCompleted = () => setTodos(clearDone(todos()));
  const moveTaskUp = (id: number) => setTodos(moveUp(todos(), id));
  const moveTaskDown = (id: number) => setTodos(moveDown(todos(), id));

  // インライン編集: クリックで開始、Enter/blur で確定、Escape で取消。
  const beginEdit = (todo: Todo) => {
    setEditingId(todo.id);
    setEditDraft(todo.text);
  };
  const commitEdit = () => {
    const id = editingId();
    if (id === null) return; // Escape 後の blur など、二重確定を無視する。
    setTodos(editText(todos(), id, editDraft())); // 空文字はモデル側で無視。
    setEditingId(null);
  };
  const cancelEdit = () => setEditingId(null);

  return (
    <view style={{
      width: '100%',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: colors().bg,
      defaultColor: colors().text,
      defaultFontSize: 14,
      defaultFontFamily: 'Inter, Segoe UI, system-ui, sans-serif',
    }}>
      <AppBar
        page={page()}
        setPage={setPage}
        colors={colors()}
        theme={theme()}
        accent={accent()}
        onToggleTheme={toggleTheme}
        onAccent={setAccent}
      />

      {page() === 'gallery'
        ? <CssGallery colors={colors()} />
        : <FreeDrawPrototype
          colors={colors()}
          todos={visible()}
          filter={filter()}
          sort={sort()}
          draft={draft()}
          draftPrio={draftPrio()}
          editingId={editingId()}
          editDraft={editDraft()}
          summary={summary()}
          onDraft={setDraft}
          onDraftPrio={setDraftPrio}
          onAdd={addTask}
          onFilter={setFilter}
          onSort={setSort}
          onToggle={toggle}
          onRemove={removeTask}
          onBeginEdit={beginEdit}
          onEditInput={setEditDraft}
          onCommitEdit={commitEdit}
          onCancelEdit={cancelEdit}
          onMoveUp={moveTaskUp}
          onMoveDown={moveTaskDown}
          onClearDone={clearCompleted}
        />}
    </view>
  );
}
