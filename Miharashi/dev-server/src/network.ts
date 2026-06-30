import { networkInterfaces } from 'node:os';

/** dev-server をローカルネットワーク（LAN）から到達可能にする bind ホスト。全 IPv4 を listen する。 */
export const ALL_INTERFACES_HOSTNAME = '0.0.0.0';

/** ローカルネットワーク URL の 1 つ（NIC 1 アドレス分）。 */
export interface LocalNetworkUrl {
  /** NIC の IPv4 アドレス（例 `192.168.1.23`）。 */
  readonly address: string;
  /** スマホ等が叩く完全な origin（例 `http://192.168.1.23:5181`）。 */
  readonly url: string;
  /** 由来の NIC 名（例 `en0` / `wlan0`）。複数 NIC の見分け用に添える。 */
  readonly interfaceName: string;
}

/** `os.networkInterfaces()` の family は Node により string / number 揺れがあるので IPv4 判定を吸収する。 */
function isIPv4(family: string | number): boolean {
  return family === 'IPv4' || family === 4;
}

/**
 * このマシンの**ローカルネットワーク（LAN）**側 origin を列挙する。loopback（127.0.0.1）や
 * internal な NIC は除き、同一 LAN のスマホ／別端末がそのまま叩ける `http://<ip>:<port>` を返す。
 *
 * dev-server は dev-only ツールであり、QR で配るのはこの LAN URL（CONTEXT.md「Dev Server」）。
 * 0.0.0.0 で listen していても表示・QR 化するのは具体 IP の URL にする（0.0.0.0 は端末から叩けない）。
 */
export function localNetworkUrls(port: number, scheme = 'http'): LocalNetworkUrl[] {
  const result: LocalNetworkUrl[] = [];
  const interfaces = networkInterfaces();
  for (const [interfaceName, addrs] of Object.entries(interfaces)) {
    if (addrs == null) continue;
    for (const addr of addrs) {
      // loopback / internal は LAN から到達できないので除く。IPv4 のみ（QR/手入力で短く扱える）。
      if (addr.internal || !isIPv4(addr.family)) continue;
      result.push({
        address: addr.address,
        interfaceName,
        url: `${scheme}://${addr.address}:${port}`,
      });
    }
  }
  return result;
}
