//! コードポイント → フォールバックファミリの coverage テーブル（データ駆動）。
//!
//! オンデマンドフォント解決の単一の真実。レイヤ分割は、*コードポイント → ファミリ*
//! が core 所有でプラットフォーム非依存のドメイン知識、*ファミリ → ソース*（web は
//! CDN URL、native は OS 検索）が各アダプタの責務（ADR-0043）。フォント追加が
//! データ変更だけで済むよう、ソート済み coverage テーブルに集約してある。各アダプタの
//! 横断整合テストが、解決されうる全ファミリが実際に取得可能であることを保証する（ADR-0101）。
//!
//! 解決は呼び出し側（`text::lower_glyph_runs`）で `.notdef` にゲートされる。同梱の
//! デフォルトフォントが描画できなかったグリフについてのみ「どのフォントがこのコードポイントを
//! カバーするか」を問う。このゲートがあるからこそ、下記の emoji レンジを記号面全体を覆うほど
//! 意図的に広く取っても、ベースフォントが既に持つグリフを誤ルーティングすることがない。

/// 連続した両端含む Unicode レンジと、それをカバーすると見込まれるフォールバックファミリの対応。
#[derive(Clone, Copy, Debug)]
pub struct Coverage {
    pub start: u32,
    pub end: u32,
    pub family: &'static str,
}

const fn cov(start: u32, end: u32, family: &'static str) -> Coverage {
    Coverage { start, end, family }
}

/// `.notdef` の emoji コードポイントが解決されるモノクロ emoji フォールバック
/// ファミリ（ADR-0101）。coverage テーブルの emoji 行が指すファミリの単一の真実で、
/// アダプタ側のレンダラ別 emoji ディスパッチ（[`upgrades_to_color_emoji`]）もこの
/// 同一識別子を参照する。ファミリ → ソースの対応はアダプタの責務（ADR-0043）。
pub const EMOJI_FALLBACK_FAMILY: &str = "Noto Emoji";

