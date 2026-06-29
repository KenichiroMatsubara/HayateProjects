/**
 * 依存ゼロの最小 QR エンコーダ。dev-server の起動コマンドが**ローカルネットワーク URL**を
 * 端末画面に QR で出すためだけのもの（CONTEXT.md「Dev Server」）。スマホのカメラ（標準カメラ／
 * Web の `BarcodeDetector`／ネイティブの code-scanner）で読めれば十分なので、用途を URL に絞る：
 *
 *   - **byte モード固定**（URL は ASCII。数字/英数モードの最適化はしない）。
 *   - **EC レベル M 固定**（端末表示の読み取りに十分な誤り訂正）。
 *   - **version 1–6 のみ**（最大 106 byte。LAN URL には十分で、version 情報ブロック（≥7）が
 *     不要になり実装が縮む）。超過は明示エラー（URL でこの長さは現実に出ない）。
 *
 * `ws` を入れず WebSocket フレームを手で組む dev-server の方針（index.ts）と同じく、QR ライブラリも
 * 足さず手で組む。アルゴリズムは公知の QR 生成手順（ISO/IEC 18004 / Nayuki のリファレンス実装と同型）。
 */

// ── GF(256) 演算（Reed–Solomon 用）──────────────────────────────────────────
// 原始多項式 0x11d 上の指数 / 対数表。RS の誤り訂正符号語計算に使う。
const GF_EXP = new Uint8Array(512);
const GF_LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    GF_EXP[i] = x;
    GF_LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) GF_EXP[i] = GF_EXP[i - 255]!;
}

/** GF(256) の乗算。0 を含む積は 0。 */
function gfMul(a: number, b: number): number {
  if (a === 0 || b === 0) return 0;
  return GF_EXP[GF_LOG[a]! + GF_LOG[b]!]!;
}

/** degree 次の Reed–Solomon 生成多項式の係数（先頭の 1 を除く degree 個）を返す。 */
function rsComputeDivisor(degree: number): number[] {
  const result = new Array<number>(degree).fill(0);
  result[degree - 1] = 1;
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < degree; j++) {
      result[j] = gfMul(result[j]!, root);
      if (j + 1 < degree) result[j] = result[j]! ^ result[j + 1]!;
    }
    root = gfMul(root, 0x02);
  }
  return result;
}

/** data を divisor で割った剰余（= 誤り訂正符号語、長さ degree）を返す。 */
function rsComputeRemainder(data: readonly number[], divisor: readonly number[]): number[] {
  const degree = divisor.length;
  const result = new Array<number>(degree).fill(0);
  for (const b of data) {
    const factor = b ^ result.shift()!;
    result.push(0);
    for (let i = 0; i < degree; i++) result[i] = result[i]! ^ gfMul(divisor[i]!, factor);
  }
  return result;
}

// ── version / EC（レベル M）テーブル ────────────────────────────────────────
/** サポートする最小・最大 version。≥7 は version 情報ブロックが要るので 6 で打ち切る。 */
const MIN_VERSION = 1;
const MAX_VERSION = 6;

/** EC レベル M の、1 ブロックあたり誤り訂正符号語数（version 1–6）。 */
const ECC_CODEWORDS_PER_BLOCK_M: Record<number, number> = {
  1: 10,
  2: 16,
  3: 26,
  4: 18,
  5: 24,
  6: 16,
};
/** EC レベル M の、誤り訂正ブロック数（version 1–6）。 */
const NUM_BLOCKS_M: Record<number, number> = { 1: 1, 2: 1, 3: 1, 4: 2, 5: 2, 6: 4 };

/** EC レベル M の format 情報 2 bit（ISO/IEC 18004。L=01,M=00,Q=11,H=10）。 */
const FORMAT_BITS_M = 0;

/** version のシンボル一辺のモジュール数。 */
function moduleCount(version: number): number {
  return version * 4 + 17;
}

/**
 * version のデータ＋誤り訂正モジュールの総 codeword 数。version 情報（≥7）は扱わない前提の式。
 * （ISO/IEC 18004 の生データモジュール数 / 8。）
 */
function rawCodewordCount(version: number): number {
  let result = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const numAlign = Math.floor(version / 7) + 2;
    result -= (25 * numAlign - 10) * numAlign - 55;
  }
  return Math.floor(result / 8);
}

