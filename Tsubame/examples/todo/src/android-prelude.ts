/**
 * Hermes（埋め込み JS エンジン, ADR-0112）向けの最小グローバル shim。
 *
 * ブラウザ実行時に存在する DOM / タイマー系グローバルのうち、Tsubame の
 * 共有コード（`renderTsubame` の resize フォールバック等）が実行時に参照する
 * ものだけを、クラッシュしない最小実装で用意する。実際のビューポート/フレーム
 * 駆動はネイティブが所有する（`__tsubame.resize` / `__tsubame.pumpFrame`）ので、
 * ここで定義する `window` / `requestAnimationFrame` は no-op で良い。
 *
 * このモジュールは副作用 import であり、他のいかなる import よりも前に
 * 実行されなければならない（`main.android.tsx` の先頭で読み込む）。
 */

type AnyGlobal = Record<string, unknown>;
const g = globalThis as unknown as AnyGlobal;

// ネイティブログ橋（ホストが注入していれば使う）。
const nativeLog = g['__hayateLog'] as
  | ((level: string, message: string) => void)
  | undefined;

if (g['console'] === undefined) {
  const make =
    (level: string) =>
    (...args: unknown[]): void => {
      nativeLog?.(level, args.map((a) => String(a)).join(' '));
    };
  g['console'] = {
    log: make('log'),
    info: make('info'),
    warn: make('warn'),
    error: make('error'),
    debug: make('debug'),
  };
}

// マイクロタスク: Solid のスケジューラが使う。Hermes はジョブキューを持つが
// グローバル `queueMicrotask` を公開しないことがあるため Promise で補う。
// キューの排出はネイティブが毎フレーム行う（ADR-0112）。
if (typeof g['queueMicrotask'] !== 'function') {
  g['queueMicrotask'] = (cb: () => void): void => {
    void Promise.resolve().then(cb);
  };
}

// `requestAnimationFrame` は自走させない。フレーム駆動はネイティブ vsync が
// `__tsubame.pumpFrame` で行う。ここでは renderTsubame の resize デバウンスが
// 参照してもクラッシュしないだけの no-op を置く。
if (typeof g['requestAnimationFrame'] !== 'function') {
  g['requestAnimationFrame'] = (_cb: FrameRequestCallback): number => 0;
}
if (typeof g['cancelAnimationFrame'] !== 'function') {
  g['cancelAnimationFrame'] = (_handle: number): void => {};
}

// `fetch`: Android 経路では使わない（フォント等はネイティブが調達）。万一参照
// されてもクラッシュしないよう reject するスタブを置く。
if (typeof g['fetch'] !== 'function') {
  g['fetch'] = (): Promise<never> =>
    Promise.reject(new Error('fetch is unavailable on Android (ADR-0112)'));
}

// 簡易インメモリ Storage（localStorage 代替）。Todo デモのテーマ永続化が
// `window.localStorage` を要求する。Android では永続化先がまだ無いので
// プロセス内メモリで足りる（後段でネイティブ KV に橋渡し可能）。
function createMemoryStorage(): Storage {
  const map = new Map<string, string>();
  return {
    get length(): number {
      return map.size;
    },
    clear: (): void => map.clear(),
    getItem: (key: string): string | null => map.get(key) ?? null,
    key: (index: number): string | null => [...map.keys()][index] ?? null,
    removeItem: (key: string): void => {
      map.delete(key);
    },
    setItem: (key: string, value: string): void => {
      map.set(key, String(value));
    },
  } as Storage;
}

// `URLSearchParams`: Todo デモは `new URLSearchParams(window.location.search).get('page')`
// でページ判定する。Hermes には無いので最小実装を置く（query 文字列のパースと get/has）。
if (typeof g['URLSearchParams'] !== 'function') {
  class MinimalURLSearchParams {
    private readonly map = new Map<string, string>();
    constructor(init?: string) {
      if (typeof init === 'string') {
        for (const pair of init.replace(/^\?/, '').split('&')) {
          if (pair === '') continue;
          const eq = pair.indexOf('=');
          const k = eq < 0 ? pair : pair.slice(0, eq);
          const v = eq < 0 ? '' : pair.slice(eq + 1);
          try {
            this.map.set(decodeURIComponent(k), decodeURIComponent(v));
          } catch {
            this.map.set(k, v);
          }
        }
      }
    }
    get(key: string): string | null {
      return this.map.has(key) ? (this.map.get(key) as string) : null;
    }
    has(key: string): boolean {
      return this.map.has(key);
    }
    getAll(key: string): string[] {
      return this.map.has(key) ? [this.map.get(key) as string] : [];
    }
  }
  g['URLSearchParams'] = MinimalURLSearchParams;
}

// `window`: renderTsubame は `element` 省略時に window.addEventListener('resize')
// と window.innerWidth/innerHeight を参照し、Todo デモは window.location.search /
// window.localStorage を参照する。リサイズはネイティブが `__tsubame.resize` で
// 直接通知するので、イベント系は no-op で足りる。
if (g['window'] === undefined) {
  g['window'] = {
    addEventListener: (_type: string, _handler: unknown): void => {},
    removeEventListener: (_type: string, _handler: unknown): void => {},
    innerWidth: 0,
    innerHeight: 0,
    location: { search: '', href: '', pathname: '/' },
    localStorage: createMemoryStorage(),
  };
}

// `document`: Todo デモはテーマ変更時に `document.documentElement.style` の
// CSS 変数を更新して素 HTML オーバーレイ（#renderer-switch）へ橋渡しするが、
// Android にそのオーバーレイは無いので setProperty 等は no-op で良い。
if (g['document'] === undefined) {
  const noopStyle = {
    setProperty: (_name: string, _value: string): void => {},
    getPropertyValue: (_name: string): string => '',
    removeProperty: (_name: string): string => '',
  };
  g['document'] = {
    documentElement: { style: noopStyle },
    body: {
      appendChild: <T>(node: T): T => node,
      removeChild: <T>(node: T): T => node,
    },
    getElementById: (_id: string): null => null,
    addEventListener: (_type: string, _handler: unknown): void => {},
    removeEventListener: (_type: string, _handler: unknown): void => {},
    baseURI: '',
  };
}
