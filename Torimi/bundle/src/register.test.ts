import { TORIMI_PROTOCOL_VERSION_GLOBAL } from '@torimi/protocol-handshake';
import { PROTOCOL_VERSION } from '@torimi/tsubame-renderer-hayate';
import { afterEach, describe, expect, it } from 'vitest';
import { registerTorimiApp } from './register.js';

/**
 * Bundle Registration（ADR-0008 §4 / CONTEXT.md）の振る舞いを、ホストが見る wire シーム
 * （global 群）越しに観測する。ホストはバンドルの中身（FW・renderer）を解さず、
 * `__torimiProtocolVersion` / `__torimiMount` / `__tsubame` だけを読む — テストも同じ面を読む。
 */

const g = globalThis as Record<string, unknown>;

afterEach(() => {
  delete g[TORIMI_PROTOCOL_VERSION_GLOBAL];
  delete g.__torimiMount;
  delete g.__hayateHost;
  delete g.__tsubame;
});

describe('registerTorimiApp — protocol version burn-in', () => {
  it('embeds the bundled renderer-hayate wire version for the host handshake', () => {
    registerTorimiApp(() => undefined);

    expect(g[TORIMI_PROTOCOL_VERSION_GLOBAL]).toBe(PROTOCOL_VERSION);
  });
});

describe('registerTorimiApp — Web Host target (`__hayateHost` 不在)', () => {
  it('registers __torimiMount so the host can hand over its bootstrap and get the app mounted', () => {
    const mountedWith: unknown[] = [];
    registerTorimiApp((renderer) => {
      mountedWith.push(renderer);
    });

    // 登録だけでは mount しない — mount はホストが bootstrap を渡した時に起きる。
    const torimiMount = g.__torimiMount as ((host: unknown) => void) | undefined;
    expect(typeof torimiMount).toBe('function');
    expect(mountedWith).toHaveLength(0);

    // ホストが host bootstrap（raw + frame-clock）を渡すと、バンドルが持ち込む renderer が
    // 構築・始動されて mount に届く。始動の観測はホストの frame-clock が武装されること。
    let framesRequested = 0;
    torimiMount?.({
      raw: {},
      requestFrame: () => {
        framesRequested += 1;
        return framesRequested;
      },
      cancelFrame: () => undefined,
    });

    expect(mountedWith).toHaveLength(1);
    expect(mountedWith[0]).toBeTruthy();
    expect(framesRequested).toBeGreaterThan(0);
  });
});

describe('registerTorimiApp — Native Host target (`__hayateHost` 注入済み)', () => {
  /** ネイティブが JSI で注入する RawHayate の代役。契約の観測に必要な呼び出しに応えるだけ。 */
  function fakeRawHayate(): object {
    return new Proxy(
      {},
      {
        get: (_target, prop) => {
          if (prop === 'poll_events') return () => [];
          if (prop === 'element_subtree_ids') return () => new Float64Array();
          return () => undefined;
        },
      },
    );
  }

  it('mounts immediately with the injected raw and exposes __tsubame (pumpFrame / stop)', () => {
    g.__hayateHost = fakeRawHayate();

    let disposed = false;
    registerTorimiApp(() => () => {
      disposed = true;
    });

    // native ホストは eval 後に `__tsubame` を読む（ADR-0112 の wire シーム）。
    const tsubame = g.__tsubame as { pumpFrame?: unknown; stop?: unknown } | undefined;
    expect(typeof tsubame?.pumpFrame).toBe('function');
    expect(typeof tsubame?.stop).toBe('function');

    // stop はアプリツリーの dispose まで畳む（runTsubameApp の合成 dispose を素通し）。
    (tsubame as { stop: () => void }).stop();
    expect(disposed).toBe(true);
  });

  it('does not register the web mount seam when targeting the native host', () => {
    g.__hayateHost = fakeRawHayate();

    registerTorimiApp(() => undefined);

    // ターゲット差は内部分岐 — native では web の受け渡しシームを立てない（排他）。
    expect(g.__torimiMount).toBeUndefined();
  });
});
