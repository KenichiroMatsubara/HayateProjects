/**
 * Miharashi Web ホスト用のカメラ QR スキャナ。スマホのブラウザで host ページを開いたとき、
 * 起動コマンドが端末に出した **dev-server の LAN URL** を QR から読み取って `?dev=` を手で
 * 打たずに接続できるようにする（CONTEXT.md「Dev Server」/「Host」）。
 *
 * 依存ゼロ：標準の `BarcodeDetector` Web API（Android Chrome 等が実装）と `getUserMedia` だけで
 * 組む。dev-server が `ws` を入れず WS を手組みするのと同じ方針。`BarcodeDetector` 非対応の
 * ブラウザ（現状の iOS Safari 等）では {@link isCameraScanSupported} が false を返すので、呼び側は
 * 手入力にフォールバックする（host-boot 側が UI を出し分ける）。
 *
 * 実カメラ／実ブラウザを巻き込まずに配線を検証できるよう、`getUserMedia` / detector / frame ループは
 * 注入 seam にする（host-web の他テストと同じ流儀）。
 */

/** `BarcodeDetector.detect` が返す 1 件。`rawValue` がデコード文字列（= URL）。 */
export interface DetectedBarcode {
  readonly rawValue: string;
}

/** 使う範囲だけ写し取った `BarcodeDetector` の最小形。 */
export interface BarcodeDetectorLike {
  detect(source: CanvasImageSource): Promise<DetectedBarcode[]>;
}

/** 実カメラ／実ブラウザを差し替えるための注入 seam。 */
export interface CameraScanSeams {
  /** カメラ stream を取る seam。既定は `navigator.mediaDevices.getUserMedia`。 */
  readonly getUserMedia?: (constraints: MediaStreamConstraints) => Promise<MediaStream>;
  /** QR デコーダを作る seam。既定は `new BarcodeDetector({ formats: ['qr_code'] })`。 */
  readonly createDetector?: () => BarcodeDetectorLike;
  /** 次フレームを予約する seam。既定は `requestAnimationFrame`。 */
  readonly requestFrame?: (cb: () => void) => number;
  /** 予約フレームを取り消す seam。既定は `cancelAnimationFrame`。 */
  readonly cancelFrame?: (handle: number) => void;
}

export interface ScanQrFromCameraOptions extends CameraScanSeams {
  /** プレビューを流す video 要素（背面カメラの映像を表示しつつ、各フレームを detect にかける）。 */
  readonly video: HTMLVideoElement;
  /** QR を 1 件読めたら呼ぶ。値は raw 文字列（= dev-server URL）。読み取り後はカメラを止める。 */
  readonly onResult: (text: string) => void;
  /** 取得・デコードの失敗を通知する。カメラ拒否やデコーダ生成失敗もここに来る。 */
  readonly onError?: (error: unknown) => void;
}

/** 起動中スキャンのハンドル。`cancel()` でカメラとフレームループを確実に畳む。 */
export interface QrScanController {
  cancel(): void;
}

/** この環境でカメラ QR スキャンが使えるか（`BarcodeDetector` と `getUserMedia` の両方が要る）。 */
export function isCameraScanSupported(scope: unknown = globalThis): boolean {
  const g = scope as {
    BarcodeDetector?: unknown;
    navigator?: { mediaDevices?: { getUserMedia?: unknown } };
    isSecureContext?: boolean;
  };
  return (
    typeof g.BarcodeDetector === 'function' &&
    typeof g.navigator?.mediaDevices?.getUserMedia === 'function'
  );
}

/** 既定 seam（実ブラウザ）。テストは {@link ScanQrFromCameraOptions} 経由で全て差し替える。 */
function defaultGetUserMedia(constraints: MediaStreamConstraints): Promise<MediaStream> {
  return navigator.mediaDevices.getUserMedia(constraints);
}
function defaultCreateDetector(): BarcodeDetectorLike {
  // 標準 API。lib.dom に型が無いことがあるので構造に絞って呼ぶ。
  const ctor = (
    globalThis as unknown as {
      BarcodeDetector: new (o: { formats: string[] }) => BarcodeDetectorLike;
    }
  ).BarcodeDetector;
  return new ctor({ formats: ['qr_code'] });
}

/**
 * 背面カメラを開き、フレームごとに `BarcodeDetector` で QR を探す。1 件読めたら
 * {@link ScanQrFromCameraOptions.onResult} を呼んでカメラを止める（one-shot）。返した
 * {@link QrScanController.cancel} はいつ呼んでも安全で、取得中・ループ中のどちらでも track と
 * フレーム予約を畳む。
 */
export function scanQrFromCamera(options: ScanQrFromCameraOptions): QrScanController {
  const getUserMedia = options.getUserMedia ?? defaultGetUserMedia;
  const createDetector = options.createDetector ?? defaultCreateDetector;
  const requestFrame = options.requestFrame ?? ((cb) => requestAnimationFrame(cb));
  const cancelFrame = options.cancelFrame ?? ((h) => cancelAnimationFrame(h));

  let stopped = false;
  let stream: MediaStream | undefined;
  let frameHandle: number | undefined;

  const stopCamera = (): void => {
    if (frameHandle != null) {
      cancelFrame(frameHandle);
      frameHandle = undefined;
    }
    for (const track of stream?.getTracks() ?? []) track.stop();
    stream = undefined;
    // 表示中の映像を外す（次回 mount をきれいに保つ）。
    try {
      options.video.srcObject = null;
    } catch {
      // 一部環境で setter が無くても無害なので握りつぶす。
    }
  };

  const finish = (text: string): void => {
    if (stopped) return;
    stopped = true;
    stopCamera();
    options.onResult(text);
  };

  const fail = (error: unknown): void => {
    if (stopped) return;
    stopped = true;
    stopCamera();
    options.onError?.(error);
  };

  void (async () => {
    try {
      // 背面カメラ優先（書類/画面の QR を写すため）。
      const acquired = await getUserMedia({
        video: { facingMode: 'environment' },
        audio: false,
      });
      if (stopped) {
        // 取得中に cancel された：開いた track を即畳む。
        for (const track of acquired.getTracks()) track.stop();
        return;
      }
      stream = acquired;
      options.video.srcObject = acquired;
      await options.video.play();
      if (stopped) {
        stopCamera();
        return;
      }
      const detector = createDetector();
      const tick = (): void => {
        if (stopped) return;
        detector.detect(options.video).then((codes) => {
          if (stopped) return;
          const hit = codes.find((c) => c.rawValue !== '');
          if (hit != null) {
            finish(hit.rawValue);
            return;
          }
          frameHandle = requestFrame(tick);
        }, fail);
      };
      tick();
    } catch (error) {
      fail(error);
    }
  })();

  return {
    cancel(): void {
      if (stopped) return;
      stopped = true;
      stopCamera();
    },
  };
}