/** version のデータ codeword 数（生 codeword − 誤り訂正 codeword）。 */
function dataCodewordCount(version: number): number {
  return rawCodewordCount(version) - ECC_CODEWORDS_PER_BLOCK_M[version]! * NUM_BLOCKS_M[version]!;
}

/** version の byte モード収容バイト数（mode 4bit + 文字数 8bit のヘッダ分を差し引く）。 */
function byteCapacity(version: number): number {
  return Math.floor((dataCodewordCount(version) * 8 - 4 - 8) / 8);
}

/** byteLen を収容できる最小 version（M, byte モード）。超過は明示エラー。 */
function selectVersion(byteLen: number): number {
  for (let v = MIN_VERSION; v <= MAX_VERSION; v++) {
    if (byteLen <= byteCapacity(v)) return v;
  }
  throw new Error(
    `Miharashi QR: データが大きすぎます（${byteLen} byte > 最大 ${byteCapacity(MAX_VERSION)} byte）`,
  );
}

/** version の位置合わせパターン中心座標（version 1 は無し）。 */
function alignmentPositions(version: number): number[] {
  if (version === 1) return [];
  const numAlign = Math.floor(version / 7) + 2;
  const step = Math.ceil((version * 4 + 4) / (numAlign * 2 - 2)) * 2;
  const result = [6];
  for (let pos = moduleCount(version) - 7; result.length < numAlign; pos -= step) {
    result.splice(1, 0, pos);
  }
  return result;
}

// ── データ codeword 列の構築（ヘッダ + パディング + RS + インターリーブ）──────────
/** byte 列を mode/文字数ヘッダ付きのデータ codeword に詰め、終端・パディングまで行う。 */
function buildDataCodewords(bytes: Uint8Array, version: number): number[] {
  const capacityBits = dataCodewordCount(version) * 8;
  const bits: number[] = [];
  const pushBits = (value: number, len: number): void => {
    for (let i = len - 1; i >= 0; i--) bits.push((value >>> i) & 1);
  };
  // byte モード指示子 0100 + 文字数（version 1–9 は 8bit）+ 本体。
  pushBits(0x4, 4);
  pushBits(bytes.length, 8);
  for (const b of bytes) pushBits(b, 8);
  // 終端ビット（最大 4、容量超過しない範囲）→ byte 境界へ 0 詰め。
  for (let i = 0; i < 4 && bits.length < capacityBits; i++) bits.push(0);
  while (bits.length % 8 !== 0) bits.push(0);
  // codeword 化。
  const codewords: number[] = [];
  for (let i = 0; i < bits.length; i += 8) {
    let byte = 0;
    for (let j = 0; j < 8; j++) byte = (byte << 1) | bits[i + j]!;
    codewords.push(byte);
  }
  // 残り容量を 0xEC / 0x11 の交互パディングで埋める（ISO/IEC 18004）。
  const totalDataCodewords = dataCodewordCount(version);
  for (let pad = 0xec; codewords.length < totalDataCodewords; pad ^= 0xec ^ 0x11) {
    codewords.push(pad);
  }
  return codewords;
}

/** データ codeword をブロック分割→各ブロックに RS を付与→インターリーブして最終 codeword 列にする。 */
function addEccAndInterleave(dataCodewords: readonly number[], version: number): number[] {
  const numBlocks = NUM_BLOCKS_M[version]!;
  const blockEccLen = ECC_CODEWORDS_PER_BLOCK_M[version]!;
  const rawCodewords = rawCodewordCount(version);
  const numShortBlocks = numBlocks - (rawCodewords % numBlocks);
  const shortBlockLen = Math.floor(rawCodewords / numBlocks);
  const divisor = rsComputeDivisor(blockEccLen);

  const blocks: number[][] = [];
  let k = 0;
  for (let i = 0; i < numBlocks; i++) {
    const datLen = shortBlockLen - blockEccLen + (i < numShortBlocks ? 0 : 1);
    const dat = dataCodewords.slice(k, k + datLen);
    k += datLen;
    const ecc = rsComputeRemainder(dat, divisor);
    // short ブロックはデータ部を 1 つ短く詰めるので、インターリーブ位置を揃える穴埋めに 0 を足す。
    if (i < numShortBlocks) dat.push(0);
    blocks.push([...dat, ...ecc]);
  }

  const result: number[] = [];
  const maxLen = blocks[0]!.length;
  for (let i = 0; i < maxLen; i++) {
    for (let j = 0; j < blocks.length; j++) {
      // short ブロックのデータ部末尾に足した穴埋め 0 は飛ばす。
      if (i !== shortBlockLen - blockEccLen || j >= numShortBlocks) result.push(blocks[j]![i]!);
    }
  }
  return result;
}

