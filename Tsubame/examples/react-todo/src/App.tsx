import { useMemo, useState } from 'react';
import type { HayateCssStyle } from '@tsubame/renderer-protocol';

interface Todo {
  id: number;
  text: string;
  done: boolean;
}

const SEED: readonly Todo[] = [
  { id: 1, text: 'tsubame-react のデモを動かす', done: true },
  { id: 2, text: 'view / text / button を組む', done: false },
  { id: 3, text: 'text-input で新規タスクを追加', done: false },
];

// 単一カードの落ち着いたダークパレット（デモ用にハードコード）。
const C = {
  bg: '#0b1020',
  panel: '#141a2e',
  line: '#26304a',
  ink: '#e7edf8',
  muted: '#8a97b3',
  accent: '#14b8a6',
  black: '#06140f',
  danger: '#f0648c',
} as const;

export function App() {
  const [todos, setTodos] = useState<Todo[]>(() => SEED.map((t) => ({ ...t })));
  const [draft, setDraft] = useState('');
  const [nextId, setNextId] = useState(1000);

  const remaining = useMemo(() => todos.filter((t) => !t.done).length, [todos]);

  const add = () => {
    const text = draft.trim();
    if (!text) return;
    setTodos((prev) => [{ id: nextId, text, done: false }, ...prev]);
    setNextId((n) => n + 1);
    setDraft('');
  };

  const toggle = (id: number) =>
    setTodos((prev) => prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)));

  const remove = (id: number) => setTodos((prev) => prev.filter((t) => t.id !== id));

  const clearDone = () => setTodos((prev) => prev.filter((t) => !t.done));

  return (
    <view style={shell}>
      <scroll-view style={page}>
        <view style={card}>
          <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'baseline', gap: 10 }}>
            <text style={{ defaultColor: C.ink, defaultFontSize: 22, fontWeight: 700 }}>
              React TODO
            </text>
            <text style={{ defaultColor: C.muted, defaultFontSize: 13 }}>
              {`残り ${remaining} / ${todos.length} 件`}
            </text>
          </view>

          <view style={{ display: 'flex', flexDirection: 'row', gap: 8 }}>
            <view style={{ flexGrow: 1 }}>
              <text-input
                value={draft}
                placeholder="新しいタスクを入力…"
                style={input}
                onInput={(e) => setDraft(e.value ?? '')}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') add();
                }}
              />
            </view>
            <button style={primaryBtn} onClick={add}>
              追加
            </button>
          </view>

          <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {todos.length === 0 ? (
              <text style={{ defaultColor: C.muted, defaultFontSize: 14 }}>
                タスクはありません 🎉
              </text>
            ) : (
              todos.map((todo) => (
                <view key={todo.id} style={row}>
                  <button style={check(todo.done)} onClick={() => toggle(todo.id)}>
                    {todo.done ? '✓' : ''}
                  </button>
                  <view style={{ flexGrow: 1 }} onClick={() => toggle(todo.id)}>
                    <text style={label(todo.done)}>{todo.text}</text>
                  </view>
                  <button style={removeBtn} onClick={() => remove(todo.id)}>
                    ×
                  </button>
                </view>
              ))
            )}
          </view>

          <view style={{ height: 1, backgroundColor: C.line }} />

          <view style={{ display: 'flex', flexDirection: 'row', justifyContent: 'flex-end' }}>
            <button style={ghostBtn} onClick={clearDone}>
              完了済みを削除
            </button>
          </view>
        </view>
      </scroll-view>
    </view>
  );
}

const shell: HayateCssStyle = {
  width: '100%',
  height: '100%',
  display: 'flex',
  flexDirection: 'column',
  backgroundColor: C.bg,
  defaultColor: C.ink,
  defaultFontSize: 14,
  defaultFontFamily: 'Inter, Segoe UI, system-ui, sans-serif',
};

const page: HayateCssStyle = {
  flexGrow: 1,
  width: '100%',
  height: '100%',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  paddingTop: 36,
  paddingBottom: 36,
  paddingLeft: 16,
  paddingRight: 16,
};

const card: HayateCssStyle = {
  width: 520,
  maxWidth: '100%',
  display: 'flex',
  flexDirection: 'column',
  gap: 16,
  padding: 22,
  backgroundColor: C.panel,
  borderRadius: 16,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
  boxShadow: [{ offsetX: 0, offsetY: 18, blur: 40, spread: -8, color: '#00000066', inset: false }],
};

const input: HayateCssStyle = {
  width: '100%',
  height: 40,
  paddingLeft: 12,
  paddingRight: 12,
  backgroundColor: C.bg,
  defaultColor: C.ink,
  defaultFontSize: 14,
  borderRadius: 9,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
};

const primaryBtn: HayateCssStyle = {
  height: 40,
  paddingLeft: 18,
  paddingRight: 18,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: C.accent,
  defaultColor: C.black,
  defaultFontSize: 13,
  fontWeight: 700,
  borderRadius: 9,
  cursor: 'pointer',
  ':hover': { opacity: 0.9 },
};

const row: HayateCssStyle = {
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  gap: 10,
  padding: 10,
  backgroundColor: C.bg,
  borderRadius: 10,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
};

const check = (done: boolean): HayateCssStyle => ({
  width: 24,
  height: 24,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: done ? C.accent : 'transparent',
  defaultColor: C.black,
  defaultFontSize: 14,
  fontWeight: 700,
  borderRadius: 6,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: done ? C.accent : C.line,
  cursor: 'pointer',
});

const label = (done: boolean): HayateCssStyle => ({
  defaultColor: done ? C.muted : C.ink,
  defaultFontSize: 14,
  textDecoration: done ? 'line-through' : 'none',
  cursor: 'pointer',
});

const removeBtn: HayateCssStyle = {
  width: 28,
  height: 28,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: 'transparent',
  defaultColor: C.muted,
  defaultFontSize: 18,
  borderRadius: 6,
  cursor: 'pointer',
  ':hover': { defaultColor: C.danger },
};

const ghostBtn: HayateCssStyle = {
  height: 34,
  paddingLeft: 14,
  paddingRight: 14,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: 'transparent',
  defaultColor: C.muted,
  defaultFontSize: 13,
  borderRadius: 8,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
  cursor: 'pointer',
  ':hover': { defaultColor: C.ink, borderColor: C.muted },
};
