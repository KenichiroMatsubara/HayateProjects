import { describe, expect, it, vi } from 'vitest';
import {
  isCameraScanSupported,
  scanQrFromCamera,
  type BarcodeDetectorLike,
  type DetectedBarcode,
} from './index.js';

/**
 * カメラ QR スキャナの配線契約テスト。実カメラ／実ブラウザ／実デコーダを巻き込まず、
 * getUserMedia / detector / frame ループを注入 seam で差し替え、「カメラを開く → フレームを
 * detect → 読めたら onResult してカメラを止める」「途中 cancel で track を畳む」「取得失敗を
 * onError に流す」を観測する（host-web の boot.test.ts と同型）。
 */

/** 停止を観測できる fake track / stream。 */
function fakeStream(): { stream: MediaStream; stopped: () => number } {
  let stops = 0;
  const track = { stop: () => void stops++ } as unknown as MediaStreamTrack;
  const stream = { getTracks: () => [track] } as unknown as MediaStream;
  return { stream, stopped: () => stops };
}

/** srcObject の出し入れを観測できる fake video。 */
function fakeVideo(): HTMLVideoElement {
  return {
    srcObject: null,
    play: vi.fn().mockResolvedValue(undefined),
  } as unknown as HTMLVideoElement;
}

/** 指定の連続結果を順に返す fake detector（最後の値以降は同じ値を返し続ける）。 */
function fakeDetector(sequence: DetectedBarcode[][]): BarcodeDetectorLike {
  let i = 0;
  return {
    detect: vi.fn(async () => {
      const out = sequence[Math.min(i, sequence.length - 1)] ?? [];
      i++;
      return out;
    }),
  };
}

describe('isCameraScanSupported', () => {
  it('is true only when both BarcodeDetector and getUserMedia exist', () => {
    expect(
      isCameraScanSupported({
        BarcodeDetector: function () {},
        navigator: { mediaDevices: { getUserMedia: () => {} } },
      }),
    ).toBe(true);
    expect(isCameraScanSupported({ navigator: { mediaDevices: { getUserMedia: () => {} } } })).toBe(
      false,
    );
    expect(isCameraScanSupported({ BarcodeDetector: function () {} })).toBe(false);
  });
});

describe('scanQrFromCamera', () => {
  it('opens the camera, reads the first QR, reports it, and stops the camera', async () => {
    const { stream, stopped } = fakeStream();
    const video = fakeVideo();
    const onResult = vi.fn();

    scanQrFromCamera({
      video,
      onResult,
      getUserMedia: vi.fn().mockResolvedValue(stream),
      createDetector: () => fakeDetector([[{ rawValue: 'http://192.168.1.23:5181' }]]),
      requestFrame: (cb) => {
        queueMicrotask(cb);
        return 1;
      },
      cancelFrame: () => undefined,
    });

    await vi.waitFor(() => expect(onResult).toHaveBeenCalledWith('http://192.168.1.23:5181'));
    expect(stopped()).toBe(1);
    expect(video.srcObject).toBeNull();
  });

  it('keeps scanning across empty frames until a QR appears', async () => {
    const { stream } = fakeStream();
    const onResult = vi.fn();

    scanQrFromCamera({
      video: fakeVideo(),
      onResult,
      getUserMedia: vi.fn().mockResolvedValue(stream),
      createDetector: () => fakeDetector([[], [], [{ rawValue: 'http://10.0.0.5:5179' }]]),
      requestFrame: (cb) => {
        queueMicrotask(cb);
        return 1;
      },
      cancelFrame: () => undefined,
    });

    await vi.waitFor(() => expect(onResult).toHaveBeenCalledWith('http://10.0.0.5:5179'));
  });

  it('stops the acquired track when cancelled during getUserMedia', async () => {
    const { stream, stopped } = fakeStream();
    let resolveMedia: (s: MediaStream) => void = () => undefined;
    const getUserMedia = vi.fn(
      () =>
        new Promise<MediaStream>((resolve) => {
          resolveMedia = resolve;
        }),
    );
    const onResult = vi.fn();

    const controller = scanQrFromCamera({
      video: fakeVideo(),
      onResult,
      getUserMedia,
      createDetector: () => fakeDetector([[{ rawValue: 'x' }]]),
      requestFrame: (cb) => {
        queueMicrotask(cb);
        return 1;
      },
      cancelFrame: () => undefined,
    });

    controller.cancel();
    resolveMedia(stream);
    await vi.waitFor(() => expect(stopped()).toBe(1));
    expect(onResult).not.toHaveBeenCalled();
  });

  it('reports getUserMedia rejection via onError', async () => {
    const onError = vi.fn();
    scanQrFromCamera({
      video: fakeVideo(),
      onResult: vi.fn(),
      onError,
      getUserMedia: vi.fn().mockRejectedValue(new Error('NotAllowedError')),
      createDetector: () => fakeDetector([[]]),
      requestFrame: (cb) => {
        queueMicrotask(cb);
        return 1;
      },
      cancelFrame: () => undefined,
    });

    await vi.waitFor(() => expect(onError).toHaveBeenCalled());
  });
});