// ── マトリクス構築 ──────────────────────────────────────────────────────────
/** エンコード済み QR シンボル。`modules[y][x]` が true なら暗モジュール。 */
export interface QrMatrix {
  /** 一辺のモジュール数。 */
  readonly size: number;
  /** 選んだ version（1–6）。 */
  readonly version: number;
  /** 適用したマスクパターン（0–7）。 */
  readonly mask: number;
  /** モジュール格子。`modules[y][x]` が true で暗（dark）。 */
  readonly modules: boolean[][];
}

const PENALTY_N1 = 3;
const PENALTY_N2 = 3;
const PENALTY_N3 = 40;
const PENALTY_N4 = 10;

/** 内部のマトリクス組み立て。関数パターン・データ配置・マスク選択を担う。 */
class Matrix {
  readonly size: number;
  readonly modules: boolean[][];
  /** 関数パターン（データを置けない予約領域）。 */
  readonly #isFunction: boolean[][];

  constructor(readonly version: number) {
    this.size = moduleCount(version);
    this.modules = Array.from({ length: this.size }, () => new Array<boolean>(this.size).fill(false));
    this.#isFunction = Array.from({ length: this.size }, () =>
      new Array<boolean>(this.size).fill(false),
    );
  }

  #setFunction(x: number, y: number, dark: boolean): void {
    this.modules[y]![x] = dark;
    this.#isFunction[y]![x] = true;
  }

  /** 検出パターン（finder 7×7 + 分離帯）を中心 (x,y) に描く。 */
  #drawFinder(x: number, y: number): void {
    for (let dy = -4; dy <= 4; dy++) {
      for (let dx = -4; dx <= 4; dx++) {
        const dist = Math.max(Math.abs(dx), Math.abs(dy));
        const xx = x + dx;
        const yy = y + dy;
        if (xx >= 0 && xx < this.size && yy >= 0 && yy < this.size) {
          this.#setFunction(xx, yy, dist !== 2 && dist !== 4);
        }
      }
    }
  }

  /** 位置合わせパターン（5×5）を中心 (x,y) に描く。 */
  #drawAlignment(x: number, y: number): void {
    for (let dy = -2; dy <= 2; dy++) {
      for (let dx = -2; dx <= 2; dx++) {
        this.#setFunction(x + dx, y + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
      }
    }
  }

  /** format 情報 15bit（EC レベル + mask）を 2 か所へ描く。 */
  #drawFormatBits(mask: number): void {
    const data = (FORMAT_BITS_M << 3) | mask;
    let rem = data;
    for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
    const bits = ((data << 10) | rem) ^ 0x5412;
    const bit = (i: number): boolean => ((bits >>> i) & 1) !== 0;
    const n = this.size;
    for (let i = 0; i <= 5; i++) this.#setFunction(8, i, bit(i));
    this.#setFunction(8, 7, bit(6));
    this.#setFunction(8, 8, bit(7));
    this.#setFunction(7, 8, bit(8));
    for (let i = 9; i < 15; i++) this.#setFunction(14 - i, 8, bit(i));
    for (let i = 0; i < 8; i++) this.#setFunction(n - 1 - i, 8, bit(i));
    for (let i = 8; i < 15; i++) this.#setFunction(8, n - 15 + i, bit(i));
    this.#setFunction(8, n - 8, true); // 常時暗モジュール
  }

  /** タイミング・finder・位置合わせ・format 予約を描く。 */
  drawFunctionPatterns(): void {
    for (let i = 0; i < this.size; i++) {
      this.#setFunction(6, i, i % 2 === 0);
      this.#setFunction(i, 6, i % 2 === 0);
    }
    this.#drawFinder(3, 3);
    this.#drawFinder(this.size - 4, 3);
    this.#drawFinder(3, this.size - 4);
    const pos = alignmentPositions(this.version);
    const last = pos.length - 1;
    for (let i = 0; i < pos.length; i++) {
      for (let j = 0; j < pos.length; j++) {
        // finder と重なる 3 隅は除く。
        if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) continue;
        this.#drawAlignment(pos[i]!, pos[j]!);
      }
    }
    this.#drawFormatBits(0); // ダミー。マスク確定後に本物を描く。
  }

  /** インターリーブ済み codeword をジグザグ順でデータ領域に流し込む。 */
  drawCodewords(codewords: readonly number[]): void {
    let i = 0; // ビット位置
    for (let right = this.size - 1; right >= 1; right -= 2) {
      const col = right === 6 ? 5 : right; // タイミング列（6）を跨ぐ
      for (let vert = 0; vert < this.size; vert++) {
        for (let j = 0; j < 2; j++) {
          const x = col - j;
          const upward = ((col + 1) & 2) === 0;
          const y = upward ? this.size - 1 - vert : vert;
          if (!this.#isFunction[y]![x] && i < codewords.length * 8) {
            this.modules[y]![x] = ((codewords[i >>> 3]! >>> (7 - (i & 7))) & 1) !== 0;
            i++;
          }
        }
      }
    }
  }

  /** マスク条件を満たすデータモジュールを反転する（関数パターンは触らない）。 */
  #applyMask(mask: number): void {
    for (let y = 0; y < this.size; y++) {
      for (let x = 0; x < this.size; x++) {
        if (this.#isFunction[y]![x]) continue;
        let invert: boolean;
        switch (mask) {
          case 0:
            invert = (x + y) % 2 === 0;
            break;
          case 1:
            invert = y % 2 === 0;
            break;
          case 2:
            invert = x % 3 === 0;
            break;
          case 3:
            invert = (x + y) % 3 === 0;
            break;
          case 4:
            invert = (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0;
            break;
          case 5:
            invert = ((x * y) % 2) + ((x * y) % 3) === 0;
            break;
          case 6:
            invert = (((x * y) % 2) + ((x * y) % 3)) % 2 === 0;
            break;
          default:
            invert = ((((x + y) % 2) + ((x * y) % 3)) % 2) === 0;
            break;
        }
        if (invert) this.modules[y]![x] = !this.modules[y]![x];
      }
    }
  }

  #finderPenaltyAddHistory(run: number, history: number[]): void {
    if (history[0] === 0) run += this.size; // 先頭 run には明境界を足す
    history.pop();
    history.unshift(run);
  }

  #finderPenaltyCount(history: readonly number[]): number {
    const n = history[1]!;
    const core =
      n > 0 &&
      history[2] === n &&
      history[3] === n * 3 &&
      history[4] === n &&
      history[5] === n;
    return (
      (core && history[0]! >= n * 4 && history[6]! >= n ? 1 : 0) +
      (core && history[6]! >= n * 4 && history[0]! >= n ? 1 : 0)
    );
  }

  #finderPenaltyTerminate(runColor: boolean, runLen: number, history: number[]): number {
    if (runColor) {
      this.#finderPenaltyAddHistory(runLen, history);
      runLen = 0;
    }
    runLen += this.size;
    this.#finderPenaltyAddHistory(runLen, history);
    return this.#finderPenaltyCount(history);
  }

  /** ISO/IEC 18004 のマスク評価（小さいほど良い）。 */
  #penaltyScore(): number {
    let result = 0;
    const n = this.size;
    const m = this.modules;
    for (let y = 0; y < n; y++) {
      let runColor = false;
      let run = 0;
      const history = [0, 0, 0, 0, 0, 0, 0];
      for (let x = 0; x < n; x++) {
        if (m[y]![x] === runColor) {
          run++;
          if (run === 5) result += PENALTY_N1;
          else if (run > 5) result++;
        } else {
          this.#finderPenaltyAddHistory(run, history);
          if (!runColor) result += this.#finderPenaltyCount(history) * PENALTY_N3;
          runColor = m[y]![x]!;
          run = 1;
        }
      }
      result += this.#finderPenaltyTerminate(runColor, run, history) * PENALTY_N3;
    }
    for (let x = 0; x < n; x++) {
      let runColor = false;
      let run = 0;
      const history = [0, 0, 0, 0, 0, 0, 0];
      for (let y = 0; y < n; y++) {
        if (m[y]![x] === runColor) {
          run++;
          if (run === 5) result += PENALTY_N1;
          else if (run > 5) result++;
        } else {
          this.#finderPenaltyAddHistory(run, history);
          if (!runColor) result += this.#finderPenaltyCount(history) * PENALTY_N3;
          runColor = m[y]![x]!;
          run = 1;
        }
      }
      result += this.#finderPenaltyTerminate(runColor, run, history) * PENALTY_N3;
    }
    for (let y = 0; y < n - 1; y++) {
      for (let x = 0; x < n - 1; x++) {
        const c = m[y]![x];
        if (c === m[y]![x + 1] && c === m[y + 1]![x] && c === m[y + 1]![x + 1]) result += PENALTY_N2;
      }
    }
    let dark = 0;
    for (const row of m) for (const c of row) if (c) dark++;
    const total = n * n;
    const k = Math.ceil(Math.abs(dark * 20 - total * 10) / total) - 1;
    result += k * PENALTY_N4;
    return result;
  }

  /** 全 8 マスクを試し、最小ペナルティのマスクを適用して確定する。適用したマスク番号を返す。 */
  selectAndApplyBestMask(): number {
    let bestMask = 0;
    let minPenalty = Infinity;
    for (let mask = 0; mask < 8; mask++) {
      this.#applyMask(mask);
      this.#drawFormatBits(mask);
      const penalty = this.#penaltyScore();
      if (penalty < minPenalty) {
        minPenalty = penalty;
        bestMask = mask;
      }
      this.#applyMask(mask); // 元に戻す（XOR なので同マスク再適用で復元）
    }
    this.#applyMask(bestMask);
    this.#drawFormatBits(bestMask);
    return bestMask;
  }
}

