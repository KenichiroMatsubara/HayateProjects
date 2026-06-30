import Foundation
import UIKit
import VisionKit

/// QR スキャナ capability（ADR-0125）の iOS leaf 実体（Swift 側）。
///
/// Rust の `hayate_adapter_ios::qr_scanner::IosQrScanner` が `hayate_ios_qr_*` C FFI 経由で呼ぶ。
/// `audio_output`（`hayate_ios_audio_*`）と同型に、Swift が UIKit/VisionKit を持ち Rust は ObjC-free
/// （ADR-0114 shape 1）。VisionKit `DataScannerViewController`（iOS 16+）でカメラから QR を 1 件読み、
/// payload 文字列を返す。`MobileQrScanner` facade を通じて Android（Code Scanner）と単一 API に揃う。
///
/// 実機検証（カメラ実機 + iOS SDK）はサンドボックス外。

/// 提示中の coordinator を生かしておく強参照（`DataScannerViewController` は delegate を弱参照する）。
private var activeQrCoordinator: AnyObject?

/// 最前面の view controller（modal を辿った先）を返す。スキャナの提示元に使う。
private func topmostViewController() -> UIViewController? {
    let scenes = UIApplication.shared.connectedScenes
    let windowScene = (scenes.first { $0.activationState == .foregroundActive } as? UIWindowScene)
        ?? (scenes.first as? UIWindowScene)
    var top = windowScene?.windows.first(where: { $0.isKeyWindow })?.rootViewController
        ?? windowScene?.windows.first?.rootViewController
    while let presented = top?.presentedViewController {
        top = presented
    }
    return top
}

/// VisionKit スキャナを提示し、最初の QR（or キャンセル）で `onResult` を呼ぶ薄い coordinator。
@available(iOS 16.0, *)
private final class QrScanCoordinator: NSObject, DataScannerViewControllerDelegate {
    private let scanner: DataScannerViewController
    private let onResult: (String?) -> Void
    private var finished = false

    init(onResult: @escaping (String?) -> Void) {
        self.onResult = onResult
        scanner = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .balanced,
            isHighFrameRateTrackingEnabled: false,
            isPinchToZoomEnabled: true,
            isGuidanceEnabled: true,
            isHighlightingEnabled: true
        )
        super.init()
        scanner.delegate = self
    }

    /// nav controller に包んで「キャンセル」ボタンを付け、提示してスキャンを開始する。
    func present(from presenter: UIViewController) {
        let nav = UINavigationController(rootViewController: scanner)
        scanner.title = "QR をスキャン"
        scanner.navigationItem.leftBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .cancel,
            target: self,
            action: #selector(cancelTapped)
        )
        presenter.present(nav, animated: true) { [weak self] in
            try? self?.scanner.startScanning()
        }
    }

    @objc private func cancelTapped() {
        finish(with: nil)
    }

    func dataScanner(
        _ dataScanner: DataScannerViewController,
        didAdd addedItems: [RecognizedItem],
        allItems: [RecognizedItem]
    ) {
        guard !finished else { return }
        for item in addedItems {
            if case let .barcode(barcode) = item, let value = barcode.payloadStringValue {
                finish(with: value)
                return
            }
        }
    }

    private func finish(with value: String?) {
        guard !finished else { return }
        finished = true
        scanner.stopScanning()
        // scanner は nav controller 内なので、これで提示中の nav ごと閉じる。
        scanner.dismiss(animated: true) { [onResult] in
            onResult(value)
        }
    }
}

/// Rust（`IosQrScanner::scan`）から worker スレッドで呼ばれる同期入口。main でスキャナを提示し、
/// 結果まで呼び出しスレッドをブロックして返す。読み取れたら malloc 済み C 文字列（呼び側が
/// `hayate_ios_qr_free` で解放）、キャンセル / 非対応端末は `nil`。
@_cdecl("hayate_ios_qr_scan")
public func hayate_ios_qr_scan() -> UnsafeMutablePointer<CChar>? {
    let semaphore = DispatchSemaphore(value: 0)
    var result: String?

    DispatchQueue.main.async {
        // iOS 16+ かつ端末が DataScanner 対応・利用可能でなければ結果なしで返す。
        guard #available(iOS 16.0, *),
            DataScannerViewController.isSupported,
            DataScannerViewController.isAvailable,
            let presenter = topmostViewController()
        else {
            semaphore.signal()
            return
        }
        let coordinator = QrScanCoordinator { value in
            result = value
            activeQrCoordinator = nil
            semaphore.signal()
        }
        activeQrCoordinator = coordinator
        coordinator.present(from: presenter)
    }

    semaphore.wait()
    guard let value = result else { return nil }
    // Rust 側が CStr へコピー後に hayate_ios_qr_free で free する所有権移譲。
    return strdup(value)
}

/// `hayate_ios_qr_scan` が返した C 文字列を解放する（Rust が読み終えたら呼ぶ）。
@_cdecl("hayate_ios_qr_free")
public func hayate_ios_qr_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}
