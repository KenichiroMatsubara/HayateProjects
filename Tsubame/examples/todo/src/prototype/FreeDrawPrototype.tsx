import { createEffect, createSignal, onCleanup, onMount } from 'solid-js';
import { AddForm } from '../components/AddForm';
import { Footer } from '../components/TaskCard';
import { TodoRow } from '../components/TodoRow';
import { Toolbar } from '../components/Toolbar';
import type { Palette } from '../theme';
import type { Filter, Priority, SortMode, Todo } from '../todo-model';
import { priorityTone } from '../ui/styles';
import {
  CONSTELLATION_POINTS,
  constellationPainter,
  focusOrbPainter,
  orbitPainter,
} from './skia-painters';

/**
 * PROTOTYPE — SkiaSafeの自由描画をTodoの情報構造へ組み込む3案。
 * 既存 tasks 画面上で `?variant=A|B|C` により切替可能。採用後は勝者だけを本実装へ書き直す。
 */
type Variant = 'A' | 'B' | 'C';

export interface FreeDrawPrototypeProps {
  colors: Palette;
  todos: readonly Todo[];
  filter: Filter;
  sort: SortMode;
  draft: string;
  draftPrio: Priority;
  editingId: number | null;
  editDraft: string;
  summary: { total: number; remaining: number; percent: number };
  onDraft: (value: string) => void;
  onDraftPrio: (value: Priority) => void;
  onAdd: () => void;
  onFilter: (value: Filter) => void;
  onSort: (value: SortMode) => void;
  onToggle: (id: number) => void;
  onRemove: (id: number) => void;
  onBeginEdit: (todo: Todo) => void;
  onEditInput: (value: string) => void;
  onCommitEdit: () => void;
  onCancelEdit: () => void;
  onMoveUp: (id: number) => void;
  onMoveDown: (id: number) => void;
  onClearDone: () => void;
}

const VARIANTS: readonly Variant[] = ['A', 'B', 'C'];
const VARIANT_NAMES: Record<Variant, string> = {
  A: 'Orbit timeline',
  B: 'Focus orb',
  C: 'Constellation',
};

const INVERSE_INK = '#f5f7ff';
const INVERSE_MUTED = '#9aa6bc';
const IS_DEV = (import.meta as ImportMeta & { env?: { DEV?: boolean } }).env?.DEV ?? false;

function initialVariant(): Variant {
  const value = new URLSearchParams(window.location.search).get('variant');
  return value === 'B' || value === 'C' ? value : 'A';
}

function SharedControls(props: FreeDrawPrototypeProps) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <AddForm
        colors={props.colors}
        draft={props.draft}
        prio={props.draftPrio}
        onInput={props.onDraft}
        onPrio={props.onDraftPrio}
        onAdd={props.onAdd}
      />
      <Toolbar colors={props.colors} filter={props.filter} sort={props.sort} onFilter={props.onFilter} onSort={props.onSort} />
    </view>
  );
}

function Rows(props: FreeDrawPrototypeProps) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      {props.todos.map((todo) => (
        <TodoRow
          colors={props.colors}
          todo={todo}
          reorderable={props.sort === 'manual'}
          editing={props.editingId === todo.id}
          editDraft={props.editDraft}
          onToggle={() => props.onToggle(todo.id)}
          onRemove={() => props.onRemove(todo.id)}
          onBeginEdit={() => props.onBeginEdit(todo)}
          onEditInput={props.onEditInput}
          onCommitEdit={props.onCommitEdit}
          onCancelEdit={props.onCancelEdit}
          onMoveUp={() => props.onMoveUp(todo.id)}
          onMoveDown={() => props.onMoveDown(todo.id)}
        />
      ))}
    </view>
  );
}

function VariantA(props: FreeDrawPrototypeProps) {
  const completed = props.summary.total - props.summary.remaining;
  return (
    <view style={{ width: 720, maxWidth: '100%', display: 'flex', flexDirection: 'column', gap: 16 }}>
      <view style={{ height: 310, position: 'relative', overflow: 'hidden', borderRadius: 28 }}>
        <view draw={orbitPainter(props.colors, completed, props.summary.total)} style={{ position: 'absolute', width: '100%', height: '100%' }} />
        <view style={{ position: 'absolute', left: 30, top: 28, display: 'flex', flexDirection: 'column', gap: 6 }}>
          <text style={{ color: props.colors.muted, fontSize: 12 }}>TODAY / FLIGHT PATH</text>
          <text style={{ color: props.colors.ink, fontSize: 34, fontWeight: 700 }}>流れをつくる。</text>
          <text style={{ color: props.colors.muted, fontSize: 14 }}>{`${completed}個の通過点 · あと${props.summary.remaining}個`}</text>
        </view>
        <view style={{ position: 'absolute', right: 26, bottom: 24, width: 174, padding: 16, backgroundColor: props.colors.black, borderRadius: 18 }}>
          <text style={{ color: props.colors.accent, fontSize: 11 }}>MOMENTUM</text>
          <text style={{ color: INVERSE_INK, fontSize: 30, fontWeight: 700 }}>{`${props.summary.percent}%`}</text>
          <text style={{ color: INVERSE_MUTED, fontSize: 11 }}>軌道は完了とともに光る</text>
        </view>
      </view>
      <view style={{ padding: 18, display: 'flex', flexDirection: 'column', gap: 12, backgroundColor: props.colors.panel, borderRadius: 22 }}>
        <SharedControls {...props} />
        <Rows {...props} />
        <Footer colors={props.colors} percent={props.summary.percent} onClearDone={props.onClearDone} />
      </view>
    </view>
  );
}

