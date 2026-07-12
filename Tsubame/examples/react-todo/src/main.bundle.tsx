// `@torimi/bundle` は最初の import に置く（wire 契約）。native prelude（条件適用の
// グローバル shim）が FW / アプリのモジュール評価より先に効くのはこの順序があるため —
// react の scheduler は module 評価時に `setTimeout` 等を capture する。
import { registerTorimiApp } from '@torimi/bundle';

import { renderTsubame } from '@torimi/tsubame-react';
import { App } from './App';

/**
 * Torimi react App Bundle の**全ターゲット共通**エントリ（#767 / ADR-0008 §4）。旧
 * `main.torimi.tsx`（Web Host 用, #531）と旧 `main.android.tsx`（Native Host 用, #739）の
 * 二重エントリを置き換えた 1 ファイル。
 *
 * protocol version の焼き込み・mount seam（`__torimiMount` / `__tsubame`）の登録・native
 * prelude といった wire 契約の配線は `@torimi/bundle` が隠し、ターゲット差（Native / Web）は
 * `__hayateHost` の有無でランタイム内部分岐する。solid 版（`examples/todo/src/main.bundle.tsx`）
 * と対称：ここに残る FW 知識は mount の 1 行だけなので、FW を差し替えても露出する wire
 * シームは同一 — 同じホストが両方を描画できる（ADR-0001）。
 */

registerTorimiApp((renderer) => renderTsubame(<App />, renderer));