/** 入力文字列／バイト列を UTF-8 バイトに正規化する。 */
function toBytes(data: string | Uint8Array): Uint8Array {
  return typeof data === 'string' ? new TextEncoder().encode(data) : data;
}

/**
 * 文字列（既定 UTF-8）／バイト列を QR シンボルにエンコードする。byte モード・EC レベル M・
 * version 自動選択（1–6）。URL を端末に出して読ませる用途に特化（CONTEXT.md「Dev Server」）。
 */
export function encodeQr(data: string | Uint8Array): QrMatrix {
  const bytes = toBytes(data);
  const version = selectVersion(bytes.length);
  const dataCodewords = buildDataCodewords(bytes, version);
  const codewords = addEccAndInterleave(dataCodewords, version);
  const matrix = new Matrix(version);
  matrix.drawFunctionPatterns();
  matrix.drawCodewords(codewords);
  const mask = matrix.selectAndApplyBestMask();
  return { size: matrix.size, version, mask, modules: matrix.modules };
}

// ── 端末レンダリング ────────────────────────────────────────────────────────
/** 端末描画のオプション。 */
export interface QrTerminalOptions {
  /** 周囲の余白（quiet zone）モジュール数。既定 2（読み取り安定のため最低 2 は欲しい）。 */
  readonly quietZone?: number;
}

