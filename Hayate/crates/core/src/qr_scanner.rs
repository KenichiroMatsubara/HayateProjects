//! qr scanner capability 契約（ADR-0125）。モデル: カメラで QR/バーコードを 1 件読み取る
//! （`scan -> code?`）。`file_picker` と同型の **async-UI 一発取得**（システム UI を出し、結果を
//! 1 件返すかキャンセル）で、キャンセルは `Ok(None)`。
//!
//! 契約の正本はここ（platform 非依存）。leaf（android = Google Code Scanner / ios = VisionKit
//! `DataScannerViewController`）はこの trait を満たすだけで別契約を切らない。Mobile Family Adapter
//! （ADR-0117）の `MobileQrScanner` facade が `cfg(target_os)` で leaf を解決するので、上位は
//! iOS/Android を**単一 API**で扱える。Web は family-of-1 として別 leaf（`@miharashi/host-web` の
//! `scanQrFromCamera`、`BarcodeDetector`）を直接置く（ADR-0117）。

use crate::capability::CapabilityError;

/// スキャンで読み取れた 1 件のコード（scaffold では raw 文字列のみ。format/座標は実装時に足す）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedCode {
    /// デコード結果の生文字列（QR が URL なら dev-server の LAN URL がそのまま入る）。
    pub value: String,
}

/// カメラで QR/バーコードを 1 件読み取るシステム UI。読み取れたら `Ok(Some(code))`、ユーザが
/// 閉じた場合は `Ok(None)`。`file_picker` と同じく**呼ぶと native UI が出て結果が返るまでブロック
/// する**（呼び側は UI スレッド以外から呼ぶ）。
pub trait QrScanner {
    fn scan(&mut self) -> Result<Option<ScannedCode>, CapabilityError>;
}
