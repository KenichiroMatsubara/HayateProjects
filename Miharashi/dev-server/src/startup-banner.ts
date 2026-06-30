import { localNetworkUrls, type LocalNetworkUrl } from './network.js';
import { encodeQr, qrToTerminalString } from './qr.js';

/** 起動バナーの組み立てオプション。 */
export interface StartupBannerOptions {
  /** dev-server が listen しているポート。 */
  readonly port: number;
  /** loopback origin（例 `http://127.0.0.1:5181`）。`server.listen()` の戻り値をそのまま渡せる。 */
  readonly loopbackUrl?: string;
  /** QR 化する LAN URL。既定は `localNetworkUrls(port)` の先頭。 */
  readonly networkUrls?: readonly LocalNetworkUrl[];
}

/**
 * QR にするのに最適な LAN URL を 1 つ選ぶ。複数 NIC がある場合の優先度は「プライベート
 * アドレス（192.168 / 10 / 172.16–31）を優先」。スマホは同じ Wi‑Fi の LAN にいる前提なので、
 * VPN や docker bridge より家庭/社内 LAN のアドレスを選びやすくする。
 */
function pickPrimaryUrl(urls: readonly LocalNetworkUrl[]): LocalNetworkUrl | undefined {
  const isPrivate = (addr: string): boolean =>
    addr.startsWith('192.168.') ||
    addr.startsWith('10.') ||
    /^172\.(1[6-9]|2\d|3[01])\./.test(addr);
  return urls.find((u) => isPrivate(u.address)) ?? urls[0];
}

/**
 * 起動コマンドが端末に出すバナー文字列を組み立てる。LAN URL の一覧と、その代表 URL の
 * QR コード（端末描画）を含める。スマホのカメラ（標準カメラ / Web の `BarcodeDetector` /
 * ネイティブ code-scanner）でこの QR を読めば dev-server の LAN URL がそのまま得られる。
 *
 * 文字列を返す純関数にして、出力先（console / ログ）はラッパが決める（テスト容易性）。
 */
export function buildStartupBanner(options: StartupBannerOptions): string {
  const urls = options.networkUrls ?? localNetworkUrls(options.port);
  const lines: string[] = [];
  lines.push('Miharashi dev-server を起動しました。');
  if (options.loopbackUrl != null) lines.push(`  ローカル:        ${options.loopbackUrl}`);

  const primary = pickPrimaryUrl(urls);
  if (primary == null) {
    // NIC が見つからない（オフライン等）。QR は出せないが loopback では使える。
    lines.push('  ローカルネットワーク: 検出できませんでした（Wi‑Fi / LAN 接続を確認してください）。');
    return lines.join('\n');
  }

  lines.push('  ローカルネットワーク:');
  for (const u of urls) {
    const mark = u === primary ? '◀ これを QR 化' : '';
    lines.push(`    ${u.url}  (${u.interfaceName}) ${mark}`.trimEnd());
  }
  lines.push('');
  lines.push('スマホのカメラでこの QR を読み取り、上の URL を入力してください:');
  lines.push('');
  lines.push(qrToTerminalString(encodeQr(primary.url)));
  return lines.join('\n');
}

/**
 * 起動バナーを stdout に出す薄いラッパ。出力副作用をここに閉じ、組み立ては
 * {@link buildStartupBanner} に任せる。
 */
export function printStartupBanner(options: StartupBannerOptions): void {
  console.log(buildStartupBanner(options));
}
