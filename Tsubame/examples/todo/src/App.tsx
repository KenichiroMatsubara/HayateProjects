import { createEffect, createMemo, createSignal } from 'solid-js';
import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { CssGallery } from './CssGallery';
import type { DetectModeResult } from './detect-mode';
import {
  ACCENT_KEYS,
  accentColor,
  inputStyle,
  loadTheme,
  palette,
  saveTheme,
  type AccentKey,
  type Palette,
  type Theme,
} from './theme';
import {
  add,
  canReorder,
  clearDone,
  completion,
  editText,
  FILTER_VALUES,
  moveDown,
  moveUp,
  PRIORITY_VALUES,
  remove,
  SEED,
  SORT_VALUES,
  toggleDone,
  visibleTodos,
  type Filter,
  type Priority,
  type SortMode,
  type Todo,
} from './todo-model';

type Page = 'tasks' | 'gallery';

export interface TodoAppProps {
  detected: DetectModeResult;
}

function priorityTone(p: Palette, prio: Priority): string {
  if (prio === 3) return p.danger;
  if (prio === 2) return p.accent2;
  return p.blue;
}

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

const SpX = (w: number) => <view style={{ width: w, height: 1 }} />;

function seedTodos(): Todo[] {
  return SEED.map((todo) => ({ ...todo }));
}

