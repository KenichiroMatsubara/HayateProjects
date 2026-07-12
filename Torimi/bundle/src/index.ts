// native prelude（条件適用のグローバル shim）を、このパッケージのどの依存（renderer-hayate /
// app / host-native）よりも先に評価する。アプリのエントリは `@torimi/bundle` を最初の import に
// 置くこと — react の scheduler 等は module 評価時にタイマーを capture するため（詳細は
// native-prelude.ts のモジュールコメント）。
import './native-prelude.js';

export { registerTorimiApp, TORIMI_MOUNT_GLOBAL } from './register.js';
export type { TsubameMount } from '@torimi/tsubame-app';
