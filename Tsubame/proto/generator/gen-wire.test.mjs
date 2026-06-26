import { describe, it, expect } from 'vitest';
import { loadProtocolSpec } from '@hayate/protocol-spec/load';
import { PROTOCOL_VERSION } from '../generated/protocol.ts';

/**
 * wire の protocol version は spec manifest の `version` を唯一の source of truth とする
 * （#530 / CONTEXT.md「Protocol Version」）。バンドルが埋める encoder 版数も、ホストの
 * decoder 版数も、ここから派生する。生成物が手動編集や spec 改訂で manifest からずれない
 * ことをこのテストで固定する。
 */
describe('generated PROTOCOL_VERSION', () => {
  it('matches the protocol-spec manifest version', () => {
    const spec = loadProtocolSpec();
    expect(PROTOCOL_VERSION).toBe(spec.manifest.version);
  });
});
