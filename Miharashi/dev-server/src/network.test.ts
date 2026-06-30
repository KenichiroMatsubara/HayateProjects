import { describe, expect, it } from 'vitest';
import { localNetworkUrls } from './network.js';

/**
 * LAN URL 列挙の契約テスト。実マシンの NIC を引くため具体アドレスは断定できないが、
 * 「loopback を含めない」「url が scheme://address:port 形」という不変だけ押さえる
 * （CI で LAN NIC が無ければ空配列でも可）。
 */
describe('localNetworkUrls', () => {
  it('never includes loopback and formats each url as scheme://address:port', () => {
    const urls = localNetworkUrls(5181);
    for (const u of urls) {
      expect(u.address).not.toBe('127.0.0.1');
      expect(u.url).toBe(`http://${u.address}:5181`);
      expect(u.interfaceName).toBeTruthy();
    }
  });

  it('honors a custom scheme', () => {
    const urls = localNetworkUrls(443, 'https');
    for (const u of urls) expect(u.url.startsWith('https://')).toBe(true);
  });
});
