import { describe, expect, it, vi } from 'vitest';
import { devServerContract } from '@torimi/dev-server-contract';
import { subscribeReload, type ReloadSocket } from './index.js';

/**
 * Torimi Web ホストの full reload ループ（ホスト側）の配線契約テスト（ADR-0001 / CONTEXT.md
 * 「Reload」）。実ブラウザ / 実 WS を巻き込まず、WS 接続（{@link ReloadSocket}）と再接続スケジュール
 * を注入 seam で差し替え、「dev-server の WS に繋ぎ、`reload` 受信で再 mount を起こし、切断時は
 * 名前付き backoff で再接続する」配線を観測する。
 */

/** メッセージ / close を後から差し込めるテスト用 socket。 */
function fakeSocket(): ReloadSocket & {
  emitMessage(data: string): void;
  emitClose(): void;
  closed: boolean;
} {
  let onMessage: (data: string) => void = () => {};
  let onClose: () => void = () => {};
  return {
    closed: false,
    onMessage(cb) {
      onMessage = cb;
    },
    onClose(cb) {
      onClose = cb;
    },
    close() {
      this.closed = true;
    },
    emitMessage(data) {
      onMessage(data);
    },
    emitClose() {
      onClose();
    },
  };
}

describe('subscribeReload', () => {
  it('invokes onReload when the dev-server sends a reload message', () => {
    const socket = fakeSocket();
    const onReload = vi.fn();

    subscribeReload({
      devServerUrl: 'http://dev.example',
      onReload,
      connect: () => socket,
    });

    socket.emitMessage(devServerContract.reloadMessage);

    expect(onReload).toHaveBeenCalledTimes(1);
  });

  it('ignores non-reload messages', () => {
    const socket = fakeSocket();
    const onReload = vi.fn();

    subscribeReload({
      devServerUrl: 'http://dev.example',
      onReload,
      connect: () => socket,
    });

    socket.emitMessage('something-else');

    expect(onReload).not.toHaveBeenCalled();
  });

  it('connects to the dev-server reload route over the ws scheme', () => {
    const connect = vi.fn(() => fakeSocket());

    subscribeReload({
      devServerUrl: 'http://127.0.0.1:5181',
      onReload: () => {},
      connect,
    });

    expect(connect).toHaveBeenCalledWith(`ws://127.0.0.1:5181${devServerContract.reloadRoute}`);
  });

  it('reconnects after a fixed backoff when the socket closes', () => {
    const sockets = [fakeSocket(), fakeSocket()];
    const connect = vi.fn(() => sockets[connect.mock.calls.length - 1]!);
    let scheduledDelay: number | undefined;
    let scheduled: (() => void) | undefined;

    subscribeReload({
      devServerUrl: 'http://dev.example',
      onReload: () => {},
      connect,
      scheduleReconnect: (fn, delayMs) => {
        scheduled = fn;
        scheduledDelay = delayMs;
      },
    });

    expect(connect).toHaveBeenCalledTimes(1);

    // 切断 → 名前付き backoff で再接続がスケジュールされる。
    sockets[0]!.emitClose();
    expect(scheduledDelay).toBeGreaterThan(0);

    // スケジュールされた再接続が走ると、新しい接続が張られる。
    scheduled?.();
    expect(connect).toHaveBeenCalledTimes(2);
  });

  it('stops reconnecting once the subscription is closed', () => {
    const socket = fakeSocket();
    const connect = vi.fn(() => socket);
    const scheduleReconnect = vi.fn();

    const subscription = subscribeReload({
      devServerUrl: 'http://dev.example',
      onReload: () => {},
      connect,
      scheduleReconnect,
    });

    subscription.close();
    expect(socket.closed).toBe(true);

    // 閉じた後の切断イベントでは再接続をスケジュールしない。
    socket.emitClose();
    expect(scheduleReconnect).not.toHaveBeenCalled();
  });
});