function rendererBadge(detected: DetectModeResult): string {
  if (detected.mode === 'DOM') return 'DOM';
  return detected.backend ?? 'Canvas';
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
        : <scroll-view style={{
          flexGrow: 1,
          width: '100%',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          paddingTop: 28,
          paddingBottom: 28,
          backgroundColor: colors().bg,
        }}>
          <view style={{
            width: 620,
            maxWidth: '100%',
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            padding: 22,
            backgroundColor: colors().panel,
            borderRadius: 18,
            borderWidth: 1,
            borderColor: colors().line,
            boxShadow: [{ offsetX: 0, offsetY: 18, blur: 40, spread: -8, color: colors().shadow, inset: false }],
          }}>
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

function AppBar(props: {
  page: Page;
  setPage: (page: Page) => void;
  detected: DetectModeResult;
  colors: Palette;
  theme: Theme;
  accent: AccentKey;
  onToggleTheme: () => void;
  onAccent: (accent: AccentKey) => void;
}) {
  const tab = (active: boolean): HayateCssStyle => ({
    height: 34,
    paddingLeft: 16,
    paddingRight: 16,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? props.colors.accent : props.colors.panel,
    defaultColor: active ? props.colors.black : props.colors.text,
    borderRadius: 10,
    borderWidth: 1,
    borderColor: active ? props.colors.accent : props.colors.line,
    defaultFontSize: 13,
    ':hover': {
      backgroundColor: active ? props.colors.accent : props.colors.panel3,
      borderColor: active ? props.colors.accent : props.colors.line,
    },
  });

  const swatch = (key: AccentKey): HayateCssStyle => {
    const selected = props.accent === key;
    return {
      width: 22,
      height: 22,
      backgroundColor: accentColor(props.theme, key),
      borderRadius: 999,
      borderWidth: selected ? 3 : 1,
      borderColor: selected ? props.colors.ink : props.colors.line,
      ':hover': { borderColor: props.colors.ink },
    };
  };

  return (
    <view style={{
      height: 64,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
      backgroundColor: props.colors.rail,
      borderWidth: 1,
      borderColor: props.colors.line,
    }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        {SpX(24)}
        <view style={{
          width: 38,
          height: 38,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: props.colors.accent,
          borderRadius: 12,
        }}>
          <text style={{ fontSize: 18, color: props.colors.black }}>TS</text>
        </view>
        <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <text style={{ fontSize: 20, color: props.colors.ink }}>Tsubame Task Studio</text>
          <text style={{ fontSize: 12, color: props.colors.muted }}>POP TODO + Hayate CSS gallery</text>
        </view>
      </view>

      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <button style={tab(props.page === 'tasks')} onClick={() => props.setPage('tasks')}>Tasks</button>
        <button style={tab(props.page === 'gallery')} onClick={() => props.setPage('gallery')}>CSS Gallery</button>

        <view style={{ width: 1, height: 22, backgroundColor: props.colors.line }} />
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          {ACCENT_KEYS.map((key) => (
            <button style={swatch(key)} onClick={() => props.onAccent(key)}>{' '}</button>
          ))}
        </view>
        <button
          style={{
            width: 34,
            height: 34,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: props.colors.panel,
            defaultColor: props.colors.text,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: props.colors.line,
            defaultFontSize: 15,
            ':hover': { backgroundColor: props.colors.panel3, borderColor: props.colors.line },
          }}
          onClick={props.onToggleTheme}
        >
          {props.theme === 'dark' ? '☀' : '🌙'}
        </button>

        <text style={{ color: props.colors.quiet, fontSize: 11 }}>renderer</text>
        <view style={{
          height: 28,
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          backgroundColor: props.colors.panel,
          borderRadius: 10,
          borderWidth: 1,
          borderColor: props.colors.line,
        }}>
          {SpX(12)}
          <text style={{ color: props.colors.accent, fontSize: 13 }}>{rendererBadge(props.detected)}</text>
          {SpX(10)}
          <view style={{ width: 1, height: 16, backgroundColor: props.colors.line }} />
          {SpX(10)}
          <text style={{ color: props.colors.muted, fontSize: 12 }}>
            {props.detected.source === 'query' ? props.detected.renderer : 'auto'}
          </text>
          {SpX(12)}
        </view>
        {SpX(24)}
      </view>
    </view>
  );
}

function Header(props: { colors: Palette; remaining: number; total: number; percent: number }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <view style={{
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
      }}>
        <text style={{ color: props.colors.ink, fontSize: 24 }}>きょうのタスク</text>
        <text style={{ color: props.colors.muted, fontSize: 13 }}>
          {`残り ${props.remaining} 件 / 全 ${props.total} 件`}
        </text>
      </view>
      <ProgressBar colors={props.colors} percent={props.percent} />
    </view>
  );
}

/**
 * 読み取り専用テキストの選択ジェスチャデモ（ADR-0097 / issue #266・#267・#268・#269）。
 * `selectable` な view が Selection Region を確立し、Canvas Mode では core が選択
 * ハイライトを SceneGraph に描画する。DOM Mode ではブラウザネイティブ選択に委ねる。
 * ドラッグに加え、ダブルクリックで単語・トリプルクリックで段落、Shift+クリック /
 * Shift+矢印で範囲拡張、Cmd/Ctrl+A で全選択ができる。Cmd/Ctrl+C を押すと選択テキ
 * ストが Platform Adapter 経由でクリップボードへコピーされ、別アプリへ貼り付けられる。
 *
 * 複数の段落（別々の block）を一つの Selection Region に並べているので、ある段落
 * から次の段落へドラッグすると block をまたいだ連続選択になる（issue #269）。選択は
 * `selectable` の境界を越えて外側のテキストへは漏れない。
 */
function SelectableNote(props: { colors: Palette }) {
  const para = { color: props.colors.muted, fontSize: 13 } as const;
  return (
    <view
      selectable
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 12,
        backgroundColor: props.colors.panel2,
        borderRadius: 12,
        borderWidth: 1,
        borderColor: props.colors.line,
      }}
    >
      <text style={para}>
        この段落は選択できます。ダブルクリックで単語、トリプルクリックで段落を選び、Shift+クリックや Shift+矢印で範囲を伸縮、Cmd/Ctrl+A で全選択できます。選択して Cmd/Ctrl+C を押すとクリップボードへコピーされ、別アプリへ貼り付けられます。
      </text>
      <text style={para}>
        これは二つ目の段落です。一つ目の段落からここまでドラッグすると、block をまたいだ連続選択になります（issue #269）。先頭 block は anchor から行末まで、末尾 block は行頭から focus まで、間の block は丸ごとハイライトされます。
      </text>
      <text style={para}>
        Canvas Mode では core が選択ハイライトを描画し、DOM Mode ではブラウザのネイティブ選択に委ねます。選択は `selectable` の境界内に閉じ、外側のテキストへは漏れません。
      </text>
    </view>
  );
}

