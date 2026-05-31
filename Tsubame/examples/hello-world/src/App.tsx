import { createSignal, createMemo } from 'solid-js';

export type Mode = 'DOM' | 'Canvas';
export type ModeSource = 'query' | 'auto';

interface Todo {
  id: number;
  text: string;
  done: boolean;
}

type Filter = 'all' | 'active' | 'completed';

const PRESETS = [
  '📚 ドキュメント執筆',
  '☕ コーヒーを淹れる',
  '🚶 散歩する',
  '📧 メール返信',
  '🎵 音楽鑑賞',
  '💪 運動',
];

const C = {
  bg:        '#020617',
  header:    '#0f172a',
  headerHi:  '#111b30',
  item:      '#1e293b',
  itemDone:  '#0b1426',
  text:      '#f1f5f9',
  textDim:   '#94a3b8',
  textMuted: '#64748b',
  primary:   '#38bdf8',
  primaryFg: '#021029',
  success:   '#22c55e',
  danger:    '#f87171',
  dangerBg:  '#4c1010',
  chip:      '#1e293b',
};

// HayateStyle に padding が無いので、幅指定の view を spacer として使う。
const SpX = (w: number) => <view style={{ width: w, height: 1 }} />;
const SpY = (h: number) => <view style={{ width: 1, height: h }} />;

export interface TodoAppProps {
  mode: Mode;
  source: ModeSource;
  width: number;
  height: number;
}

