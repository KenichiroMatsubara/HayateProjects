import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { LogBatch } from '@torimi/dev-server-contract';
import { createDeviceLogSink } from './device-log-sink.js';

/**
 * Device Log sink（#786）の挙動テスト。sink 単体を temp dir に対して駆動し、
 * 「どういうターミナル行が組み立てられ、どういうファイルに・どういう行で残るか」という
 * 外部挙動だけを検証する（内部実装には触れない）（ADR-0005）。
 */
describe('createDeviceLogSink', () => {
  let dir: string;
  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'torimi-log-sink-'));
  });
  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it('assembles each entry into a terminal line as `[<label> (<id>)] <level>: <message>`', () => {
    const printed: string[] = [];
    const sink = createDeviceLogSink({ logsDir: dir, print: (line) => printed.push(line) });
    const batch: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [
        { seq: 1, ts: 1720000000000, source: 'js', level: 'error', message: 'boom' },
        { seq: 2, ts: 1720000000001, source: 'host', level: 'warn', message: 'careful' },
      ],
    };

    sink('a1b2c3', batch);

    expect(printed).toEqual([
      '[Pixel 8 (a1b2c3)] error: boom',
      '[Pixel 8 (a1b2c3)] warn: careful',
    ]);
  });

  it('appends each entry to `<deviceId>/<receive-date>.torimi.log` with a `HH:MM:SS.mmm [<level>] <source>: <message>` line', async () => {
    // 受信日はサーバ受信時の開発機ローカル日付で決める（注入 now で固定）。
    const receiveDate = new Date(2026, 6, 10, 15, 4, 5); // 2026-07-10 ローカル
    // 行内時刻は端末側 ts から生成する。ts はこのローカル時刻に対応する epoch ms。
    const ts = new Date(2026, 6, 10, 9, 8, 7, 123).getTime();
    const sink = createDeviceLogSink({ logsDir: dir, print: () => {}, now: () => receiveDate });

    sink('a1b2c3', {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 1, ts, source: 'host', level: 'error', message: 'kaboom' }],
    });

    const file = join(dir, 'a1b2c3', '2026-07-10.torimi.log');
    const contents = await readFile(file, 'utf8');
    expect(contents).toBe('09:08:07.123 [error] host: kaboom\n');
  });

  it('monotonically appends multiple batches of one device to the same date file (never overwrites)', async () => {
    const receiveDate = new Date(2026, 6, 10, 15, 0, 0);
    const sink = createDeviceLogSink({ logsDir: dir, print: () => {}, now: () => receiveDate });
    const ts = new Date(2026, 6, 10, 9, 0, 0, 0).getTime();

    sink('dev1', { deviceLabel: 'Pixel', entries: [{ seq: 1, ts, source: 'js', level: 'log', message: 'first' }] });
    sink('dev1', { deviceLabel: 'Pixel', entries: [{ seq: 2, ts, source: 'js', level: 'log', message: 'second' }] });

    const contents = await readFile(join(dir, 'dev1', '2026-07-10.torimi.log'), 'utf8');
    expect(contents).toBe('09:00:00.000 [log] js: first\n09:00:00.000 [log] js: second\n');
  });

  it('separates different deviceIds into their own directories', async () => {
    const receiveDate = new Date(2026, 6, 10, 15, 0, 0);
    const sink = createDeviceLogSink({ logsDir: dir, print: () => {}, now: () => receiveDate });
    const ts = new Date(2026, 6, 10, 9, 0, 0, 0).getTime();

    sink('dev1', { deviceLabel: 'A', entries: [{ seq: 1, ts, source: 'js', level: 'log', message: 'a' }] });
    sink('dev2', { deviceLabel: 'B', entries: [{ seq: 1, ts, source: 'js', level: 'log', message: 'b' }] });

    expect((await readdir(dir)).sort()).toEqual(['dev1', 'dev2']);
    expect(await readFile(join(dir, 'dev1', '2026-07-10.torimi.log'), 'utf8')).toBe('09:00:00.000 [log] js: a\n');
    expect(await readFile(join(dir, 'dev2', '2026-07-10.torimi.log'), 'utf8')).toBe('09:00:00.000 [log] js: b\n');
  });
});
