// ビルド済み Android バンドルを Hermes が確実に評価できる構文へ降格する（ADR-0112）。
// Hermes は匿名 class 式（var X = class{}）を正しく評価できない（device で undefined 化）。
// preset-env で class/arrow/spread/?? 等を ES5 相当へ transpile し、class キーワードを消す。
import { readFileSync, writeFileSync } from 'node:fs';
import { transformAsync } from '@babel/core';

const file = new URL('../dist-android/tsubame.js', import.meta.url);
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
