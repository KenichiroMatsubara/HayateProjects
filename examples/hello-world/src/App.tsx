import { createSignal } from 'solid-js';

/**
 * DOM / Canvas のどちらの Renderer でも変更なしで動く同一コンポーネント。
 * Element 語彙（view / text / button）と StylePatch のみを使い、
 * レンダリング先を一切意識しない。
 */
export function App() {
  const [count, setCount] = createSignal(0);

  return (
    <view
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 16,
        width: 400,
        height: 300,
        backgroundColor: '#0f172a',
        borderRadius: 16,
      }}
    >
      <text style={{ color: '#e2e8f0', fontSize: 28, fontWeight: 700 }}>
        Hello, Tsubame 燕
      </text>
      <text style={{ color: '#94a3b8', fontSize: 16 }}>
        {`クリック回数: ${count()}`}
      </text>
      <button
        style={{
          backgroundColor: '#38bdf8',
          color: '#0f172a',
          fontSize: 16,
          fontWeight: 600,
          borderRadius: 8,
        }}
        onClick={() => setCount(count() + 1)}
      >
        カウント +1
      </button>
    </view>
  );
}
