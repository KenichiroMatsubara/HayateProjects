import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

// 「Tsubame は host を知らない」の依存不変条件（#477, CONTEXT-MAP の依存境界）。
// `@tsubame/renderer-hayate` の **出荷コード**（公開ライブラリ surface）は Hayate の
// web ランタイム adapter（`hayate-adapter-web*`）を import しない。surface 取得・WASM
// ロード・WebGPU プローブ・backend 選択は host bootstrap の責務で、それは Hayate 側
// （`@hayate/host`）または App（合成ルート）が持つ。
//
// 対象外: テスト（`*.test.ts`）と `test-helpers/`。null backend（`hayate-adapter-web-null`）
// は ADR-0055 の codec/golden 結合テスト専用 fixture（devDependency）であって、出荷時の
// ランタイム依存ではない。不変条件は出荷コードの import グラフに対して張る。

const srcDir = fileURLToPath(new URL('.', import.meta.url));

/** 出荷対象の `.ts`（`*.test.ts` と `test-helpers/` を除く）を再帰収集する。 */
function shippedSources(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === 'test-helpers' || entry === '__snapshots__') continue;
      out.push(...shippedSources(full));
      continue;
    }
    if (!entry.endsWith('.ts')) continue;
    if (entry.endsWith('.test.ts')) continue;
    if (entry.endsWith('.d.ts')) continue;
    out.push(full);
  }
  return out;
}

/** コード中の `import`/`import()`/`export ... from` 文だけを拾う（コメントの言及は除く）。 */
function importsHostAdapter(source: string): boolean {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
  return /\b(?:import|export)\b[^\n;]*['"]@?hayate[-/]adapter-web[^'"]*['"]/.test(code);
}

describe('renderer-hayate shipped code is decoupled from the host adapter (#477)', () => {
  const files = shippedSources(srcDir);

  it('finds shipped sources to check', () => {
    expect(files.length).toBeGreaterThan(0);
  });

  for (const file of files) {
    const rel = file.slice(srcDir.length);
    it(`${rel} does not import hayate-adapter-web*`, () => {
      expect(importsHostAdapter(readFileSync(file, 'utf8'))).toBe(false);
    });
  }
});
