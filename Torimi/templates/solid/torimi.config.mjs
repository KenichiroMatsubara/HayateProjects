// Torimi CLI 設定（ADR-0008 §3）。フラットに build（一発ビルドコマンド）と bundle（出力パス）
// だけ。native の Hermes 降格・降格済み別パス配信・ポートは torimi CLI が持つ（per-target 分岐なし）。
export default {
  build: 'vite build --config vite.config.torimi.ts',
  bundle: 'dist-torimi/bundle.js',
  // watch: 'src'（既定）
};
