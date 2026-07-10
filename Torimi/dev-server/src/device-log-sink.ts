import { appendFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { LogBatch, LogEntry } from '@torimi/dev-server-contract';

export interface DeviceLogSinkOptions {
  /**
   * ファイル出力先のベースディレクトリ。sink は `<logsDir>/<deviceId>/<受信日>.torimi.log`
   * に追記する。CLI は `<cwd>/.torimi/logs` を渡し、テストは temp dir を渡せる（#786）。
   */
  readonly logsDir: string;
  /** ターミナル出力先。既定は `console.log`。テストは組み立て結果の行を捕まえる。 */
  readonly print?: (line: string) => void;
  /**
   * 受信時刻の供給元。日付ファイルの振り分け（どの `YYYY-MM-DD` に書くか）は
   * **サーバ受信時の開発機ローカル日付**で決める（ADR-0005）。既定は `() => new Date()`。
   * テストは固定日付を渡してファイル名を決定的にする。
   */
  readonly now?: () => Date;
}

/** 開発機ローカルの `YYYY-MM-DD`（受信日＝日付ファイル名）。 */
function localDateStamp(d: Date): string {
  const p2 = (n: number): string => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())}`;
}

/** 端末側 `ts`（epoch ms）を開発機ローカルの `HH:MM:SS.mmm` に印字する。 */
function formatLineTime(ts: number): string {
  const d = new Date(ts);
  const p2 = (n: number): string => String(n).padStart(2, '0');
  return `${p2(d.getHours())}:${p2(d.getMinutes())}:${p2(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, '0')}`;
}

/** 1 エントリのターミナル行 `[<label> (<id>)] <level>: <message>` を組み立てる。 */
function terminalLine(deviceId: string, deviceLabel: string, entry: LogEntry): string {
  return `[${deviceLabel} (${deviceId})] ${entry.level}: ${entry.message}`;
}

/** ファイル追記の 1 行 `HH:MM:SS.mmm [<level>] <source>: <message>`（時刻は端末側 `ts`）。 */
function fileLine(entry: LogEntry): string {
  return `${formatLineTime(entry.ts)} [${entry.level}] ${entry.source}: ${entry.message}`;
}

/**
 * Dev Server の `onLogBatch` に繋ぐ Device Log sink（#786）。バッチの各エントリを
 * ターミナルへ出し、`<logsDir>/<deviceId>/<受信日>.torimi.log` に素のテキスト行で追記する。
 * sink 本体は dev-server に置き、出力先ディレクトリは引数で受ける（テスト時 temp dir・ADR-0005）。
 */
export function createDeviceLogSink(
  options: DeviceLogSinkOptions,
): (deviceId: string, batch: LogBatch) => void {
  const print = options.print ?? ((line: string): void => console.log(line));
  const now = options.now ?? ((): Date => new Date());
  return (deviceId, batch) => {
    if (batch.entries.length === 0) return;
    for (const entry of batch.entries) {
      print(terminalLine(deviceId, batch.deviceLabel, entry));
    }
    // 受信日は 1 バッチ受理につき一度だけ決める（同一バッチは同じ日付ファイルへ単調追記）。
    const deviceDir = join(options.logsDir, deviceId);
    const logFile = join(deviceDir, `${localDateStamp(now())}.torimi.log`);
    mkdirSync(deviceDir, { recursive: true });
    appendFileSync(logFile, batch.entries.map((entry) => `${fileLine(entry)}\n`).join(''));
  };
}
