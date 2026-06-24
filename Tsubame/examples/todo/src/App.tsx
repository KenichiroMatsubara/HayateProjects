import { createEffect, createMemo, createSignal } from 'solid-js';
import { CssGallery } from './CssGallery';
import type { DetectModeResult } from './detect-mode';
import { AddForm } from './components/AddForm';
import { AppBar } from './components/AppBar';
import { EmptyState, Footer, Header, SelectableNote } from './components/TaskCard';
import { TodoRow } from './components/TodoRow';
import { Toolbar } from './components/Toolbar';
import {
  loadTheme,
  palette,
  saveTheme,
  type AccentKey,
  type Theme,
} from './theme';
import {
  add,
  canReorder,
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

export interface TodoAppProps {
  detected: DetectModeResult;
}

function seedTodos(): Todo[] {
  return SEED.map((todo) => ({ ...todo }));
}

export function TodoApp(props: TodoAppProps) {
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
        detected={props.detected}
        colors={colors()}
        theme={theme()}
        accent={accent()}
        onToggleTheme={toggleTheme}
        onAccent={setAccent}
      />

      {page() === 'gallery'
        ? <CssGallery colors={colors()} />
        : <scroll-view
          style={{
            flexGrow: 1,
            width: '100%',
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            paddingTop: 28,
            paddingBottom: 28,
            paddingLeft: 16,
            paddingRight: 16,
            backgroundColor: colors().bg,
          }}
          // 狭幅では左右余白を詰める。カードが画面端へ密着しないよう左右パディングは残す。
          styleVariants={[
            { condition: { maxWidth: 719 }, style: { paddingTop: 16, paddingBottom: 16, paddingLeft: 12, paddingRight: 12 } },
          ]}
        >
          <view
            style={{
              width: 620,
              maxWidth: '100%',
              display: 'flex',
              flexDirection: 'column',
              gap: 16,
              padding: 22,
              backgroundColor: colors().panel,
              borderRadius: 18,
              borderWidth: 1,
              borderStyle: 'solid',
              borderColor: colors().line,
              boxShadow: [{ offsetX: 0, offsetY: 18, blur: 40, spread: -8, color: colors().shadow, inset: false }],
            }}
            // 狭幅では余白と角丸を詰める（本物の @media・ADR-0081）。
            styleVariants={[
              { condition: { maxWidth: 719 }, style: { padding: 14, gap: 12, borderRadius: 12 } },
            ]}
          >
            <Header colors={colors()} remaining={summary().remaining} total={summary().total} percent={summary().percent} />
            <SelectableNote colors={colors()} />
            <AddForm
              colors={colors()}
              draft={draft()}
              prio={draftPrio()}
              onInput={setDraft}
              onPrio={setDraftPrio}
              onAdd={addTask}
            />
            <Toolbar colors={colors()} filter={filter()} sort={sort()} onFilter={setFilter} onSort={setSort} />
            <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {visible().length === 0
                ? <EmptyState colors={colors()} />
                : visible().map((todo) => (
                  <TodoRow
                    colors={colors()}
                    todo={todo}
                    reorderable={canReorder(sort())}
                    editing={editingId() === todo.id}
                    editDraft={editDraft()}
                    onToggle={() => toggle(todo.id)}
                    onRemove={() => removeTask(todo.id)}
                    onBeginEdit={() => beginEdit(todo)}
                    onEditInput={setEditDraft}
                    onCommitEdit={commitEdit}
                    onCancelEdit={cancelEdit}
                    onMoveUp={() => moveTaskUp(todo.id)}
                    onMoveDown={() => moveTaskDown(todo.id)}
                  />
                ))}
            </view>
            <view style={{ height: 1, backgroundColor: colors().line }} />
            <Footer colors={colors()} percent={summary().percent} onClearDone={clearCompleted} />
          </view>
        </scroll-view>}
    </view>
  );
}
