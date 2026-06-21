// 強制ガード(ADR-0069): プラットフォームの `EditContext` — 生成
// (`new EditContext`) と着脱(`.editContext`) — は web IME ブリッジモジュール
// (`edit-context-sync.ts`) だけが触れてよい。EditContext のアタッチがモバイルの
// ソフトキーボードを上げるため、`raw.ime_wants_keyboard()` を起点に 1 モジュールへ
// 閉じ込めることで、単なるタップでキーボードが出る退行を防ぐ。他のプロダクション
// コードは `attachTextInput` / `syncEditContext` 経由にすること。
//
// テストファイルは対象外: 候補ウィンドウ経路を試すため host 管理の EditContext を
// 正当にスタブする。

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it, expect } from 'vitest';

const SRC = dirname(fileURLToPath(import.meta.url));

/** プラットフォームの EditContext に触れてよい唯一のモジュール。 */
const BRIDGE_FILE = 'edit-context-sync.ts';

/** プラットフォーム EditContext API への直接アクセスを意味する部分文字列。
 * `syncEditContext` / `editContexts` 等の識別子で誤検知しないよう選んでいる。 */
const FORBIDDEN = ['new EditContext', '.editContext'];

function tsFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...tsFiles(path));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(path);
    }
  }
  return out;
}

describe('EditContext encapsulation (#392)', () => {
  it('confines direct EditContext access to the bridge module', () => {
    const violations: string[] = [];
    for (const file of tsFiles(SRC)) {
      if (file.endsWith(BRIDGE_FILE)) continue;
      const lines = readFileSync(file, 'utf8').split('\n');
      lines.forEach((line, i) => {
        const trimmed = line.trimStart();
        if (trimmed.startsWith('//') || trimmed.startsWith('*')) return;
        for (const needle of FORBIDDEN) {
          if (line.includes(needle)) {
            violations.push(`${file.slice(SRC.length + 1)}:${i + 1}: ${line.trim()}`);
          }
        }
      });
    }

    expect(
      violations,
      `direct EditContext access must live only in ${BRIDGE_FILE} ` +
        `(route through attachTextInput / syncEditContext):\n${violations.join('\n')}`,
    ).toEqual([]);
  });
});
