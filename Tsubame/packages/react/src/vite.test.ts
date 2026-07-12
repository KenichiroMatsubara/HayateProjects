import { describe, expect, it } from 'vitest';

import { tsubameReact } from './vite.js';

// FW 変換 preset が、従前の example の react 用 config（automatic JSX を @torimi/tsubame-react に
// 向け、process.env.NODE_ENV を production へ静的置換）と同等の設定断片を返すことを固定する。
describe('tsubameReact preset', () => {
  it('points automatic JSX at @torimi/tsubame-react and pins NODE_ENV to production', () => {
    const cfg = tsubameReact();
    expect(cfg.esbuild).toMatchObject({ jsx: 'automatic', jsxImportSource: '@torimi/tsubame-react' });
    expect(cfg.define).toMatchObject({ 'process.env.NODE_ENV': '"production"' });
  });
});
