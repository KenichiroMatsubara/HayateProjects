import { describe, expect, it } from 'vitest';
import manifest from '@hayate/protocol-spec/manifest' with { type: 'json' };
import { HOST_PROTOCOL_VERSION } from './index.js';

/**
 * ホストには decoder の wire 定数バージョンが焼き込まれる。Torimi はこれを起動時にバンドルの
 * encoder 版数と突き合わせる（#530 / CONTEXT「Protocol Version」）。decoder（WASM）も spec から
 * 生成されるため、JS 側の版数も同じ spec manifest を source of truth とする。
 */
describe('HOST_PROTOCOL_VERSION（ホストに焼き込まれた decoder の wire 版数）', () => {
  it('matches the protocol-spec manifest version', () => {
    expect(HOST_PROTOCOL_VERSION).toBe(manifest.version);
  });
});
