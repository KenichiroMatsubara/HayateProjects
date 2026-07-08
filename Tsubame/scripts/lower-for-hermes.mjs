// ビルド済み Android バンドルを Hermes が確実に評価できる構文へ降格する（ADR-0112）。
// Hermes は匿名 class 式（var X = class{}）を正しく評価できない（device で undefined 化）。
// preset-env で class/arrow/spread/?? 等を ES5 相当へ transpile し、class キーワードを消す。
//
// 各 example の `torimi:native:build` から `node ../../scripts/lower-for-hermes.mjs <bundle path>` で
// 呼ぶ共有ステップ（#739：solid ローカルコピーから共有化）。対象パスは呼び出し元パッケージの
// cwd 起点で解決する。@babel 依存は tsubame-monorepo（Tsubame/package.json）が持つ。
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { transformAsync } from '@babel/core';

const target = process.argv[2];
if (!target) {
  console.error('usage: node lower-for-hermes.mjs <bundle path (cwd-relative)>');
  process.exit(1);
}

const file = resolve(process.cwd(), target);
const code = readFileSync(file, 'utf8');
const out = await transformAsync(code, {
  babelrc: false,
  configFile: false,
  compact: false,
  presets: [['@babel/preset-env', { targets: { ie: '11' }, modules: false }]],
});
writeFileSync(file, out.code);
const left = (out.code.match(/[^.]\bclass\b/g) || []).length;
console.log('lowered for Hermes; class keywords left:', left, 'size', out.code.length);
