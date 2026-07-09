import { readFile, writeFile } from 'node:fs/promises';

import { transformAsync } from '@babel/core';
// Pass the resolved preset object (not the string "@babel/preset-env"): Babel
// resolves preset names relative to the cwd of the app being built, where
// preset-env may be nested under `torimi`'s own deps and unfindable. Importing it
// here resolves it against the CLI's install, so `torimi lower/build` works in any
// external project.
import presetEnv from '@babel/preset-env';

// Hermes は匿名 class 式（`var X = class{}`）を正しく評価できない（device で undefined 化, ADR-0112）。
// preset-env で class/arrow/spread/?? 等を ES5 相当へ降格し class キーワードを消す。降格は
// ターゲット固有（native のみ）＝ CLI の責務（ADR-0008 §1、`lower-for-hermes` から移管）。
export async function lowerForHermes(code: string): Promise<string> {
  const out = await transformAsync(code, {
    babelrc: false,
    configFile: false,
    compact: false,
    presets: [[presetEnv, { targets: { ie: '11' }, modules: false }]],
  });
  if (!out?.code) throw new Error('torimi lower: babel produced no output');
  return out.code;
}

// 降格が効いたかの目安：プロパティアクセス（`foo.class`）や識別子片（`className`）ではない
// `class` キーワードの残数。行頭の `class` も数えるため lookbehind を使う（旧 lower-for-hermes の
// `[^.]\bclass\b` は先頭の class を取りこぼしていた）。
export function countClassKeywords(code: string): number {
  return (code.match(/(?<![.\w])class\b/g) || []).length;
}

export interface LowerResult {
  readonly classKeywordsLeft: number;
  readonly size: number;
}

// `src` を降格して `dest` へ書き出す。native の dev/build では dest を別パスにして未降格を
// 配らない。`torimi lower <file>` のエスケープハッチでは src===dest（in-place）。
export async function lowerFileTo(src: string, dest: string): Promise<LowerResult> {
  const lowered = await lowerForHermes(await readFile(src, 'utf8'));
  await writeFile(dest, lowered);
  return { classKeywordsLeft: countClassKeywords(lowered), size: lowered.length };
}
