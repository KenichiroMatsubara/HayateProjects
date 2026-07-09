// `@torimi/bundle` は最初の import に置く（wire 契約）。native prelude（条件適用の
// グローバル shim）が FW / アプリのモジュール評価より先に効くのはこの順序があるため。
import { registerTorimiApp } from '@torimi/bundle';

import { renderTsubame } from '@tsubame/solid';

import { App } from './App';

// Torimi App Bundle の全ターゲット共通エントリ（ADR-0008 §4）。protocol version の焼き込み・
// mount seam の登録・native prelude といった wire 契約の配線は `@torimi/bundle` が隠す。ここに
// 残る FW 知識は mount の 1 行だけ。native / web の差はビルド後の Hermes 降格だけ（torimi CLI）。
registerTorimiApp((renderer) => renderTsubame(() => <App />, renderer));