/// coverage テーブル。**`start` でソート済みかつ非重複**（`table_is_sorted_and_non_overlapping`
/// が保証）。編集時もソートを保つこと。
///
/// emoji レンジは意図的に記号面全体を覆う（モジュールドキュメント参照）。内側に少数の
/// 非 emoji コードポイントが含まれるが、解決は `.notdef` ゲート済みなので、そもそも描画できなかった
/// グリフに対してしか参照されない。
pub const FONT_COVERAGE: &[Coverage] = &[
    // ── Hebrew ───────────────────────────────────────────────────────────
    cov(0x0590, 0x05FF, "Noto Sans Hebrew"), // Hebrew
    // ── Arabic ───────────────────────────────────────────────────────────
    cov(0x0600, 0x06FF, "Noto Sans Arabic"), // Arabic
    cov(0x0750, 0x077F, "Noto Sans Arabic"), // Arabic Supplement
    cov(0x08A0, 0x08FF, "Noto Sans Arabic"), // Arabic Extended-A
    // ── Devanagari ───────────────────────────────────────────────────────
    cov(0x0900, 0x097F, "Noto Sans Devanagari"), // Devanagari
    // ── Thai ─────────────────────────────────────────────────────────────
    cov(0x0E00, 0x0E7F, "Noto Sans Thai"), // Thai
    // ── Korean ───────────────────────────────────────────────────────────
    cov(0x1100, 0x11FF, "Noto Sans KR"), // Hangul Jamo
    // ── Emoji / symbols (BMP) ────────────────────────────────────────────
    cov(0x2600, 0x27BF, EMOJI_FALLBACK_FAMILY), // Misc Symbols + Dingbats（☀ ✨ ➡ …）
    cov(0x2B00, 0x2BFF, EMOJI_FALLBACK_FAMILY), // Misc Symbols and Arrows（⭐ ⬛ …）
    // ── CJK (BMP) ────────────────────────────────────────────────────────
    cov(0x2E80, 0x2EFF, "Noto Sans JP"), // CJK Radicals Supplement
    cov(0x2F00, 0x2FDF, "Noto Sans JP"), // Kangxi Radicals
    cov(0x3000, 0x303F, "Noto Sans JP"), // CJK Symbols and Punctuation
    cov(0x3040, 0x309F, "Noto Sans JP"), // Hiragana
    cov(0x30A0, 0x30FF, "Noto Sans JP"), // Katakana
    cov(0x3130, 0x318F, "Noto Sans KR"), // Hangul Compatibility Jamo
    cov(0x31F0, 0x31FF, "Noto Sans JP"), // Katakana Phonetic Extensions
    cov(0x3400, 0x4DBF, "Noto Sans JP"), // CJK Unified Ideographs Ext A
    cov(0x4E00, 0x9FFF, "Noto Sans JP"), // CJK Unified Ideographs
    // ── Devanagari Extended / Korean (A-zone) ────────────────────────────
    cov(0xA8E0, 0xA8FF, "Noto Sans Devanagari"), // Devanagari Extended
    cov(0xA960, 0xA97F, "Noto Sans KR"),         // Hangul Jamo Extended-A
    cov(0xAC00, 0xD7AF, "Noto Sans KR"),         // Hangul Syllables
    cov(0xD7B0, 0xD7FF, "Noto Sans KR"),         // Hangul Jamo Extended-B
    // ── CJK Compatibility / presentation forms ───────────────────────────
    cov(0xF900, 0xFAFF, "Noto Sans JP"),     // CJK Compatibility Ideographs
    cov(0xFB1D, 0xFB4F, "Noto Sans Hebrew"), // Hebrew Presentation Forms
    cov(0xFB50, 0xFDFF, "Noto Sans Arabic"), // Arabic Presentation Forms-A
    cov(0xFE70, 0xFEFF, "Noto Sans Arabic"), // Arabic Presentation Forms-B
    // ── Emoji / symbols (SMP) ────────────────────────────────────────────
    // Mahjong / Dominoes / Playing Cards / Enclosed Alphanumerics（1F1E6..1F1FF の
    // 地域インジケータ旗 🇦–🇿 を含む）/ Misc Pictographs / Emoticons / Transport /
    // Supplemental Pictographs / Symbols Extended-A。
    cov(0x1F000, 0x1FAFF, EMOJI_FALLBACK_FAMILY),
    // ── CJK Unified Ideographs Extensions (SIP) ──────────────────────────
    cov(0x20000, 0x2A6DF, "Noto Sans JP"), // Ext B
    cov(0x2A700, 0x2B73F, "Noto Sans JP"), // Ext C
    cov(0x2B740, 0x2B81F, "Noto Sans JP"), // Ext D
    cov(0x2B820, 0x2CEAF, "Noto Sans JP"), // Ext E
    cov(0x2CEB0, 0x2EBEF, "Noto Sans JP"), // Ext F
];

/// 同梱のデフォルトフォントが `.notdef` で描画するコードポイントを、それをカバーすると
/// 見込まれるフォールバックファミリへ解決する。デフォルトフォント自身がカバーする見込みなら
/// `None` を返す。
///
/// ファミリ名は、各プラットフォームアダプタが自身の ファミリ名 → フォントソース テーブルで
/// 使うキー（ADR-0043）。
pub fn family_for_codepoint(cp: u32) -> Option<&'static str> {
    // `start <= cp` を満たす最大のレンジを取り、実際に `cp` を含むか確認する。
    let idx = FONT_COVERAGE.partition_point(|c| c.start <= cp);
    let c = FONT_COVERAGE.get(idx.checked_sub(1)?)?;
    (cp <= c.end).then_some(c.family)
}

/// このテーブルが解決しうる相異なるフォールバックファミリをソートして返す。アダプタは
/// 各ファミリが自身のマニフェストで取得可能であることの表明に使う（横断整合、ADR-0101）。
pub fn coverage_families() -> Vec<&'static str> {
    let mut families: Vec<&'static str> = FONT_COVERAGE.iter().map(|c| c.family).collect();
    families.sort_unstable();
    families.dedup();
    families
}

/// `family` が emoji フォールバックファミリ（[`EMOJI_FALLBACK_FAMILY`]）か。
pub fn is_emoji_fallback_family(family: &str) -> bool {
    family == EMOJI_FALLBACK_FAMILY
}