export function TodoApp(props: TodoAppProps) {
  const [todos, setTodos] = createSignal<Todo[]>([
    { id: 1, text: '✨ Tsubame で Hello World を描く',     done: true  },
    { id: 2, text: '🎨 DOM Renderer の動作確認',           done: true  },
    { id: 3, text: '⚡ Canvas Renderer (mock) の動作確認', done: true  },
    { id: 4, text: '📝 本格的な TODO デモを書く',          done: false },
    { id: 5, text: '🚀 Hayate 実 WASM を配備',             done: false },
    { id: 6, text: '🔧 padding / border 等を StylePatch に追加', done: false },
  ]);
  const [filter, setFilter] = createSignal<Filter>('all');
  let nextId = 100;

  const filtered = createMemo<Todo[]>(() => {
    const f = filter();
    if (f === 'all') return todos();
    if (f === 'active') return todos().filter((t) => !t.done);
    return todos().filter((t) => t.done);
  });
  const activeCount = createMemo(() => todos().filter((t) => !t.done).length);
  const doneCount = createMemo(() => todos().filter((t) => t.done).length);

  const add = (text: string): void => {
    setTodos([...todos(), { id: nextId++, text, done: false }]);
  };
  const toggle = (id: number): void => {
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  };
  const remove = (id: number): void => {
    setTodos(todos().filter((t) => t.id !== id));
  };
  const clearDone = (): void => {
    setTodos(todos().filter((t) => !t.done));
  };

  const HEADER_H = 64;
  const FILTER_H = 52;
  const FOOTER_H = 104;
  const listH = props.height - HEADER_H - FILTER_H - FOOTER_H;
  const itemW = Math.min(720, props.width - 48);

  return (
    <view style={{
      width: props.width,
      height: props.height,
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: C.bg,
    }}>
      {/* ─── Header ─── */}
      <view style={{
        width: props.width,
        height: HEADER_H,
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        backgroundColor: C.header,
      }}>
        <view style={{
          display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 14,
        }}>
          {SpX(28)}
          <view style={{
            width: 36, height: 36,
            backgroundColor: C.primary, borderRadius: 10,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <text style={{ color: C.primaryFg, fontSize: 20, fontWeight: 700 }}>燕</text>
          </view>
          <text style={{ color: C.text, fontSize: 20, fontWeight: 700 }}>
            Tsubame TODO Board
          </text>
          <text style={{ color: C.textMuted, fontSize: 12 }}>
            — Solid Native Demo
          </text>
        </view>
        <view style={{
          display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10,
        }}>
          <text style={{ color: C.textMuted, fontSize: 11 }}>renderer</text>
          <view style={{
            backgroundColor: C.primary,
            borderRadius: 999,
            height: 24,
            display: 'flex', flexDirection: 'row', alignItems: 'center',
          }}>
            {SpX(12)}
            <text style={{ color: C.primaryFg, fontSize: 12, fontWeight: 700 }}>
              {props.mode}
            </text>
            {SpX(12)}
          </view>
          <text style={{ color: C.textMuted, fontSize: 10 }}>
            {props.source === 'query' ? '?mode' : 'auto'}
          </text>
          {SpX(28)}
        </view>
      </view>

      {/* ─── Filter bar ─── */}
      <view style={{
        width: props.width,
        height: FILTER_H,
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        backgroundColor: C.headerHi,
      }}>
        <view style={{
          display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10,
        }}>
          {SpX(28)}
          <view style={{
            backgroundColor: C.chip, borderRadius: 8,
            height: 28,
            display: 'flex', flexDirection: 'row', alignItems: 'center',
          }}>
            {SpX(10)}
            <text style={{ color: C.text, fontSize: 14, fontWeight: 700 }}>
              {`${activeCount()}`}
            </text>
            {SpX(4)}
            <text style={{ color: C.textMuted, fontSize: 12 }}>active</text>
            {SpX(8)}
            <view style={{ width: 1, height: 14, backgroundColor: C.headerHi }} />
            {SpX(8)}
            <text style={{ color: C.textDim, fontSize: 14, fontWeight: 700 }}>
              {`${doneCount()}`}
            </text>
            {SpX(4)}
            <text style={{ color: C.textMuted, fontSize: 12 }}>done</text>
            {SpX(10)}
          </view>
        </view>

        <view style={{
          display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 6,
        }}>
          {(['all', 'active', 'completed'] as const).map((f) => (
            <button
              style={{
                backgroundColor: filter() === f ? C.primary : C.chip,
                color: filter() === f ? C.primaryFg : C.textDim,
                fontSize: 12,
                fontWeight: 700,
                borderRadius: 999,
                height: 28,
              }}
              onClick={() => setFilter(f)}
            >
              {f === 'all' ? 'すべて' : f === 'active' ? '未完了' : '完了'}
            </button>
          ))}
        </view>

        <view style={{
          display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8,
        }}>
          <button
            style={{
              backgroundColor: doneCount() > 0 ? C.dangerBg : C.chip,
              color: doneCount() > 0 ? C.danger : C.textMuted,
              fontSize: 12,
              fontWeight: 700,
              borderRadius: 8,
              height: 28,
              opacity: doneCount() > 0 ? 1 : 0.5,
            }}
            onClick={clearDone}
          >
            完了をクリア
          </button>
          {SpX(28)}
        </view>
      </view>

      {/* ─── List ─── */}
      <scroll-view style={{
        width: props.width,
        height: listH,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 8,
        backgroundColor: C.bg,
      }}>
        {SpY(16)}
        {filtered().length === 0
          ? <view style={{
              width: itemW,
              height: 80,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: C.headerHi,
              borderRadius: 12,
              opacity: 0.6,
            }}>
              <text style={{ color: C.textMuted, fontSize: 14 }}>
                該当する TODO がありません
              </text>
            </view>
          : filtered().map((todo) => (
            <view style={{
              width: itemW,
              height: 56,
              display: 'flex',
              flexDirection: 'row',
              alignItems: 'center',
              justifyContent: 'space-between',
              backgroundColor: todo.done ? C.itemDone : C.item,
              borderRadius: 12,
              opacity: todo.done ? 0.7 : 1,
            }}>
              <view style={{
                display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 14,
              }}>
                {SpX(14)}
                <button
                  style={{
                    backgroundColor: todo.done ? C.success : C.chip,
                    color: todo.done ? '#ffffff' : C.textMuted,
                    borderRadius: 8,
                    width: 32,
                    height: 32,
                    fontSize: 16,
                    fontWeight: 700,
                  }}
                  onClick={() => toggle(todo.id)}
                >
                  {todo.done ? '✓' : ' '}
                </button>
                <text style={{
                  color: todo.done ? C.textMuted : C.text,
                  fontSize: 15,
                  fontWeight: 500,
                }}>
                  {todo.text}
                </text>
              </view>
              <view style={{
                display: 'flex', flexDirection: 'row', alignItems: 'center',
              }}>
                <button
                  style={{
                    backgroundColor: C.dangerBg,
                    color: C.danger,
                    borderRadius: 8,
                    height: 28,
                    fontSize: 12,
                    fontWeight: 700,
                  }}
                  onClick={() => remove(todo.id)}
                >
                  削除
                </button>
                {SpX(14)}
              </view>
            </view>
          ))}
        {SpY(16)}
      </scroll-view>

      {/* ─── Footer: preset add ─── */}
      <view style={{
        width: props.width,
        height: FOOTER_H,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: C.header,
        gap: 12,
      }}>
        <text style={{ color: C.textDim, fontSize: 12, fontWeight: 600 }}>
          + プリセットから追加
        </text>
        <view style={{
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
        }}>
          {PRESETS.map((p) => (
            <button
              style={{
                backgroundColor: C.chip,
                color: C.text,
                fontSize: 13,
                fontWeight: 500,
                borderRadius: 999,
                height: 34,
              }}
              onClick={() => add(p)}
            >
              {p}
            </button>
          ))}
        </view>
      </view>
    </view>
  );
}
