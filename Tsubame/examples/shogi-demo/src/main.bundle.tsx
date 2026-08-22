// `@torimi/bundle` は最初の import に置く（wire 契約）。native prelude（条件適用の
// グローバル shim）が FW / アプリのモジュール評価より先に効くのはこの順序があるため —
// react の scheduler は module 評価時に `setTimeout` 等を capture する。
import { registerTorimiApp } from '@torimi/bundle';

import { renderTsubame } from '@torimi/tsubame-react';
import { App } from './App';

/**
 * Torimi 将棋 App Bundle の**全ターゲット共通**エントリ（ADR-0008 §4）。
 * protocol version の焼き込み・mount seam（`__torimiMount` / `__tsubame`）の登録・native
 * prelude といった wire 契約の配線は `@torimi/bundle` が隠し、ターゲット差（Native / Web）は
 * `__hayateHost` の有無でランタイム内部分岐する。react-demo / solid-demo と対称で、
 * ここに残る FW 知識は mount の 1 行だけ（ADR-0001）。
 */

registerTorimiApp((renderer) => renderTsubame(<App />, renderer));
