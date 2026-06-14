import { createMemo, createSignal } from 'solid-js';
import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { CssGallery } from './CssGallery';
import type { DetectModeResult } from './detect-mode';
import { COLORS, inputStyle } from './theme';
import {
  add,
  clearDone,
  completion,
  remove,
  SEED,
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

const PRIORITY_TONE: Record<Priority, string> = {
  3: COLORS.danger,
  2: COLORS.accent2,
  1: COLORS.blue,
};

const PRIORITY_LABEL: Record<Priority, string> = {
  3: '高',
  2: '中',
  1: '低',
};

const FILTERS: { value: Filter; label: string }[] = [
  { value: 'all', label: 'すべて' },
  { value: 'active', label: '未完了' },
  { value: 'done', label: '完了済み' },
];

const SORTS: { value: SortMode; label: string }[] = [
  { value: 'manual', label: '手動' },
  { value: 'name', label: '名前' },
  { value: 'prio', label: '優先度' },
];

const PRIORITIES: Priority[] = [3, 2, 1];

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
  let nextId = 1000;

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

  return (
    <view style={{
      width: '100%',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: COLORS.bg,
      defaultColor: COLORS.text,
      defaultFontSize: 14,
      defaultFontFamily: 'Inter, Segoe UI, system-ui, sans-serif',
    }}>
      <AppBar page={page()} setPage={setPage} detected={props.detected} />

      {page() === 'gallery'
        ? <CssGallery />
        : <scroll-view style={{
          flexGrow: 1,
          width: '100%',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          paddingTop: 28,
          paddingBottom: 28,
          backgroundColor: COLORS.bg,
        }}>
          <view style={{
            width: 620,
            maxWidth: '100%',
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            padding: 22,
            backgroundColor: COLORS.panel,
            borderRadius: 18,
            borderWidth: 1,
            borderColor: COLORS.line,
          }}>
            <Header remaining={summary().remaining} total={summary().total} percent={summary().percent} />
            <AddForm
              draft={draft()}
              prio={draftPrio()}
              onInput={setDraft}
              onPrio={setDraftPrio}
              onAdd={addTask}
            />
            <Toolbar filter={filter()} sort={sort()} onFilter={setFilter} onSort={setSort} />
            <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {visible().length === 0
                ? <EmptyState />
                : visible().map((todo) => (
                  <TodoRow
                    todo={todo}
                    onToggle={() => toggle(todo.id)}
                    onRemove={() => removeTask(todo.id)}
                  />
                ))}
            </view>
            <view style={{ height: 1, backgroundColor: COLORS.line }} />
            <Footer percent={summary().percent} onClearDone={clearCompleted} />
          </view>
        </scroll-view>}
    </view>
  );
}

