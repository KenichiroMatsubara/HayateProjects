/**
 * Hermes（埋め込み JS エンジン, ADR-0112）向けの最小グローバル shim（react 版）。
 *
 * solid 版（`examples/todo/src/android-prelude.ts`）と同じ思想 — ブラウザ実行時に存在する
 * グローバルのうち、バンドルが実行時に参照するものだけをクラッシュしない最小実装で用意する。
 * react 固有の差分は timer shim：react の scheduler（react-reconciler が使う）は継続を
 * `MessageChannel` か `setTimeout` に載せるが、埋め込み Hermes はどちらも公開しない。
 * フレーム駆動はネイティブが所有し（`__tsubame.pumpFrame`）、マイクロタスクは毎フレーム
 * ネイティブが排出する（ADR-0112）ので、遅延無視のマイクロタスク実装で足りる。
 *
 * このモジュールは副作用 import であり、他のいかなる import よりも前に実行されなければ
 * ならない（`main.android.tsx` の先頭で読み込む）。
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

// マイクロタスク: react の同期 flush 後の後始末や Promise 継続が使う。Hermes はジョブ
// キューを持つがグローバル `queueMicrotask` を公開しないことがあるため Promise で補う。
// キューの排出はネイティブが毎フレーム行う（ADR-0112）。
if (typeof g['queueMicrotask'] !== 'function') {
  g['queueMicrotask'] = (cb: () => void): void => {
    void Promise.resolve().then(cb);
  };
}

// `setTimeout` / `clearTimeout`: react の scheduler が作業ループの継続に使う
// （`MessageChannel` が無い環境では `setTimeout` へフォールバックする）。埋め込み Hermes は
// timer を持たないので、遅延を無視してマイクロタスクに載せる最小実装を置く。排出は毎フレーム
// ネイティブが行うため、「次フレームまでに継続が走る」という scheduler の期待は満たす。
if (typeof g['setTimeout'] !== 'function') {
  let nextTimerHandle = 1;
  const cancelled = new Set<number>();
  g['setTimeout'] = (cb: (...args: unknown[]) => void, _delayMs?: number): number => {
    const handle = nextTimerHandle++;
    void Promise.resolve().then(() => {
      if (cancelled.delete(handle)) return;
      cb();
    });
    return handle;
  };
  g['clearTimeout'] = (handle: number): void => {
    cancelled.add(handle);
  };
}

// `requestAnimationFrame` は自走させない。フレーム駆動はネイティブ vsync が
// `__tsubame.pumpFrame` で行う。scheduler 等が参照してもクラッシュしないだけの no-op を置く。
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
