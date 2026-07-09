import { For, createSignal } from 'solid-js';

// Tsubame の Element 語彙（view / text / text-input / button）で書く最小の Todo。
// スタイルは Hayate CSS（flex ベースの型付き宣言）。DOM ではなくホストの Canvas に描画される。
interface Todo {
  id: number;
  text: string;
  done: boolean;
}

export function App() {
  const [todos, setTodos] = createSignal<Todo[]>([
    { id: 1, text: 'Torimi へようこそ', done: false },
    { id: 2, text: 'src/App.tsx を編集して保存すると reload されます', done: false },
  ]);
  const [draft, setDraft] = createSignal('');
  let nextId = 3;

  const add = () => {
    const text = draft().trim();
    if (!text) return;
    setTodos([...todos(), { id: nextId++, text, done: false }]);
    setDraft('');
  };
  const toggle = (id: number) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));

  return (
    <view style={{ display: 'flex', flexDirection: 'column', padding: 24, gap: 16, backgroundColor: '#0f1115' }}>
      <text style={{ defaultColor: '#e6e8ee', defaultFontSize: 26 }}>__PROJECT_NAME__</text>

      <view style={{ display: 'flex', flexDirection: 'row', gap: 8, alignItems: 'center' }}>
        <view style={{ flexGrow: 1 }}>
          <text-input
            value={draft()}
            placeholder="新しいタスク…"
            style={{ height: 40, defaultFontSize: 15, defaultColor: '#e6e8ee', backgroundColor: '#191c24', borderRadius: 8 }}
            onInput={(event) => setDraft(event.value ?? '')}
            onKeyDown={(event) => {
              if (event.key === 'Enter') add();
            }}
          />
        </view>
        <button
          style={{ height: 40, minWidth: 64, display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: '#4f7cff', borderRadius: 8 }}
          onClick={() => add()}
        >
          <text style={{ defaultColor: '#ffffff', defaultFontSize: 15 }}>追加</text>
        </button>
      </view>

      <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <For each={todos()}>
          {(todo) => (
            <view
              style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10, padding: 12, backgroundColor: '#191c24', borderRadius: 10 }}
              onClick={() => toggle(todo.id)}
            >
              <text style={{ defaultFontSize: 18 }}>{todo.done ? '☑' : '☐'}</text>
              <text style={{ defaultColor: todo.done ? '#6b7280' : '#e6e8ee', defaultFontSize: 16 }}>{todo.text}</text>
            </view>
          )}
        </For>
      </view>
    </view>
  );
}
