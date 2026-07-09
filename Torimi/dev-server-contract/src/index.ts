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
  /**
   * Device Log を受ける HTTP ルートの接頭辞。ホストは `POST <logRoutePrefix><deviceId>` で
   * {@link LogBatch} を送る。deviceId は接頭辞以降のパス全部を不透明文字列として扱う（ADR-0005）。
   */
  readonly logRoutePrefix: string;
}

export const devServerContract: DevServerContract = {
  bundleRoute: '/bundle.js',
  reloadRoute: '/reload',
  reloadMessage: 'reload',
  logRoutePrefix: '/log/',
};

/** Device Log 1 エントリのログレベル。`console.*` の別名に対応する（ADR-0005）。 */
export type LogLevel = 'log' | 'info' | 'warn' | 'error' | 'debug';

/**
 * Device Log 1 エントリのログ源。`js`（`console.*`・JS ランタイムエラー）と
 * `host`（bundle 取得失敗・native エラー等、JS 起動前に死ぬケースを含む）の 2 系統（ADR-0005）。
 */
export type LogSource = 'js' | 'host';

/**
 * Device Log の 1 エントリ。互換はバージョントークンなしの additive-only —
 * 受け側は未知フィールドを黙って無視し、送り側は既存フィールドの意味変更・削除・改名を
 * しない（変更は新フィールド追加で行う）（ADR-0005）。
 */
export interface LogEntry {
  /** 端末ごと単調増加の連番。サーバは `(deviceId, seq)` で再送重複を捨てる（at-least-once）。 */
  readonly seq: number;
  /** 端末側で記録した時刻（epoch ms）。 */
  readonly ts: number;
  /** ログ源の系統。 */
  readonly source: LogSource;
  /** ログレベル。 */
  readonly level: LogLevel;
  /** ログ本文。 */
  readonly message: string;
}

/**
 * `POST <logRoutePrefix><deviceId>` で送る Device Log バッチ。Device ID に意味を
 * 焼き込まない代わりに、人間向け表示用の Device Label を毎回運ぶ（ADR-0005）。
 */
export interface LogBatch {
  /** 表示用の端末ラベル（端末モデル名等）。 */
  readonly deviceLabel: string;
  /** バッチに含まれるログエントリ列。 */
  readonly entries: readonly LogEntry[];
}

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
