/**
 * Dev Server が App Bundle を配信し reload シグナルを流す wire 契約。`dev-server`
 * （配信側）と `host-web`（受信側）が対等に import する唯一の入口 — どちらか一方が
 * 正本でもう一方が値で複製する、という非対称を持たない（ADR-0001 の Protocol Version
 * と同じ扱い）。
 */
export interface DevServerContract {
  /** App Bundle（単一 JS）を配信する HTTP ルート。ホストはこのパスで fetch する。 */
  readonly bundleRoute: string;
  /** reload シグナルを流す WebSocket ルート。ホストはここに繋ぎ reload を待つ。 */
  readonly reloadRoute: string;
  /** ホストに full reload を促す WS メッセージ本文。 */
  readonly reloadMessage: string;
}

export const devServerContract: DevServerContract = {
  bundleRoute: '/bundle.js',
  reloadRoute: '/reload',
  reloadMessage: 'reload',
};

/** Demo Manifest の 1 エントリ。ホストにとってバンドルは不透明で、FW 知識は持ち込まない（ADR-0003）。 */
export interface DemoManifestEntry {
  /** デモ選択メニューに出す表示名。 */
  readonly name: string;
  /** App Bundle の URL。Demo Endpoint origin からの相対パス可。 */
  readonly bundleUrl: string;
}

/**
 * Demo Endpoint が配信するデモ一覧。ホストはこれでメニューを構成し、初回起動は
 * 先頭エントリを自動ロードする（ADR-0003）。
 */
export interface DemoManifest {
  readonly demos: readonly DemoManifestEntry[];
}

/**
 * Demo Endpoint（Cloudflare Worker）の wire 契約。`demo-endpoint`（配信側）と各ホスト
 * （受信側）が対等に import する唯一の入口（ADR-0001 の流儀・ADR-0003）。reload の
 * ルートは {@link devServerContract} の reloadRoute をそのまま使う（Demo Endpoint は
 * 受けて保持するだけで reload を送らない）。
 */
export interface DemoEndpointContract {
  /** Demo Manifest を配信する HTTP ルート。 */
  readonly demoManifestRoute: string;
}

export const demoEndpointContract: DemoEndpointContract = {
  demoManifestRoute: '/demos.json',
};
