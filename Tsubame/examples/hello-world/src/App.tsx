import { createMemo, createSignal } from 'solid-js';
import { CssGallery } from './CssGallery';
import type { DetectModeResult } from './detect-mode';
import { COLORS, inputStyle } from './theme';

type Page = 'tasks' | 'gallery';
type Status = 'todo' | 'doing' | 'done';
type Filter = 'all' | Status;
type Priority = 'P0' | 'P1' | 'P2';

interface Todo {
  id: number;
  title: string;
  detail: string;
  project: string;
  priority: Priority;
  status: Status;
  due: string;
  estimate: number;
}

export interface TodoAppProps {
  detected: DetectModeResult;
}

const STATUS_LABEL: Record<Status, string> = {
  todo: 'Ready',
  doing: 'Doing',
  done: 'Done',
};

const STATUS_COLOR: Record<Status, string> = {
  todo: COLORS.blue,
  doing: COLORS.accent2,
  done: COLORS.success,
};

const PRIORITY_COLOR: Record<Priority, string> = {
  P0: COLORS.danger,
  P1: COLORS.accent2,
  P2: COLORS.accent,
};

const BASE_TODOS: Todo[] = [
  {
    id: 1,
    title: 'Ship renderer mode switcher',
    detail: 'Keep DOM, Canvas, and auto detection visible while the app is running.',
    project: 'Tsubame',
    priority: 'P1',
    status: 'done',
    due: 'Today',
    estimate: 1,
  },
  {
    id: 2,
    title: 'Polish the task board surface',
    detail: 'Use nested panels, counters, hover states, and clear task actions.',
    project: 'Demo UX',
    priority: 'P0',
    status: 'doing',
    due: 'Today',
    estimate: 3,
  },
  {
    id: 3,
    title: 'Exercise pseudo-style hover',
    detail: 'Highlight cards and action buttons through Hayate CSS :hover blocks.',
    project: 'Events',
    priority: 'P1',
    status: 'doing',
    due: 'Next',
    estimate: 2,
  },
  {
    id: 4,
    title: 'Prepare real Hayate integration notes',
    detail: 'Record the adapter gaps without touching the runtime from this demo pass.',
    project: 'Hayate',
    priority: 'P2',
    status: 'todo',
    due: 'Soon',
    estimate: 2,
  },
  {
    id: 5,
    title: 'Browse the CSS Gallery',
    detail: 'Verify all 40 HayateStyle properties render in DOM and Canvas modes.',
    project: 'Workflow',
    priority: 'P2',
    status: 'todo',
    due: 'Later',
    estimate: 1,
  },
];

function buildInitialTodos(): Todo[] {
  const extras: Todo[] = Array.from({ length: 14 }, (_, index) => ({
    id: 10 + index,
    title: `Scroll stress task #${index + 1}`,
    detail: 'Enough list height to exercise scroll-view in the all filter.',
    project: 'Layout',
    priority: (index % 3 === 0 ? 'P0' : index % 2 === 0 ? 'P1' : 'P2') as Priority,
    status: (index % 4 === 0 ? 'done' : index % 3 === 0 ? 'doing' : 'todo') as Status,
    due: index % 2 === 0 ? 'Today' : 'Next',
    estimate: (index % 3) + 1,
  }));
  return [...BASE_TODOS, ...extras];
}

const INITIAL_TODOS = buildInitialTodos();

const SpX = (w: number) => <view style={{ width: w, height: 1 }} />;
const SpY = (h: number) => <view style={{ width: 1, height: h }} />;

function nextStatus(status: Status): Status {
  if (status === 'todo') return 'doing';
  if (status === 'doing') return 'done';
  return 'todo';
}

function statusAction(status: Status): string {
  if (status === 'todo') return 'Start';
  if (status === 'doing') return 'Finish';
  return 'Reopen';
}

function rendererBadge(detected: DetectModeResult): string {
  if (detected.mode === 'DOM') return 'DOM';
  return detected.backend ?? 'Canvas';
}

