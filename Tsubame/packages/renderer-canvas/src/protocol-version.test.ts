import { describe, it, expect } from 'vitest';
import { PROTOCOL_VERSION } from './index.js';

/**
 * App Bundle は内包する `@tsubame/renderer-canvas` の wire 定数バージョンを protocol version として
 * 埋める（#530 / CONTEXT.md「Protocol Version」）。バンドルが埋め込めるよう、renderer-canvas は
 * 生成された版数を `PROTOCOL_VERSION` として re-export する。
 */
describe('PROTOCOL_VERSION（バンドルが埋める encoder の wire 版数）', () => {
  it('re-exports a positive integer protocol version', () => {
    expect(Number.isInteger(PROTOCOL_VERSION)).toBe(true);
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(1);
  });
});
