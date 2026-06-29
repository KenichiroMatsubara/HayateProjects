import jsQR from 'jsqr';
import { describe, expect, it } from 'vitest';
import { encodeQr, qrToTerminalString, type QrMatrix } from './qr.js';

/**
 * 手組み QR エンコーダの契約テスト。実際の QR デコーダ（jsqr）で「自分が描いた QR を読み戻せる」
 * ことを端から端まで確認する — これでバイト配置・マスク選択・ブロックインターリーブ・format/RS が
 * 揃って正しいことを 1 本で押さえる（dev-server が `ws` を入れず WS を手組みするのと同じ方針で、
 * QR ライブラリも入れず手組みする。検証だけは本物のデコーダで裏取りする）。
 */

/** QrMatrix を quiet zone 付き・スケール拡大した RGBA ビットマップにして jsqr に渡せる形にする。 */
function toImageData(qr: QrMatrix, scale = 4, quiet = 4): {
  data: Uint8ClampedArray;
  width: number;
  height: number;
} {
  const dim = (qr.size + quiet * 2) * scale;
  const data = new Uint8ClampedArray(dim * dim * 4);
  for (let py = 0; py < dim; py++) {
    for (let px = 0; px < dim; px++) {
      const mx = Math.floor(px / scale) - quiet;
      const my = Math.floor(py / scale) - quiet;
      const dark =
        mx >= 0 && my >= 0 && mx < qr.size && my < qr.size ? qr.modules[my]![mx]! : false;
      const v = dark ? 0 : 255;
      const o = (py * dim + px) * 4;
      data[o] = v;
      data[o + 1] = v;
      data[o + 2] = v;
      data[o + 3] = 255;
    }
  }
  return { data, width: dim, height: dim };
}

/** encodeQr → bitmap → jsqr デコードの往復で得たテキスト。 */
function roundTrip(text: string): string | null {
  const { data, width, height } = toImageData(encodeQr(text));
  return jsQR(data, width, height)?.data ?? null;
}

describe('encodeQr', () => {
  it('round-trips a typical dev-server LAN URL through a real QR decoder', () => {
    const url = 'http://192.168.1.23:5181';
    expect(roundTrip(url)).toBe(url);
  });

  it('round-trips a longer URL with a path and query (host page link)', () => {
    const url = 'http://192.168.10.200:5173/host.html?dev=http://192.168.10.200:5181';
    expect(roundTrip(url)).toBe(url);
  });

  it('selects a larger version as the data grows', () => {
    const small = encodeQr('http://10.0.0.2:5181');
    const large = encodeQr('http://192.168.100.100:5173/host.html?dev=http://192.168.100.100:5181');
    expect(large.size).toBeGreaterThanOrEqual(small.size);
    // version 1–6 の範囲（17 + 4v ⇒ 21..41）に収まる。
    expect(small.size).toBeGreaterThanOrEqual(21);
    expect(large.size).toBeLessThanOrEqual(41);
  });

  it('throws a clear error when the data exceeds the supported capacity', () => {
    expect(() => encodeQr('x'.repeat(200))).toThrow(/大きすぎます/);
  });
});

describe('qrToTerminalString', () => {
  it('renders one text row per two module rows (half-block) plus quiet zone', () => {
    const qr = encodeQr('http://192.168.1.23:5181');
    const out = qrToTerminalString(qr, { quietZone: 2 });
    const rows = out.split('\n');
    // (size + quiet*2) モジュール行を 2 行ずつ畳む。
    const totalRows = qr.size + 2 * 2;
    expect(rows.length).toBe(Math.ceil(totalRows / 2));
    // 半ブロック文字とリセットを含む。
    expect(out).toContain('▀');
    expect(out).toContain('\x1b[0m');
  });
});
