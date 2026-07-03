/// GPU（wgpu）経路専用の提示サーフェスの契約（ADR-0132 スライス3）。`ImeBridge` /
/// `FontFetcher` と統一の置き場所（core 所有の capability contract）。`RenderHost`
/// （`hayate-app-host`）はこの trait 越しにしかアダプタの canvas / window 資源へ触れない。
///
/// スコープは GPU 経路専用。tiny-skia（CPU）経路は canvas 2D コンテキストへの直接 blit
/// という別の資源型を要り、この trait には乗らない（ADR-0048/0118、アダプタに残置）。
/// `RenderHost` が必要とするのは、レンダラー初期化を試すたびに渡す複製と、実行時
/// フォールバックでの再サイズ確認だけなので、trait はその最小面だけを持つ。
pub trait Surface: Clone {
    /// 提示先の現在の物理ピクセル幅。
    fn width(&self) -> u32;
    /// 提示先の現在の物理ピクセル高さ。
    fn height(&self) -> u32;
}