function AppBar(props: { page: Page; setPage: (page: Page) => void; detected: DetectModeResult }) {
  const tab = (active: boolean): HayateCssStyle => ({
    height: 34,
    paddingLeft: 16,
    paddingRight: 16,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? COLORS.accent : COLORS.panel,
    defaultColor: active ? COLORS.black : COLORS.text,
    borderRadius: 10,
    borderWidth: 1,
    borderColor: active ? COLORS.accent : COLORS.line,
    defaultFontSize: 13,
    ':hover': {
      backgroundColor: active ? COLORS.accent : COLORS.panel3,
      borderColor: active ? COLORS.accent : COLORS.line,
    },
  });

  return (
    <view style={{
      height: 64,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
      backgroundColor: COLORS.rail,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        {SpX(24)}
        <view style={{
          width: 38,
          height: 38,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: COLORS.accent,
          borderRadius: 12,
        }}>
          <text style={{ fontSize: 18, color: COLORS.black }}>TS</text>
        </view>
        <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <text style={{ fontSize: 20, color: COLORS.ink }}>Tsubame Task Studio</text>
          <text style={{ fontSize: 12, color: COLORS.muted }}>POP TODO + Hayate CSS gallery</text>
        </view>
      </view>

      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <button style={tab(props.page === 'tasks')} onClick={() => props.setPage('tasks')}>Tasks</button>
        <button style={tab(props.page === 'gallery')} onClick={() => props.setPage('gallery')}>CSS Gallery</button>
        <text style={{ color: COLORS.quiet, fontSize: 11 }}>renderer</text>
        <view style={{
          height: 28,
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          backgroundColor: COLORS.panel,
          borderRadius: 10,
          borderWidth: 1,
          borderColor: COLORS.line,
        }}>
          {SpX(12)}
          <text style={{ color: COLORS.accent, fontSize: 13 }}>{rendererBadge(props.detected)}</text>
          {SpX(10)}
          <view style={{ width: 1, height: 16, backgroundColor: COLORS.line }} />
          {SpX(10)}
          <text style={{ color: COLORS.muted, fontSize: 12 }}>
            {props.detected.source === 'query' ? props.detected.renderer : 'auto'}
          </text>
          {SpX(12)}
        </view>
        {SpX(24)}
      </view>
    </view>
  );
}

function Header(props: { remaining: number; total: number; percent: number }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <view style={{
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
      }}>
        <text style={{ color: COLORS.ink, fontSize: 24 }}>きょうのタスク</text>
        <text style={{ color: COLORS.muted, fontSize: 13 }}>
          {`残り ${props.remaining} 件 / 全 ${props.total} 件`}
        </text>
      </view>
      <ProgressBar percent={props.percent} />
    </view>
  );
}

function ProgressBar(props: { percent: number }) {
  return (
    <view style={{
      width: '100%',
      height: 12,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      backgroundColor: COLORS.black,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <view style={{
        width: `${props.percent}%`,
        height: 8,
        marginLeft: 2,
        backgroundColor: COLORS.success,
        borderRadius: 6,
      }} />
    </view>
  );
}

function AddForm(props: {
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
    backgroundColor: active ? tone : COLORS.panel2,
    defaultColor: active ? COLORS.black : COLORS.muted,
    borderRadius: 9,
    borderWidth: 1,
    borderColor: active ? tone : COLORS.line,
    defaultFontSize: 13,
    ':hover': {
      backgroundColor: active ? tone : COLORS.panel3,
      borderColor: active ? tone : COLORS.line,
    },
  });

  return (
    <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      <view style={{ flexGrow: 1 }}>
        <text-input
          value={props.draft}
          placeholder="新しいタスクを入力…"
          style={inputStyle}
          onInput={(event) => props.onInput(event.value ?? '')}
          onKeyDown={(event) => {
            if (event.key === 'Enter') props.onAdd();
          }}
        />
      </view>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 4 }}>
        {PRIORITIES.map((prio) => (
          <button
            style={seg(props.prio === prio, PRIORITY_TONE[prio])}
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
          backgroundColor: COLORS.accent,
          defaultColor: COLORS.black,
          borderRadius: 9,
          borderWidth: 1,
          borderColor: COLORS.accent,
          defaultFontSize: 13,
          ':hover': { backgroundColor: COLORS.success, borderColor: COLORS.success },
        }}
        onClick={props.onAdd}
      >
        追加
      </button>
    </view>
  );
}

function chipStyle(active: boolean): HayateCssStyle {
  return {
    height: 30,
    paddingLeft: 12,
    paddingRight: 12,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? COLORS.accent : COLORS.panel2,
    defaultColor: active ? COLORS.black : COLORS.text,
    borderRadius: 999,
    borderWidth: 1,
    borderColor: active ? COLORS.accent : COLORS.line,
    defaultFontSize: 12,
    ':hover': {
      backgroundColor: active ? COLORS.accent : COLORS.panel3,
      borderColor: active ? COLORS.accent : COLORS.line,
    },
  };
}

function Toolbar(props: {
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
      <text style={{ color: COLORS.quiet, fontSize: 12 }}>表示</text>
      {FILTERS.map((item) => (
        <button style={chipStyle(props.filter === item.value)} onClick={() => props.onFilter(item.value)}>
          {item.label}
        </button>
      ))}
      <view style={{ width: 1, height: 18, marginLeft: 4, marginRight: 4, backgroundColor: COLORS.line }} />
      <text style={{ color: COLORS.quiet, fontSize: 12 }}>並び</text>
      {SORTS.map((item) => (
        <button style={chipStyle(props.sort === item.value)} onClick={() => props.onSort(item.value)}>
          {item.label}
        </button>
      ))}
    </view>
  );
}

function TodoRow(props: { todo: Todo; onToggle: () => void; onRemove: () => void }) {
  const done = props.todo.done;
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      gap: 12,
      padding: 12,
      backgroundColor: COLORS.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderColor: COLORS.line,
      opacity: done ? 0.62 : 1,
      ':hover': { backgroundColor: COLORS.panel3, borderColor: COLORS.line },
    }}>
      <button
        style={{
          width: 24,
          height: 24,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: done ? COLORS.success : COLORS.panel,
          defaultColor: COLORS.black,
          borderRadius: 7,
          borderWidth: 1,
          borderColor: done ? COLORS.success : COLORS.line,
          defaultFontSize: 14,
          ':hover': { borderColor: COLORS.success },
        }}
        onClick={props.onToggle}
      >
        {done ? '✓' : ' '}
      </button>
      <view style={{
        width: 10,
        height: 10,
        backgroundColor: PRIORITY_TONE[props.todo.prio],
        borderRadius: 999,
      }} />
      <view style={{ flexGrow: 1, display: 'flex', flexDirection: 'column' }}>
        <text style={{ color: done ? COLORS.quiet : COLORS.ink, fontSize: 15 }}>{props.todo.text}</text>
      </view>
      <text style={{ color: COLORS.quiet, fontSize: 11 }}>{`優先度 ${PRIORITY_LABEL[props.todo.prio]}`}</text>
      <button
        style={{
          width: 30,
          height: 30,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: COLORS.panel,
          defaultColor: COLORS.muted,
          borderRadius: 8,
          borderWidth: 1,
          borderColor: COLORS.line,
          defaultFontSize: 14,
          ':hover': { backgroundColor: COLORS.dangerBg, borderColor: COLORS.danger, defaultColor: COLORS.danger },
        }}
        onClick={props.onRemove}
      >
        ✕
      </button>
    </view>
  );
}

function EmptyState() {
  return (
    <view style={{
      height: 96,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: COLORS.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <text style={{ color: COLORS.muted, fontSize: 14 }}>表示するタスクがありません</text>
    </view>
  );
}

function Footer(props: { percent: number; onClearDone: () => void }) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
    }}>
      <text style={{ color: COLORS.muted, fontSize: 13 }}>{`${props.percent}% 完了`}</text>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <text style={{ color: COLORS.quiet, fontSize: 11 }}>クリックで完了 / ✕ で削除</text>
        <button
          style={{
            height: 30,
            paddingLeft: 12,
            paddingRight: 12,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: COLORS.panel2,
            defaultColor: COLORS.text,
            borderRadius: 8,
            borderWidth: 1,
            borderColor: COLORS.line,
            defaultFontSize: 12,
            ':hover': { backgroundColor: COLORS.panel3, borderColor: COLORS.line },
          }}
          onClick={props.onClearDone}
        >
          完了を消す
        </button>
      </view>
    </view>
  );
}
