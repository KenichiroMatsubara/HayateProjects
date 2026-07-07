import { PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * #739：react の Android（native, ADR-0112）バンドルが Miharashi の受け渡し契約を満たすことの
 * ガード。solid 版（`examples/todo/src/main.android.test.ts`, #533）と同じ契約 — eval 時に
 * encoder の wire 版数を `__miharashiProtocolVersion` へ立て、mount グローバル（`__tsubame`：
 * native vsync が叩く pumpFrame / stop）を露出する。ホストは中身の FW を解さないので、
 * この 2 つの wire シームが solid と同一であることが「Viewer 一本で全 JS FW が動く」の実体
 * （ADR-0001 / ADR-0003）。
 */

const g = globalThis as Record<string, unknown>;

/** 各テストでエントリを再 eval できるようにモジュールキャッシュと global を掃除する。 */
beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  delete g.__miharashiProtocolVersion;
  delete g.__hayateHost;
  delete g.__tsubame;
});

describe('react Android App Bundle embeds the Miharashi protocol version (#739)', () => {
  it('exposes the renderer-hayate protocol version for the native host handshake', async () => {
    await import('./main.android').catch(() => {
      // ホスト未注入の単体環境では `__hayateHost` 不在で throw する。version 埋め込みは
      // その前の副作用なので握りつぶしてよい（solid 版 #533 と同型）。
    });

    expect(g.__miharashiProtocolVersion).toBe(PROTOCOL_VERSION);
  });
});

describe('react Android App Bundle mounts through the native wire seam (#739)', () => {
  it('exposes __tsubame (pumpFrame / stop) after eval when the native host is injected', async () => {
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

    await import('./main.android');

    const tsubame = g.__tsubame as { pumpFrame?: unknown; stop?: unknown } | undefined;
    expect(typeof tsubame?.pumpFrame).toBe('function');
    expect(typeof tsubame?.stop).toBe('function');
  });
});
