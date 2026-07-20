// `@torimi/bundle` は最初の import に置く（wire 契約）。native prelude（条件適用の
// グローバル shim）が FW / アプリのモジュール評価より先に効くのはこの順序があるため。
import { registerTorimiApp } from '@torimi/bundle';

import { renderTsubame } from '@torimi/tsubame-solid';
import { TodoApp } from './App';

/**
 * Torimi App Bundle の**全ターゲット共通**エントリ（#767 / ADR-0008 §4）。旧
 * `main.torimi.tsx`（Web Host 用）と旧 `main.android.tsx`（Native Host 用）の二重エントリを
 * 置き換えた 1 ファイル。
 *
 * protocol version の焼き込み・mount seam（`__torimiMount` / `__tsubame`）の登録・native
 * prelude といった wire 契約の配線は `@torimi/bundle` が隠し、ターゲット差（Native / Web）は
 * `__hayateHost` の有無でランタイム内部分岐する。ここに残る FW 知識は mount の 1 行だけ
 * （ADR-0012 の唯一の FW 固有 seam）。native / web のバンドル差はビルド後の Hermes 降格だけ
 * （`torimi:native:build`）。
 */

registerTorimiApp((renderer) =>
  renderTsubame(() => <TodoApp />, renderer),
);
