import { TORIMI_MOUNT_GLOBAL } from '@torimi/host-web';
import { PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * #767：全ターゲット共通の単一エントリ（`main.bundle.tsx`）が Torimi の受け渡し契約を満たす
 * ことのガード。旧 `main.torimi.tsx`（Web, #530/#531）と旧 `main.android.tsx`（native, #533）
 * の二重エントリを置き換えたので、両ターゲットの wire シームを同じ 1 ファイルから観測する。
 * 配線の実体は `@torimi/bundle` にあり、そちらでテスト済み — ここでは「エントリが
 * registerTorimiApp に乗っている」ことだけを見る。
 */

const g = globalThis as Record<string, unknown>;

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  delete g.__torimiProtocolVersion;
  delete g.__torimiMount;
  delete g.__hayateHost;
  delete g.__tsubame;
});

describe('single App Bundle entry — Web Host target (#767)', () => {
  it('registers __torimiMount and embeds the protocol version', async () => {
    await import('./main.bundle');

    expect(typeof g[TORIMI_MOUNT_GLOBAL]).toBe('function');
    expect(g.__torimiProtocolVersion).toBe(PROTOCOL_VERSION);
  });
});

describe('single App Bundle entry — Native Host target (#767)', () => {
  it('exposes __tsubame (pumpFrame / stop) when the native host injected __hayateHost', async () => {
    // ネイティブが JSI で注入する RawHayate の代役。mount 契約の観測に必要な呼び出しに
    // 応えるだけの no-op（描画の実体は実機検証の領分）。
    g.__hayateHost = new Proxy(
      {},
      {
        get: (_target, prop) => {
          if (prop === 'poll_events') return () => [];
          if (prop === 'element_subtree_ids') return () => new Float64Array();
          return () => undefined;
        },
      },
    );

    await import('./main.bundle');

    const tsubame = g.__tsubame as { pumpFrame?: unknown; stop?: unknown } | undefined;
    expect(typeof tsubame?.pumpFrame).toBe('function');
    expect(typeof tsubame?.stop).toBe('function');
    expect(g.__torimiProtocolVersion).toBe(PROTOCOL_VERSION);
  });
});
