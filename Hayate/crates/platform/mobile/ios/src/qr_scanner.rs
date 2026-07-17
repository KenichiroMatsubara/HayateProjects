//! iOS leaf の QR スキャナ実装（ADR-0125）。
//!
//! 契約の正本 [`QrScanner`]（`hayate_core`）を満たす iOS 実装。バックエンドは **VisionKit
//! `DataScannerViewController`**（iOS 16+）— Swift ホストがカメラ付きスキャナ UI を出し、読み取れた
//! payload 文字列を返す。`audio_output`（`hayate_ios_audio_*`）と同型に、native 呼び出しは
//! `hayate_ios_qr_*` の薄い FFI に閉じ、Swift 側（ios-app の `QrScanner.swift`）が `@_cdecl` で
//! 実装する（ADR-0114 shape 1：Swift が UIKit/VisionKit を持ち、Rust は ObjC-free）。
//!
//! Android leaf（Google Code Scanner）と対称に `MobileQrScanner` facade へ載り、上位は iOS/Android を
//! 単一 API で扱う。Web は family-of-1 で別 leaf（`BarcodeDetector`）。
//!
//! 実機検証（Mac/iOS SDK + VisionKit + カメラ実機）はサンドボックス外。純粋部はホストで、FFI glue は
//! ソース走査ガード（`tests/qr_scanner_encapsulation.rs`）で封じ込める。

#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::{QrScanner, ScannedCode};

#[cfg(target_os = "ios")]
pub use platform::IosQrScanner;

#[cfg(target_os = "ios")]
mod platform {
    use super::*;
    use hayate_core::capability::CapabilityError;
    use std::ffi::CStr;
    use std::os::raw::c_char;

    // Swift ホスト（`QrScanner.swift`）が VisionKit で実装する native 境界。`scan` は worker
    // スレッドから呼ばれ、Swift が main でスキャナ UI を出して結果まで**ブロック**して返す。
    // 戻り値は malloc 済み C 文字列（読み取り payload）か null（キャンセル / 非対応）。所有権は
    // 呼び側に移り、使い終えたら `hayate_ios_qr_free` で解放する。
    extern "C" {
        fn hayate_ios_qr_scan() -> *mut c_char;
        fn hayate_ios_qr_free(ptr: *mut c_char);
    }

    /// VisionKit（`DataScannerViewController`）でカメラ QR を 1 件読む iOS leaf。役割名は
    /// 「QR/バーコードのスキャナ UI」で、Android leaf（Code Scanner）と対称。
    #[derive(Default)]
    pub struct IosQrScanner;

    impl QrScanner for IosQrScanner {
        fn scan(&mut self) -> Result<Option<ScannedCode>, CapabilityError> {
            let ptr = unsafe { hayate_ios_qr_scan() };
            if ptr.is_null() {
                // ユーザがスキャナを閉じた（or 非対応端末）。結果なしに畳む。
                return Ok(None);
            }
            // C 文字列を Rust 文字列へコピーしてから、Swift が malloc した領域を解放する。
            let value = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { hayate_ios_qr_free(ptr) };
            Ok(Some(ScannedCode { value }))
        }
    }
}
