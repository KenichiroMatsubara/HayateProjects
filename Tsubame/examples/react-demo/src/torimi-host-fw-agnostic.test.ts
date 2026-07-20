import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/**
 * #531 の中核主張のガード：「Viewer 一本で全 JS フレームワークが動く」（ADR-0001）。
 *
 * react バンドルを描画する Torimi ホストは、solid バンドルを描画するホストと**同一**で
 * なければならない。ホスト側に framework / renderer-hayate を焼き込むと FW 非依存原則が破れる
 * （CONTEXT.md「Host」: _Avoid_ フレームワークをホストに焼き込む設計）。
 *
 * このテストはユニットだが守る不変条件は振る舞い：誰かがホストに react 固有分岐を足したら
 * （= solid と別物にしたら）byte 同一性が崩れて落ちる。
 */

const reactHostBoot = fileURLToPath(new URL('./host-boot.ts', import.meta.url));
const solidHostBoot = fileURLToPath(new URL('../../solid-demo/src/host-boot.ts', import.meta.url));
const reactHostHtml = fileURLToPath(new URL('../host.html', import.meta.url));

/** ホストが import したら FW / renderer がホストに焼き込まれた証拠になるパッケージ群。 */
const FRAMEWORK_PACKAGES = new Set([
  '@torimi/tsubame-react',
  '@torimi/tsubame-solid',
  '@torimi/tsubame-renderer-hayate',
  '@torimi/tsubame-renderer-dom',
  'react',
  'react-reconciler',
  'solid-js',
]);

/** ソース中の static `from '…'` と dynamic `import('…')` の specifier を全て抜き出す。 */
function importSpecifiers(source: string): string[] {
  const specifiers: string[] = [];
  for (const match of source.matchAll(/\bfrom\s+['"]([^'"]+)['"]/g)) {
    if (match[1] != null) specifiers.push(match[1]);
  }
  for (const match of source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g)) {
    if (match[1] != null) specifiers.push(match[1]);
  }
  return specifiers;
}

describe('Torimi host shell is framework-agnostic (#531)', () => {
  it('drives the react bundle with the byte-identical host boot used for the solid bundle', () => {
    // 文字通り「同じホスト」: react-demo と solid-demo の host-boot は 1 文字も違わない。
    expect(readFileSync(reactHostBoot, 'utf8')).toBe(readFileSync(solidHostBoot, 'utf8'));
  });

  it('imports no framework or renderer into the host boot', () => {
    // プロセスの単語狩りではなく実際の import を見る — コメントが「ここには無い」と
    // renderer-hayate に言及するのは FW 非依存の*肯定*であって違反ではない。
    const imported = importSpecifiers(readFileSync(reactHostBoot, 'utf8'));
    const leaked = imported.filter((spec) => FRAMEWORK_PACKAGES.has(spec));
    expect(leaked).toEqual([]);
  });

  it('serves a bare canvas + host-boot with no framework code in the host page', () => {
    const html = readFileSync(reactHostHtml, 'utf8');
    const leaked = importSpecifiers(html).filter((spec) => FRAMEWORK_PACKAGES.has(spec));
    expect(leaked).toEqual([]);
    expect(html).toContain('torimi-canvas');
    expect(html).toContain('/src/host-boot.ts');
  });
});
