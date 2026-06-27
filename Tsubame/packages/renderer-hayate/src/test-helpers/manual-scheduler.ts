/** テスト用の決定的フレームスケジューラ。`tick()` で保留中フレームを実行する。 */
export interface ManualScheduler {
  requestFrame: (cb: FrameRequestCallback) => number;
  cancelFrame: (handle: number) => void;
  tick: (timestamp?: number) => void;
}

export function manualScheduler(): ManualScheduler {
  let pending: FrameRequestCallback | null = null;
  return {
    requestFrame: (cb: FrameRequestCallback) => {
      pending = cb;
      return 1;
    },
    cancelFrame: () => {
      pending = null;
    },
    tick: (timestamp = 16) => {
      const cb = pending;
      pending = null;
      cb?.(timestamp);
    },
  };
}