function ProgressBar(props: { colors: Palette; percent: number }) {
  return (
    <view style={{
      width: '100%',
      height: 12,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      backgroundColor: props.colors.black,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: props.colors.line,
    }}>
      <view style={{
        width: `${props.percent}%`,
        height: 8,
        marginLeft: 2,
        backgroundColor: props.colors.success,
        borderRadius: 6,
      }} />
    </view>
  );
}

function AddForm(props: {
  colors: Palette;
  draft: string;
  prio: Priority;
  onInput: (text: string) => void;
  onPrio: (prio: Priority) => void;
  onAdd: () => void;
}) {
  const seg = (active: boolean, tone: string): HayateCssStyle => ({
    height: 38,
    minWidth: 40,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? tone : props.colors.panel2,
    defaultColor: active ? props.colors.black : props.colors.muted,
    borderRadius: 9,
    borderWidth: 1,
    borderColor: active ? tone : props.colors.line,
    defaultFontSize: 13,
    ':hover': {
      backgroundColor: active ? tone : props.colors.panel3,
      borderColor: active ? tone : props.colors.line,
    },
  });

  return (
    <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      <view style={{ flexGrow: 1 }}>
        <text-input
          value={props.draft}
          placeholder="新しいタスクを入力…"
          style={inputStyle(props.colors)}
          onInput={(event) => props.onInput(event.value ?? '')}
          onKeyDown={(event) => {
            if (event.key === 'Enter') props.onAdd();
          }}
        />
      </view>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 4 }}>
        {PRIORITIES.map((prio) => (
          <button
            style={seg(props.prio === prio, priorityTone(props.colors, prio))}
            onClick={() => props.onPrio(prio)}
          >
            {PRIORITY_LABEL[prio]}
          </button>
        ))}
      </view>
      <button
        style={{
          height: 38,
          paddingLeft: 18,
          paddingRight: 18,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: props.colors.accent,
          defaultColor: props.colors.black,
          borderRadius: 9,
          borderWidth: 1,
          borderColor: props.colors.accent,
          defaultFontSize: 13,
          ':hover': { backgroundColor: props.colors.success, borderColor: props.colors.success },
        }}
        onClick={props.onAdd}
      >
        追加
      </button>
    </view>
  );
}

function chipStyle(p: Palette, active: boolean): HayateCssStyle {
  return {
    height: 30,
    paddingLeft: 12,
    paddingRight: 12,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? p.accent : p.panel2,
    defaultColor: active ? p.black : p.text,
    borderRadius: 999,
    borderWidth: 1,
    borderColor: active ? p.accent : p.line,
    defaultFontSize: 12,
    ':hover': {
      backgroundColor: active ? p.accent : p.panel3,
      borderColor: active ? p.accent : p.line,
    },
  };
}

function Toolbar(props: {
  colors: Palette;
  filter: Filter;
  sort: SortMode;
  onFilter: (filter: Filter) => void;
  onSort: (sort: SortMode) => void;
}) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      gap: 8,
      paddingTop: 10,
      paddingBottom: 10,
    }}>
      <text style={{ color: props.colors.quiet, fontSize: 12 }}>表示</text>
      {FILTERS.map((item) => (
        <button style={chipStyle(props.colors, props.filter === item.value)} onClick={() => props.onFilter(item.value)}>
          {item.label}
        </button>
      ))}
      <view style={{ width: 1, height: 18, marginLeft: 4, marginRight: 4, backgroundColor: props.colors.line }} />
      <text style={{ color: props.colors.quiet, fontSize: 12 }}>並び</text>
      {SORTS.map((item) => (
        <button style={chipStyle(props.colors, props.sort === item.value)} onClick={() => props.onSort(item.value)}>
          {item.label}
        </button>
      ))}
    </view>
  );
}

function iconButton(p: Palette): HayateCssStyle {
  return {
    width: 30,
    height: 30,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: p.panel,
    defaultColor: p.muted,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: p.line,
    defaultFontSize: 14,
    ':hover': { backgroundColor: p.panel3, borderColor: p.line, defaultColor: p.text },
  };
}

