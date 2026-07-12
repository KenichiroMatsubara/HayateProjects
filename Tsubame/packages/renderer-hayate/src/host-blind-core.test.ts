import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// 「Tsubame は host を知らない」の強形を最高点で固定する構造（型）テスト（#476,
// ADR-0004 / Tsubame CONTEXT）。renderer-hayate のコア — renderer 本体とその
// Hayate ポート — は platform 識別子をゼロ保持する：surface 型（`HTMLCanvasElement`）
// も IME プリミティブ（`EditContext`）もコード上に現れない。surface・resize・IME・
// pointer は host 側 adapter（`hayate-adapter-web` / native）が所有する。
//
// host bootstrap（`init.ts` / `init-android.ts`）は #477 でこのパッケージから退去し、
// Hayate 側（`@torimi/hayate-host`）へ移った。残る出荷コード全体が host-blind であることは
// 依存不変条件テスト `no-host-adapter-import.test.ts` が併せて固定する。

/**
 * コードからコメント（`/* … *​/` と `// …`）を除く。IME を説明する散文に現れる
 * `EditContext` への言及は「参照」ではないので、構造判定はコードだけを見る。
 */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
}

const CORE_FILES = ['./hayate-renderer.ts', './hayate.ts'] as const;
const HOST_IDENTIFIERS = ['HTMLCanvasElement', 'EditContext'] as const;

describe('renderer-hayate core is host-blind (#476, ADR-0004)', () => {
  for (const rel of CORE_FILES) {
    const path = fileURLToPath(new URL(rel, import.meta.url));
    const code = stripComments(readFileSync(path, 'utf8'));
    for (const identifier of HOST_IDENTIFIERS) {
      it(`${rel} does not reference ${identifier}`, () => {
        expect(code).not.toContain(identifier);
      });
    }
  }
});