// 24bit truecolor で「暗＝黒 / 明＝白」を明示する。端末テーマ（暗背景）でも反転せず読める。
const FG_DARK = '\x1b[38;2;0;0;0m';
const RESET = '\x1b[0m';
const BG_BLACK = '\x1b[48;2;0;0;0m';
const BG_WHITE = '\x1b[48;2;255;255;255m';

/**
 * QR を端末表示用の文字列にする。上下 2 モジュールを 1 行に畳む半ブロック（▀）描画で、
 * 文字セル縦横比でもほぼ正方になりカメラで読みやすい。暗＝黒・明＝白を ANSI 背景色で固定する。
 */
export function qrToTerminalString(qr: QrMatrix, options: QrTerminalOptions = {}): string {
  const quiet = options.quietZone ?? 2;
  const size = qr.size + quiet * 2;
  // quiet zone 込みで「暗か?」を引く。範囲外（余白）は明（false）。
  const dark = (y: number, x: number): boolean => {
    const yy = y - quiet;
    const xx = x - quiet;
    if (yy < 0 || xx < 0 || yy >= qr.size || xx >= qr.size) return false;
    return qr.modules[yy]![xx]!;
  };
  const lines: string[] = [];
  for (let y = 0; y < size; y += 2) {
    let line = '';
    for (let x = 0; x < size; x++) {
      const top = dark(y, x);
      const bottom = y + 1 < size ? dark(y + 1, x) : false;
      // ▀ は前景色で上半分、背景色で下半分を塗る。暗＝黒・明＝白。
      const bg = bottom ? BG_BLACK : BG_WHITE;
      const fg = top ? FG_DARK : '\x1b[38;2;255;255;255m';
      line += `${bg}${fg}▀`;
    }
    lines.push(line + RESET);
  }
  return lines.join('\n');
}