function TodoRow(props: {
  colors: Palette;
  todo: Todo;
  reorderable: boolean;
  editing: boolean;
  editDraft: string;
  onToggle: () => void;
  onRemove: () => void;
  onBeginEdit: () => void;
  onEditInput: (text: string) => void;
  onCommitEdit: () => void;
  onCancelEdit: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const done = props.todo.done;
  const p = props.colors;
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      gap: 12,
      padding: 12,
      backgroundColor: p.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderColor: p.line,
      opacity: done ? 0.62 : 1,
      boxShadow: [{ offsetX: 0, offsetY: 2, blur: 6, spread: -1, color: p.shadow, inset: false }],
      ':hover': {
        backgroundColor: p.panel3,
        borderColor: p.line,
        boxShadow: [{ offsetX: 0, offsetY: 6, blur: 16, spread: -2, color: p.shadow, inset: false }],
      },
    }}>
      <button
        style={{
          width: 24,
          height: 24,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: done ? p.success : p.panel,
          defaultColor: p.black,
          borderRadius: 7,
          borderWidth: 1,
          borderColor: done ? p.success : p.line,
          defaultFontSize: 14,
          ':hover': { borderColor: p.success },
        }}
        onClick={props.onToggle}
      >
        {done ? '✓' : ' '}
      </button>
      <view style={{
        width: 10,
        height: 10,
        backgroundColor: priorityTone(p, props.todo.prio),
        borderRadius: 999,
      }} />
      <view style={{ flexGrow: 1, display: 'flex', flexDirection: 'column' }}>
        {props.editing
          ? <text-input
            value={props.editDraft}
            style={{ ...inputStyle(p), height: 30, fontSize: 15 }}
            onInput={(event) => props.onEditInput(event.value ?? '')}
            onKeyDown={(event) => {
              const action = editKeyAction(event.key ?? '');
              if (action === 'commit') props.onCommitEdit();
              else if (action === 'cancel') props.onCancelEdit();
            }}
            onBlur={props.onCommitEdit}
          />
          : <button
            style={{
              display: 'flex',
              alignItems: 'center',
              backgroundColor: 'transparent',
              defaultColor: done ? p.quiet : p.ink,
              defaultFontSize: 15,
              borderWidth: 0,
              ':hover': { defaultColor: p.accent },
            }}
            onClick={props.onBeginEdit}
          >
            {props.todo.text}
          </button>}
      </view>
      <text style={{ color: p.quiet, fontSize: 11 }}>{`優先度 ${PRIORITY_LABEL[props.todo.prio]}`}</text>
      {props.reorderable
        ? <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <button style={iconButton(p)} onClick={props.onMoveUp}>↑</button>
          <button style={iconButton(p)} onClick={props.onMoveDown}>↓</button>
        </view>
        : null}
      <button
        style={{
          ...iconButton(p),
          ':hover': { backgroundColor: p.dangerBg, borderColor: p.danger, defaultColor: p.danger },
        }}
        onClick={props.onRemove}
      >
        ✕
      </button>
    </view>
  );
}

function EmptyState(props: { colors: Palette }) {
  return (
    <view style={{
      height: 96,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: props.colors.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderColor: props.colors.line,
    }}>
      <text style={{ color: props.colors.muted, fontSize: 14 }}>表示するタスクがありません</text>
    </view>
  );
}

function Footer(props: { colors: Palette; percent: number; onClearDone: () => void }) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
    }}>
      <text style={{ color: props.colors.muted, fontSize: 13 }}>{`${props.percent}% 完了`}</text>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <text style={{ color: props.colors.quiet, fontSize: 11 }}>クリックで完了 / ✕ で削除</text>
        <button
          style={{
            height: 30,
            paddingLeft: 12,
            paddingRight: 12,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: props.colors.panel2,
            defaultColor: props.colors.text,
            borderRadius: 8,
            borderWidth: 1,
            borderColor: props.colors.line,
            defaultFontSize: 12,
            ':hover': { backgroundColor: props.colors.panel3, borderColor: props.colors.line },
          }}
          onClick={props.onClearDone}
        >
          完了を消す
        </button>
      </view>
    </view>
  );
}
