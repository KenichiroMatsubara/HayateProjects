import { describe, expect, it } from 'vitest';

import { appBundle } from './vite.js';

// App Bundle 形状の契約を固定する（ADR-0008 §5）。従前の example の
// `vite.config.torimi.ts` / `vite.config.android.ts` が手書きしていた build ブロックと
// 同等（単一 IIFE・es2020・非圧縮・cssCodeSplit なし）であることをここで保証し、preset 化で
// 出力が変わっていないことを担保する。
describe('appBundle preset', () => {
  it('reproduces the App Bundle build shape the example configs hand-wrote', () => {
    const cfg = appBundle({ entry: '/abs/src/main.bundle.tsx', name: 'TsubameTodoTorimi', outDir: 'dist-torimi' });
    expect(cfg.build).toMatchObject({
      target: 'es2020',
      outDir: 'dist-torimi',
      emptyOutDir: true,
      cssCodeSplit: false,
      minify: false,
    });
    const lib = cfg.build?.lib as unknown as { entry: string; formats: string[]; name: string; fileName: () => string };
    expect(lib).toMatchObject({ entry: '/abs/src/main.bundle.tsx', formats: ['iife'], name: 'TsubameTodoTorimi' });
    expect(lib.fileName()).toBe('bundle.js');
  });

  it('defaults the output file name to bundle.js and honours an override', () => {
    const web = appBundle({ entry: '/e', name: 'N', outDir: 'dist-torimi' });
    expect((web.build?.lib as unknown as { fileName: () => string }).fileName()).toBe('bundle.js');

    const native = appBundle({ entry: '/e', name: 'N', outDir: 'dist-android', fileName: 'tsubame.js' });
    expect(native.build?.outDir).toBe('dist-android');
    expect((native.build?.lib as unknown as { fileName: () => string }).fileName()).toBe('tsubame.js');
  });

  it('accepts a file: URL entry and resolves it to a path', () => {
    const cfg = appBundle({ entry: new URL('file:///repo/src/main.bundle.tsx'), name: 'N', outDir: 'd' });
    expect((cfg.build?.lib as unknown as { entry: string }).entry).toBe('/repo/src/main.bundle.tsx');
  });
});
