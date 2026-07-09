import { describe, expect, it } from 'vitest';

import { tsubameSolid } from './vite.js';

// FW 変換 preset が vite-plugin-solid のプラグインを返すことを固定する。プラグインの
// 内部変換そのものは vite-plugin-solid の責務なので、ここでは「solid プラグインを返す」
// 契約だけを担保する（moduleName/universal の意味は example の e2e が押さえる）。
//
// vite の PluginOption 型は深くネストしうるので、型再帰（TS2589）を避けるため unknown で
// 手動フラット化してから name を拾う。
function flatten(value: unknown): unknown[] {
  const out: unknown[] = [];
  const stack: unknown[] = [value];
  while (stack.length > 0) {
    const item = stack.pop();
    if (Array.isArray(item)) stack.push(...item);
    else if (item) out.push(item);
  }
  return out;
}

describe('tsubameSolid preset', () => {
  it('returns a vite plugin (or plugin array) for solid-js/universal', () => {
    const plugin: unknown = tsubameSolid();
    expect(plugin).toBeTruthy();
    const names = flatten(plugin)
      .filter((p): p is { name: string } => typeof p === 'object' && p !== null && typeof (p as { name?: unknown }).name === 'string')
      .map((p) => p.name);
    expect(names.some((n) => /solid/i.test(n))).toBe(true);
  });
});