export function TodoApp(props: TodoAppProps) {
  const initialPage: Page =
    new URLSearchParams(window.location.search).get('page') === 'gallery' ? 'gallery' : 'tasks';
  const [page, setPage] = createSignal<Page>(initialPage);
  const [todos, setTodos] = createSignal<Todo[]>(INITIAL_TODOS);
  const [filter, setFilter] = createSignal<Filter>('all');
  const [selectedId, setSelectedId] = createSignal(2);
  const [draftTitle, setDraftTitle] = createSignal('');
  let nextId = 200;

  const activeTodos = createMemo(() => todos().filter((todo) => todo.status !== 'done'));
  const doneTodos = createMemo(() => todos().filter((todo) => todo.status === 'done'));
  const doingTodos = createMemo(() => todos().filter((todo) => todo.status === 'doing'));
  const totalEstimate = createMemo(() => activeTodos().reduce((sum, todo) => sum + todo.estimate, 0));
  const completion = createMemo(() => {
    const total = todos().length;
    return total === 0 ? 0 : Math.round((doneTodos().length / total) * 100);
  });
  const visibleTodos = createMemo(() => {
    const current = filter();
    if (current === 'all') return todos();
    return todos().filter((todo) => todo.status === current);
  });
  const selected = createMemo(() => {
    const current = todos().find((todo) => todo.id === selectedId());
    return current ?? todos()[0] ?? null;
  });

  const addTask = () => {
    const title = draftTitle().trim();
    if (!title) return;
    const id = nextId++;
    setTodos([{
      id,
      title,
      detail: 'Added from the quick-add form.',
      project: 'Inbox',
      priority: 'P2',
      status: 'todo',
      due: 'Today',
      estimate: 1,
    }, ...todos()]);
    setSelectedId(id);
    setFilter('all');
    setDraftTitle('');
  };

  const updateSelected = (patch: Partial<Pick<Todo, 'title' | 'detail'>>) => {
    const id = selectedId();
    setTodos(todos().map((todo) => (
      todo.id === id ? { ...todo, ...patch } : todo
    )));
  };

  const advance = (id: number) => {
    setTodos(todos().map((todo) => (
      todo.id === id ? { ...todo, status: nextStatus(todo.status) } : todo
    )));
    setSelectedId(id);
  };

  const remove = (id: number) => {
    const remaining = todos().filter((todo) => todo.id !== id);
    setTodos(remaining);
    if (selectedId() === id) setSelectedId(remaining[0]?.id ?? 0);
  };

  const clearDone = () => {
    const remaining = todos().filter((todo) => todo.status !== 'done');
    setTodos(remaining);
    if (selected()?.status === 'done') setSelectedId(remaining[0]?.id ?? 0);
  };

  const controlStyle = (active = false) => ({
    height: 34,
    paddingLeft: 14,
    paddingRight: 14,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? COLORS.accent : COLORS.panel,
    defaultColor: active ? COLORS.black : COLORS.text,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: active ? COLORS.accent : COLORS.line,
    defaultFontSize: 13,
    ':hover': {
      backgroundColor: active ? COLORS.accent : COLORS.panel3,
      borderColor: active ? COLORS.accent : COLORS.line,
    },
  });

  const pageToggleStyle = (active: boolean) => controlStyle(active);

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
      <view style={{
        height: 72,
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        backgroundColor: COLORS.rail,
        borderWidth: 1,
        borderColor: COLORS.line,
      }}>
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 14 }}>
          {SpX(28)}
          <view style={{
            width: 42,
            height: 42,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: COLORS.accent,
            borderRadius: 8,
          }}>
            <text style={{ fontSize: 19, color: COLORS.black }}>TS</text>
          </view>
          <view style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
            <text style={{ fontSize: 22, color: COLORS.ink }}>Tsubame Task Studio</text>
            <text style={{ fontSize: 12, color: COLORS.muted }}>Todo demo + Hayate CSS gallery</text>
          </view>
        </view>

        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <button style={pageToggleStyle(page() === 'tasks')} onClick={() => setPage('tasks')}>
            Tasks
          </button>
          <button style={pageToggleStyle(page() === 'gallery')} onClick={() => setPage('gallery')}>
            CSS Gallery
          </button>
          <text style={{ color: COLORS.quiet, fontSize: 11 }}>renderer</text>
          <view style={{
            height: 28,
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            backgroundColor: COLORS.panel,
            borderRadius: 8,
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
          {SpX(28)}
        </view>
      </view>

      {page() === 'gallery'
        ? <CssGallery />
        : <>
          <view style={{
            height: 70,
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            justifyContent: 'space-between',
            backgroundColor: COLORS.panel,
          }}>
            <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
              {SpX(28)}
              <Metric label="active" value={`${activeTodos().length}`} tone={COLORS.blue} />
              <Metric label="doing" value={`${doingTodos().length}`} tone={COLORS.accent2} />
              <Metric label="done" value={`${doneTodos().length}`} tone={COLORS.success} />
              <Metric label="hours left" value={`${totalEstimate()}`} tone={COLORS.violet} />
            </view>

            <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              {(['all', 'todo', 'doing', 'done'] as const).map((item) => (
                <button
                  style={controlStyle(filter() === item)}
                  onClick={() => setFilter(item)}
                >
                  {item === 'all' ? 'All' : STATUS_LABEL[item]}
                </button>
              ))}
              <button
                style={controlStyle(false)}
                onClick={clearDone}
              >
                Clear done
              </button>
              {SpX(28)}
            </view>
          </view>

          <view style={{
            flexGrow: 1,
            display: 'flex',
            flexDirection: 'row',
            backgroundColor: COLORS.bg,
          }}>
            <scroll-view style={{
              width: '67%',
              height: '100%',
              display: 'flex',
              flexDirection: 'column',
              gap: 10,
              paddingTop: 18,
              paddingLeft: 28,
              paddingRight: 16,
              paddingBottom: 18,
            }}>
              <view style={{
                height: 54,
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                justifyContent: 'space-between',
                backgroundColor: COLORS.panel,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: COLORS.line,
              }}>
                <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
                  {SpX(16)}
                  <text style={{ color: COLORS.ink, fontSize: 16 }}>Task queue</text>
                  <text style={{ color: COLORS.muted, fontSize: 12 }}>{`${visibleTodos().length} visible`}</text>
                </view>
                <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                  <text style={{ color: COLORS.quiet, fontSize: 12 }}>completion</text>
                  <Progress percent={completion()} />
                  {SpX(16)}
                </view>
              </view>

              {visibleTodos().length === 0
                ? <EmptyState />
                : visibleTodos().map((todo) => (
                  <TodoCard
                    todo={todo}
                    selected={selectedId() === todo.id}
                    onSelect={() => setSelectedId(todo.id)}
                    onAdvance={() => advance(todo.id)}
                    onRemove={() => remove(todo.id)}
                  />
                ))}
              {SpY(12)}
            </scroll-view>

            <view style={{
              width: '33%',
              height: '100%',
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
              paddingTop: 18,
              paddingRight: 28,
              paddingBottom: 18,
              paddingLeft: 8,
            }}>
              <DetailPanel
                todo={selected()}
                onUpdateTitle={(title) => updateSelected({ title })}
                onUpdateDetail={(detail) => updateSelected({ detail })}
              />
              <view style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 10,
                backgroundColor: COLORS.panel,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: COLORS.line,
                padding: 14,
              }}>
                <text style={{ color: COLORS.ink, fontSize: 16 }}>Quick add</text>
                <text style={{ color: COLORS.muted, fontSize: 12 }}>Type a title and press Add or Enter.</text>
                <text-input
                  value={draftTitle()}
                  placeholder="New task title"
                  style={inputStyle}
                  onInput={(event) => setDraftTitle(event.value ?? '')}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') addTask();
                  }}
                />
                <button
                  style={{
                    height: 38,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    backgroundColor: COLORS.accent,
                    defaultColor: COLORS.black,
                    borderRadius: 8,
                    borderWidth: 1,
                    borderColor: COLORS.accent,
                    defaultFontSize: 13,
                    ':hover': {
                      backgroundColor: COLORS.success,
                      borderColor: COLORS.success,
                    },
                  }}
                  onClick={addTask}
                >
                  Add
                </button>
              </view>
            </view>
          </view>
        </>}
    </view>
  );
}

