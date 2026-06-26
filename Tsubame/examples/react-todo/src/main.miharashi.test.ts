import { MIHARASHI_MOUNT_GLOBAL } from '@miharashi/host-web';
import { PROTOCOL_VERSION } from '@tsubame/renderer-canvas';
import { describe, expect, it } from 'vitest';

// react App Bundle のエントリを import すると（IIFE と同じ副作用で）global に mount と
// protocol version が立つ。ホストはこの 2 つの wire シームだけでバンドルを駆動する（ADR-0001）。
import './main.miharashi';

/**
 * #531：react バンドルが Miharashi の受け渡し契約を満たすことのガード。ホスト
 * （`@miharashi/host-web`）は中身の react を解さず、`__miharashiMount`（host bootstrap →
 * mount）と `__miharashiProtocolVersion`（encoder 版数）だけを読む。バンドル側が react と
 * `@tsubame/renderer-canvas` を持ち込む構造になっていることをここで観測する。
 */
describe('react App Bundle exposes the Miharashi mount contract (#531)', () => {
  it('registers __miharashiMount as a callable handed to the host', () => {
    expect(typeof (globalThis as Record<string, unknown>)[MIHARASHI_MOUNT_GLOBAL]).toBe('function');
  });

  it('embeds the renderer-canvas protocol version for the host handshake', () => {
    // バンドルが持ち込む renderer-canvas の wire 版数をそのまま埋める（#530 のハンドシェイク用）。
    expect((globalThis as Record<string, unknown>).__miharashiProtocolVersion).toBe(
      PROTOCOL_VERSION,
    );
  });
});
