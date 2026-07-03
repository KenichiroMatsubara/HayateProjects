/// プラットフォームフォント取得シーム（ADR-0132 スライス2）。アダプタは URL 解決・非同期
/// fetch・リトライバックオフをラップし、[`ElementTree::drive_font_requests`](crate::ElementTree::drive_font_requests)
/// が欠落フォントを検出するたびに呼ぶ `request` 以外のことはしない。`ImeBridge` と同じ
/// 形（core 所有・同期・単方向の発火のみ）を踏襲する。取得の成否は
/// `ElementTree::register_font` / `ElementTree::font_fetch_failed` で core へ非同期に
/// 報告する（`request` 自体は結果を返さない）。
pub trait FontFetcher {
    fn request(&mut self, family: &str);
}