/// レンダラがカラーグリフを描けるとき、emoji フォールバックファミリはカラー
/// ビルドへ格上げしてよいか（ADR-0101）。`paints_color_glyphs` は呼び出し側
/// アダプタが持つレンダラ能力。判定は「どの要求ファミリが emoji フォールバックか」
/// という core 所有のドメイン知識だけに依存し、モノクロ/カラービルドの実体名や
/// 取得先（URL・OS 検索）はアダプタの調達責務（ADR-0043）に委ねる。Web のカラー
/// 経路（Vello）も将来の Android 等のカラー対応ペインタも同一規則を共有する。
pub fn upgrades_to_color_emoji(requested_family: &str, paints_color_glyphs: bool) -> bool {
    paints_color_glyphs && is_emoji_fallback_family(requested_family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_non_overlapping() {
        for pair in FONT_COVERAGE.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(a.start <= a.end, "range {a:?} is inverted");
            assert!(
                a.end < b.start,
                "ranges out of order or overlapping: {a:?} then {b:?}"
            );
        }
        // partition_point の二分探索は上記の不変条件に依存する。
    }

    #[test]
    fn existing_scripts_resolve_to_their_families() {
        assert_eq!(family_for_codepoint(0x3042), Some("Noto Sans JP")); // あ
        assert_eq!(family_for_codepoint(0x4E00), Some("Noto Sans JP")); // 一
        assert_eq!(family_for_codepoint(0x20000), Some("Noto Sans JP")); // 𠀀 (Ext B)
        assert_eq!(family_for_codepoint(0xAC00), Some("Noto Sans KR")); // 가
        assert_eq!(family_for_codepoint(0x0627), Some("Noto Sans Arabic")); // ا
        assert_eq!(family_for_codepoint(0x0E01), Some("Noto Sans Thai")); // ก
        assert_eq!(family_for_codepoint(0x0915), Some("Noto Sans Devanagari")); // क
        assert_eq!(family_for_codepoint(0x05D0), Some("Noto Sans Hebrew")); // א
    }

    #[test]
    fn emoji_across_the_repertoire_resolve_to_monochrome_noto_emoji() {
        // 一部の抜粋ではなく emoji レパートリ全体がモノクロ Noto Emoji へ解決される必要がある
        // （tiny-skia は COLR/CBDT のカラーグリフを描画できない。ADR-0101）。以下は emoji を含む
        // 全ブロックにまたがる。
        for cp in [
            0x2600u32, // ☀ Misc Symbols
            0x2728,    // ✨ Dingbats
            0x2B50,    // ⭐ Misc Symbols and Arrows
            0x1F004,   // 🀄 Mahjong
            0x1F0CF,   // 🃏 Playing Cards
            0x1F1EF,   // 🇯 Regional Indicator (flags)
            0x1F319,   // 🌙 Misc Pictographs（テーマ切替グリフ）
            0x1F600,   // 😀 Emoticons
            0x1F680,   // 🚀 Transport and Map
            0x1F9E0,   // 🧠 Supplemental Pictographs
            0x1FAE0,   // 🫠 Symbols and Pictographs Extended-A
        ] {
            assert_eq!(
                family_for_codepoint(cp),
                Some("Noto Emoji"),
                "U+{cp:04X} should route to monochrome Noto Emoji"
            );
        }
    }

    #[test]
    fn latin_and_gaps_resolve_to_none() {
        assert_eq!(family_for_codepoint(0x0041), None); // 'A'
        assert_eq!(family_for_codepoint(0x00E9), None); // 'é'
        assert_eq!(family_for_codepoint(0x25FF), None); // U+2600 の直下
        assert_eq!(family_for_codepoint(0x0000), None); // テーブル全体より下
    }

    #[test]
    fn coverage_families_are_unique_and_sorted() {
        let families = coverage_families();
        let mut expected = families.clone();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(families, expected);
        assert!(families.contains(&"Noto Emoji"));
        assert!(families.contains(&"Noto Sans JP"));
    }
}
