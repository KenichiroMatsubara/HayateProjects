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
