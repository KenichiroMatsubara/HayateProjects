import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * native prelude の「常に import・条件適用」（issue #767）の検証。プレリュードは全ターゲットの
 * バンドルに常に入るので、（a）Hermes 的な素の環境では欠けたグローバルを埋め、（b）既に
 * グローバルが揃った環境（ブラウザ / node）では**何も上書きしない**、の両方が成り立って
 * はじめて単一エントリにできる。
 */

const g = globalThis as Record<string, unknown>;

/** node 環境に元から無い（= プレリュードが埋めるはずの）グローバル。 */
const SHIMMED_BY_PRELUDE = [
  'window',
  'document',
  'requestAnimationFrame',
  'cancelAnimationFrame',
] as const;

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  for (const name of SHIMMED_BY_PRELUDE) delete g[name];
});

describe('native prelude — conditional global shims', () => {
  it('fills the globals missing from a bare (Hermes-like) environment', async () => {
    await import('./native-prelude.js');

    // Solid スケジューラ / react scheduler / Todo デモが実行時に参照する面が揃う。
    expect(typeof g.requestAnimationFrame).toBe('function');
    expect(typeof g.cancelAnimationFrame).toBe('function');
    const win = g.window as { localStorage?: Storage; location?: { search?: string } };
    expect(win).toBeDefined();
    expect(typeof win.localStorage?.setItem).toBe('function');
    const doc = g.document as { documentElement?: { style?: { setProperty?: unknown } } };
    expect(typeof doc.documentElement?.style?.setProperty).toBe('function');
  });

  it('leaves already-present globals untouched (browser-safe by construction)', async () => {
    const myWindow = { marker: 'real-window' };
    const myRaf = (): number => 42;
    g.window = myWindow;
    g.requestAnimationFrame = myRaf;

    await import('./native-prelude.js');

    expect(g.window).toBe(myWindow);
    expect(g.requestAnimationFrame).toBe(myRaf);
  });

  it('provides timer shims for the react scheduler without clobbering real timers', async () => {
    // node には setTimeout がある — プレリュード後も node の実装のまま。
    const realSetTimeout = g.setTimeout;
    await import('./native-prelude.js');
    expect(g.setTimeout).toBe(realSetTimeout);
    expect(typeof g.queueMicrotask).toBe('function');
  });
});