function VariantB(props: FreeDrawPrototypeProps) {
  const next = props.todos.find((todo) => !todo.done) ?? props.todos[0];
  const rest = props.todos.filter((todo) => todo.id !== next?.id);
  return (
    <view style={{ width: 820, maxWidth: '100%', display: 'flex', flexDirection: 'column', gap: 18 }}>
      <view style={{ display: 'flex', flexDirection: 'row', flexWrap: 'wrap', gap: 18 }}>
        <view style={{ width: 360, height: 360, position: 'relative', backgroundColor: props.colors.panel, borderRadius: 32 }}>
          <view draw={focusOrbPainter(props.colors, props.summary.percent)} style={{ position: 'absolute', width: '100%', height: '100%' }} />
          <view style={{ position: 'absolute', left: 90, top: 118, width: 180, alignItems: 'center', display: 'flex', flexDirection: 'column', gap: 4 }}>
            <text style={{ color: props.colors.muted, fontSize: 11 }}>TODAY'S SIGNAL</text>
            <text style={{ color: props.colors.ink, fontSize: 48, fontWeight: 700 }}>{`${props.summary.percent}`}</text>
            <text style={{ color: props.colors.accent, fontSize: 12 }}>PERCENT ALIGNED</text>
          </view>
        </view>
        <view style={{ flexGrow: 1, minWidth: 280, padding: 24, display: 'flex', flexDirection: 'column', justifyContent: 'space-between', gap: 18, backgroundColor: props.colors.black, borderRadius: 32 }}>
          <view style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
            <text style={{ color: props.colors.accent, fontSize: 11 }}>NEXT / ONE THING</text>
            <text style={{ color: INVERSE_INK, fontSize: 28, fontWeight: 700 }}>{next?.text ?? 'すべて完了'}</text>
            <text style={{ color: INVERSE_MUTED, fontSize: 13 }}>一覧を管理する前に、次の一歩だけを見る。</text>
          </view>
          {next ? <button
            onClick={() => props.onToggle(next.id)}
            style={{ height: 54, backgroundColor: props.colors.accent, defaultColor: props.colors.black, borderRadius: 18, fontSize: 15, fontWeight: 700 }}
          >完了して軌道へ送る →</button> : null}
        </view>
      </view>
      <view style={{ padding: 20, display: 'flex', flexDirection: 'column', gap: 14, backgroundColor: props.colors.panel, borderRadius: 24 }}>
        <SharedControls {...props} />
        <text style={{ color: props.colors.muted, fontSize: 11 }}>UP NEXT · {rest.length}</text>
        <view style={{ display: 'flex', flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
          {rest.map((todo) => (
            <button
              onClick={() => props.onToggle(todo.id)}
              style={{ minWidth: 180, flexGrow: 1, padding: 16, display: 'flex', flexDirection: 'column', alignItems: 'flex-start', gap: 7, backgroundColor: props.colors.panel2, defaultColor: props.colors.text, borderRadius: 18 }}
            >
              <view style={{ width: 8, height: 8, borderRadius: 99, backgroundColor: priorityTone(props.colors, todo.prio) }} />
              <text style={{ color: todo.done ? props.colors.quiet : props.colors.ink, fontSize: 14 }}>{todo.text}</text>
              <text style={{ color: props.colors.quiet, fontSize: 10 }}>{todo.done ? 'ARCHIVED IN ORBIT' : 'TAP TO COMPLETE'}</text>
            </button>
          ))}
        </view>
      </view>
    </view>
  );
}

function VariantC(props: FreeDrawPrototypeProps) {
  const nodes = props.todos.slice(0, CONSTELLATION_POINTS.length);
  return (
    <view style={{ width: 860, maxWidth: '100%', display: 'flex', flexDirection: 'column', gap: 16 }}>
      <view style={{ height: 520, position: 'relative', overflow: 'hidden', borderRadius: 28 }}>
        <view
          draw={constellationPainter(props.colors, nodes.map((todo) => todo.prio), nodes.map((todo) => todo.done))}
          style={{ position: 'absolute', width: '100%', height: '100%' }}
        />
        <view style={{ position: 'absolute', left: 28, top: 24, display: 'flex', flexDirection: 'column', gap: 4 }}>
          <text style={{ color: props.colors.accent, fontSize: 11 }}>TASK CONSTELLATION / LIVE</text>
          <text style={{ color: INVERSE_INK, fontSize: 30, fontWeight: 700 }}>今日という星図</text>
          <text style={{ color: INVERSE_MUTED, fontSize: 12 }}>色は重力（優先度）、光は完了を表す</text>
        </view>
        {nodes.map((todo, index) => {
          const point = CONSTELLATION_POINTS[index]!;
          return <button
            onClick={() => props.onToggle(todo.id)}
            style={{
              position: 'absolute',
              left: `${Math.max(3, point.x - 8)}%`,
              top: `${Math.max(12, point.y - 2)}%`,
              maxWidth: 190,
              paddingTop: 6,
              paddingBottom: 6,
              paddingLeft: 10,
              paddingRight: 10,
              backgroundColor: props.colors.black,
              defaultColor: todo.done ? props.colors.success : INVERSE_INK,
              borderRadius: 10,
              borderWidth: 1,
              borderStyle: 'solid',
              borderColor: todo.done ? props.colors.success : priorityTone(props.colors, todo.prio),
              fontSize: 11,
            }}
          >{todo.text}</button>;
        })}
        <view style={{ position: 'absolute', right: 22, bottom: 18, padding: 12, backgroundColor: props.colors.panel, borderRadius: 14 }}>
          <text style={{ color: props.colors.ink, fontSize: 12 }}>{`${props.summary.remaining} UNRESOLVED SIGNALS`}</text>
        </view>
      </view>
      <view style={{ padding: 18, display: 'flex', flexDirection: 'column', gap: 8, backgroundColor: props.colors.panel, borderRadius: 22 }}>
        <SharedControls {...props} />
        <text style={{ color: props.colors.muted, fontSize: 11 }}>星をタップして完了 · 7件目以降はフィルターに残る</text>
      </view>
    </view>
  );
}

function PrototypeSwitcher(props: { variant: Variant; onChange: (value: Variant) => void; colors: Palette }) {
  const move = (delta: number) => {
    const index = VARIANTS.indexOf(props.variant);
    props.onChange(VARIANTS[(index + delta + VARIANTS.length) % VARIANTS.length]!);
  };

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) return;
      if (event.key === 'ArrowLeft') move(-1);
      if (event.key === 'ArrowRight') move(1);
    };
    window.addEventListener('keydown', onKey);
    onCleanup(() => window.removeEventListener('keydown', onKey));
  });

  return (
    <view style={{ position: 'absolute', left: '50%', bottom: 16, width: 330, marginLeft: -165, height: 48, padding: 5, display: 'flex', flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', backgroundColor: props.colors.black, borderRadius: 999, borderWidth: 1, borderStyle: 'solid', borderColor: props.colors.accent }}>
      <button onClick={() => move(-1)} style={{ width: 38, height: 38, backgroundColor: props.colors.panel3, defaultColor: props.colors.ink, borderRadius: 99 }}>←</button>
      <text style={{ color: INVERSE_INK, fontSize: 12 }}>{`${props.variant} — ${VARIANT_NAMES[props.variant]}`}</text>
      <button onClick={() => move(1)} style={{ width: 38, height: 38, backgroundColor: props.colors.accent, defaultColor: props.colors.black, borderRadius: 99 }}>→</button>
    </view>
  );
}

export function FreeDrawPrototype(props: FreeDrawPrototypeProps) {
  const [variant, setVariant] = createSignal<Variant>(initialVariant());

  createEffect(() => {
    // Native Host / 単体テストの location は URL ではない。共有可能な query は
    // ブラウザ上の開発プロトタイプだけの能力として閉じる。
    if (!IS_DEV || !/^https?:\/\//.test(window.location.href)) return;
    const url = new URL(window.location.href);
    url.searchParams.set('variant', variant());
    window.history.replaceState(null, '', url);
  });

  return (
    <view style={{ flexGrow: 1, width: '100%', height: 0, minHeight: 0, position: 'relative', overflow: 'hidden', backgroundColor: props.colors.bg }}>
      <scroll-view style={{ width: '100%', height: '100%', paddingTop: 22, paddingBottom: 22, paddingLeft: 16, paddingRight: 16, display: 'flex', flexDirection: 'column', alignItems: 'center', backgroundColor: props.colors.bg }}>
        {variant() === 'A' ? <VariantA {...props} /> : null}
        {variant() === 'B' ? <VariantB {...props} /> : null}
        {variant() === 'C' ? <VariantC {...props} /> : null}
      </scroll-view>
      {IS_DEV
        ? <PrototypeSwitcher variant={variant()} onChange={setVariant} colors={props.colors} />
        : null}
    </view>
  );
}
