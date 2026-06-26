import { PROTOCOL_VERSION } from '@tsubame/renderer-canvas';
import { afterEach, describe, expect, it } from 'vitest';

/**
 * #533：Android（native, ADR-0112）バンドルが Miharashi の protocol version 受け渡し契約を
 * 満たすことのガード。Web の `main.miharashi.tsx`（#530/#531）と対称に、native バンドルも
 * eval 時に encoder の wire 版数を `__miharashiProtocolVersion` へ立て、ネイティブホスト
 * （`hayate-adapter-android` の app_tsubame）が自身の decoder 版数と突き合わせられるようにする。
 *
 * `main.android.tsx` は `globalThis.__hayateHost` 未注入だと（ホスト未注入の単体環境では当然）
 * 即 throw するが、version 埋め込みはその throw より前の eval 副作用なので、import を握りつぶしても
 * global は立つ。ここではその 1 点だけを観測する（描画は実機, 本 issue 外）。
 */
describe('Android App Bundle embeds the Miharashi protocol version (#533)', () => {
  afterEach(() => {
    delete (globalThis as Record<string, unknown>).__miharashiProtocolVersion;
  });

  it('exposes the renderer-canvas protocol version for the native host handshake', async () => {
    await import('./main.android').catch(() => {
      // ホスト未注入の単体環境では `__hayateHost` 不在で throw する。version 埋め込みは
      // その前の副作用なので握りつぶしてよい。
    });

    expect((globalThis as Record<string, unknown>).__miharashiProtocolVersion).toBe(
      PROTOCOL_VERSION,
    );
  });
});
