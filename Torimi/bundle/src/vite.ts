import { fileURLToPath } from 'node:url';

import type { UserConfig } from 'vite';

// App Bundle 形状 preset（ADR-0008 §5）。「App Bundle とは何か」= 単一 IIFE・es2020・
// DOM/HTML なし・CSS code-split なし・非圧縮、という Torimi の出力契約を 1 か所に畳む。
// Bundle Registration（registerTorimiApp）と同じパッケージに同居させ、CLI 本体 `torimi`
// には置かない（ビルドツール非依存の維持）。
//
// ターゲット差（native の Hermes 降格）はここに含めない — それは Torimi CLI（#770）が
// ビルド後に別パスへ施す責務。この preset が吐くのは target 非依存の「素の App Bundle」。

export interface AppBundleOptions {
  /** 全ターゲット共通の単一エントリ（`registerTorimiApp` を呼ぶ `main.bundle.tsx`、#767）。 */
  entry: string | URL;
  /** IIFE のグローバル名。 */
  name: string;
  /** 出力ディレクトリ。 */
  outDir: string;
  /** 出力ファイル名（既定 `bundle.js`）。 */
  fileName?: string;
}

export function appBundle(options: AppBundleOptions): UserConfig {
  const { entry, name, outDir, fileName = 'bundle.js' } = options;
  return {
    build: {
      target: 'es2020',
      outDir,
      emptyOutDir: true,
      cssCodeSplit: false,
      // デバッグしやすさ優先で非圧縮。サイズ最適化は後段（hermesc/リリース）。
      minify: false,
      lib: {
        entry: entry instanceof URL ? fileURLToPath(entry) : entry,
        formats: ['iife'],
        name,
        fileName: () => fileName,
      },
    },
  };
}

export default appBundle;
