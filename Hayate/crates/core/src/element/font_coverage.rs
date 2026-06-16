//! Data-driven codepoint → fallback-family coverage table.
//!
//! Single source of truth for on-demand font routing. ADR-0042 fixes the
//! layering: *codepoint → family* is core-owned, platform-independent domain
//! knowledge; *family → source* (CDN URL on web, OS lookup on native) is each
//! adapter's job (ADR-0043). This table used to be a hand-written `match` with
//! one arm per script; it is now a single sorted coverage table so that adding
//! a font is a data change, and a cross-layer integrity test (in each adapter)
//! guarantees every routed family is actually procurable (ADR-0101).
//!
//! Resolution is gated on `.notdef` at the call site (`text::lower_glyph_runs`):
//! we only ask "which font covers this codepoint?" for glyphs the bundled
//! default font failed to render. That gate is what lets the emoji ranges below
//! be deliberately generous — covering whole symbol planes — without ever
//! mis-routing a glyph the base font already has.

/// A contiguous, inclusive Unicode range mapped to the fallback family expected
/// to cover it.
#[derive(Clone, Copy, Debug)]
pub struct Coverage {
    pub start: u32,
    pub end: u32,
    pub family: &'static str,
}

const fn cov(start: u32, end: u32, family: &'static str) -> Coverage {
    Coverage { start, end, family }
}

/// The coverage table, **sorted by `start` and non-overlapping** (enforced by
/// `table_is_sorted_and_non_overlapping`). Keep it sorted when editing.
///
/// Emoji ranges cover whole symbol planes on purpose (see module docs): a few
/// non-emoji codepoints fall inside them, but routing is `.notdef`-gated so they
/// are only ever consulted for glyphs that did not render anyway.
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
    cov(0x2600, 0x27BF, "Noto Emoji"), // Misc Symbols + Dingbats (☀ ✨ ➡ …)
    cov(0x2B00, 0x2BFF, "Noto Emoji"), // Misc Symbols and Arrows (⭐ ⬛ …)
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
    // Mahjong, Dominoes, Playing Cards, Enclosed Alphanumerics (incl. the
    // regional-indicator flags 🇦–🇿 at 1F1E6..1F1FF), Misc Pictographs,
    // Emoticons, Transport, Supplemental Pictographs, and Symbols Extended-A.
    cov(0x1F000, 0x1FAFF, "Noto Emoji"),
    // ── CJK Unified Ideographs Extensions (SIP) ──────────────────────────
    cov(0x20000, 0x2A6DF, "Noto Sans JP"), // Ext B
    cov(0x2A700, 0x2B73F, "Noto Sans JP"), // Ext C
    cov(0x2B740, 0x2B81F, "Noto Sans JP"), // Ext D
    cov(0x2B820, 0x2CEAF, "Noto Sans JP"), // Ext E
    cov(0x2CEB0, 0x2EBEF, "Noto Sans JP"), // Ext F
];

/// Resolve a codepoint to the fallback family expected to cover it when the
/// bundled default font renders it as `.notdef`. Returns `None` when the
/// default font is expected to cover the codepoint itself.
///
/// Family names are the keys each platform adapter uses in its own
/// family-name → font-source table (ADR-0043).
pub fn family_for_codepoint(cp: u32) -> Option<&'static str> {
    // Largest range whose `start <= cp`; then check it actually contains `cp`.
    let idx = FONT_COVERAGE.partition_point(|c| c.start <= cp);
    let c = FONT_COVERAGE.get(idx.checked_sub(1)?)?;
    (cp <= c.end).then_some(c.family)
}

/// Every distinct fallback family this table can route to, sorted. Adapters use
/// it to assert each routed family is procurable in their own manifest
/// (cross-layer integrity, ADR-0101).
pub fn coverage_families() -> Vec<&'static str> {
    let mut families: Vec<&'static str> = FONT_COVERAGE.iter().map(|c| c.family).collect();
    families.sort_unstable();
    families.dedup();
    families
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
        // partition_point binary search relies on the above invariant.
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
        // The whole emoji repertoire — not a hand-picked subset — must route to
        // the monochrome Noto Emoji (tiny-skia cannot paint COLR/CBDT colour
        // glyphs; ADR-0101 / issue #329). These span every emoji-bearing block,
        // including the ones the original narrow fix missed.
        for cp in [
            0x2600u32, // ☀ Misc Symbols
            0x2728,    // ✨ Dingbats
            0x2B50,    // ⭐ Misc Symbols and Arrows
            0x1F004,   // 🀄 Mahjong
            0x1F0CF,   // 🃏 Playing Cards
            0x1F1EF,   // 🇯 Regional Indicator (flags)
            0x1F319,   // 🌙 Misc Pictographs (the theme-toggle glyph)
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
        assert_eq!(family_for_codepoint(0x25FF), None); // just below U+2600
        assert_eq!(family_for_codepoint(0x0000), None); // below the whole table
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
