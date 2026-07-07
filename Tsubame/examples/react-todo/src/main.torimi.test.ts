import { TORIMI_MOUNT_GLOBAL } from '@torimi/host-web';
import { PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { describe, expect, it } from 'vitest';

// react App Bundle のエントリを import すると（IIFE と同じ副作用で）global に mount と
// protocol version が立つ。ホストはこの 2 つの wire シームだけでバンドルを駆動する（ADR-0001）。
import './main.torimi';

/**
 * #531：react バンドルが Torimi の受け渡し契約を満たすことのガード。ホスト
 * （`@torimi/host-web`）は中身の react を解さず、`__torimiMount`（host bootstrap →
 * mount）と `__torimiProtocolVersion`（encoder 版数）だけを読む。バンドル側が react と
 * `@tsubame/renderer-hayate` を持ち込む構造になっていることをここで観測する。
 */
describe('react App Bundle exposes the Torimi mount contract (#531)', () => {
  it('registers __torimiMount as a callable handed to the host', () => {
    expect(typeof (globalThis as Record<string, unknown>)[TORIMI_MOUNT_GLOBAL]).toBe('function');
  });

  it('embeds the renderer-hayate protocol version for the host handshake', () => {
    // バンドルが持ち込む renderer-hayate の wire 版数をそのまま埋める（#530 のハンドシェイク用）。
    expect((globalThis as Record<string, unknown>).__torimiProtocolVersion).toBe(
      PROTOCOL_VERSION,
    );
  });
});