function Metric(props: { label: string; value: string; tone: string }) {
  return (
    <view style={{
      height: 42,
      minWidth: 104,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'center',
      gap: 8,
      backgroundColor: COLORS.panel2,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <text style={{ color: props.tone, fontSize: 18 }}>{props.value}</text>
      <text style={{ color: COLORS.muted, fontSize: 12 }}>{props.label}</text>
    </view>
  );
}

function Progress(props: { percent: number }) {
  const filled = Math.max(2, Math.round(props.percent * 1.2));
  return (
    <view style={{
      width: 126,
      height: 18,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      backgroundColor: COLORS.black,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <view style={{ width: filled, height: 10, marginLeft: 4, backgroundColor: COLORS.success, borderRadius: 6 }} />
      <text style={{ width: 42, color: COLORS.muted, fontSize: 11 }}>{`${props.percent}%`}</text>
    </view>
  );
}

function TodoCard(props: {
  todo: Todo;
  selected: boolean;
  onSelect: () => void;
  onAdvance: () => void;
  onRemove: () => void;
}) {
  return (
    <view
      style={{
        minHeight: 118,
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
        padding: 14,
        backgroundColor: props.selected ? COLORS.panel3 : COLORS.panel,
        borderRadius: 8,
        borderWidth: 1,
        borderColor: props.selected ? COLORS.accent : COLORS.line,
        opacity: props.todo.status === 'done' ? 0.78 : 1,
        ':hover': props.selected
          ? { backgroundColor: COLORS.panel3, borderColor: COLORS.accent }
          : { backgroundColor: COLORS.panel2, borderColor: COLORS.blue },
      }}
      onClick={props.onSelect}
    >
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <Badge label={props.todo.priority} color={PRIORITY_COLOR[props.todo.priority]} />
          <Badge label={STATUS_LABEL[props.todo.status]} color={STATUS_COLOR[props.todo.status]} />
          <text style={{ color: COLORS.muted, fontSize: 12 }}>{props.todo.project}</text>
        </view>
        <text style={{ color: COLORS.quiet, fontSize: 12 }}>{`${props.todo.estimate}h - ${props.todo.due}`}</text>
      </view>

      <text style={{ color: COLORS.ink, fontSize: 17 }}>{props.todo.title}</text>
      <text style={{ color: COLORS.muted, fontSize: 13 }}>{props.todo.detail}</text>

      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <text style={{ color: COLORS.quiet, fontSize: 12 }}>Select for details</text>
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <button
            style={{
              height: 32,
              paddingLeft: 14,
              paddingRight: 14,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: COLORS.panel3,
              defaultColor: COLORS.text,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: COLORS.line,
              defaultFontSize: 12,
              ':hover': {
                backgroundColor: COLORS.accent,
                borderColor: COLORS.accent,
                defaultColor: COLORS.black,
              },
            }}
            onClick={props.onAdvance}
          >
            {statusAction(props.todo.status)}
          </button>
          <button
            style={{
              width: 34,
              height: 32,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: COLORS.panel3,
              defaultColor: COLORS.muted,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: COLORS.line,
              defaultFontSize: 15,
              ':hover': {
                backgroundColor: COLORS.dangerBg,
                borderColor: COLORS.danger,
                defaultColor: COLORS.danger,
              },
            }}
            onClick={props.onRemove}
          >
            x
          </button>
        </view>
      </view>
    </view>
  );
}

function Badge(props: { label: string; color: string }) {
  return (
    <view style={{
      height: 24,
      minWidth: 50,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: COLORS.black,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: props.color,
    }}>
      {/* color は block（view）を貫通しないため text に明示指定する（2チャネル継承） */}
      <text style={{ fontSize: 11, color: props.color }}>{props.label}</text>
    </view>
  );
}

function DetailPanel(props: {
  todo: Todo | null;
  onUpdateTitle: (title: string) => void;
  onUpdateDetail: (detail: string) => void;
}) {
  const todo = props.todo;
  return (
    <view style={{
      minHeight: 248,
      display: 'flex',
      flexDirection: 'column',
      gap: 12,
      backgroundColor: COLORS.panel,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
      padding: 16,
    }}>
      <text style={{ color: COLORS.ink, fontSize: 16 }}>Selected task</text>
      {todo === null
        ? <text style={{ color: COLORS.muted, fontSize: 13 }}>No task selected.</text>
        : <>
          <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
            <Badge label={todo.priority} color={PRIORITY_COLOR[todo.priority]} />
            <Badge label={STATUS_LABEL[todo.status]} color={STATUS_COLOR[todo.status]} />
          </view>
          <text-input
            value={todo.title}
            style={inputStyle}
            onInput={(event) => props.onUpdateTitle(event.value ?? '')}
          />
          <text-input
            value={todo.detail}
            style={{ ...inputStyle, height: 72 }}
            onInput={(event) => props.onUpdateDetail(event.value ?? '')}
          />
          <view style={{ height: 1, backgroundColor: COLORS.line }} />
          <InfoRow label="Project" value={todo.project} />
          <InfoRow label="Due" value={todo.due} />
          <InfoRow label="Estimate" value={`${todo.estimate}h`} />
        </>}
    </view>
  );
}

function InfoRow(props: { label: string; value: string }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
      <text style={{ color: COLORS.quiet, fontSize: 12 }}>{props.label}</text>
      <text style={{ color: COLORS.text, fontSize: 13 }}>{props.value}</text>
    </view>
  );
}

function EmptyState() {
  return (
    <view style={{
      height: 136,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: COLORS.panel,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <text style={{ color: COLORS.muted, fontSize: 14 }}>No tasks match this filter.</text>
    </view>
  );
}
