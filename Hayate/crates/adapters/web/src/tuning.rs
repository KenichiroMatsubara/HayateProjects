//! web Platform Adapter 向け開発用 `tuning.json` パーサ。
//!
//! 正準のデフォルト値は Rust の `const`（`scroll_drag::physics` と `hayate_core`
//! の scene-build / selection-chrome）。本モジュールは実行時にそれらを上書きする。
//! ホストが `tuning.json` を fetch し、その文字列を
//! [`HayateElementRenderer::set_tuning`](crate::canvas) に渡してここで解釈する。
//! 全フィールドは任意で、JSON に存在するキーのみがデフォルトを上書きする。不正な
//! ファイルは全体を拒否し、呼び出し側はデフォルトを保つ。`hayate-core` は実行時
//! serde 依存を持たない方針のため、パースはアダプタ側に置く。

use hayate_core::{ChromeTuning, Color};
use serde::Deserialize;

use crate::scroll_drag::ScrollPhysicsTuning;

/// `tuning.json` のトップレベル形状。任意の 2 セクション。
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TuningJson {
    pub scroll: Option<ScrollJson>,
    pub chrome: Option<ChromeJson>,
}

impl TuningJson {
    /// チューニングファイルをパースする。手書きのファイルは JSON5-lite で、`//`
    /// `/* */` コメントと末尾カンマを許容する（各値を日本語で注釈したり行を
    /// コメントアウトできる）。これらを除去して素の JSON にし `serde_json` に
    /// 渡す。本番 wasm はこのファイルを読まないため重い JSON5 パーサは含めない。
    /// 不正な JSON や未知キーは `Err` とし、呼び出し側はコンパイル時デフォルトへ
    /// フォールバックできる。
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(&to_plain_json(text))
    }

    /// マージ済みスクロール物理値（存在キーでデフォルトを上書き）。
    pub fn scroll_tuning(&self) -> ScrollPhysicsTuning {
        self.scroll.as_ref().map(ScrollJson::merged).unwrap_or_default()
    }

    /// マージ済み chrome 値（存在キーでデフォルトを上書き）。
    pub fn chrome_tuning(&self) -> ChromeTuning {
        self.chrome.as_ref().map(ChromeJson::merged).unwrap_or_default()
    }
}

/// `Option` をデフォルトフィールドに上書きする。
fn overlay<T>(slot: &mut T, value: Option<T>) {
    if let Some(v) = value {
        *slot = v;
    }
}

/// `[r, g, b, a]`（0..1）の JSON 配列を core の [`Color`] に変換する。
fn color_from(rgba: [f64; 4]) -> Color {
    Color::new(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// JSON5-lite を素の JSON へ。`//` 行コメントと `/* … */` ブロックコメントを
/// 除去し、`}`/`]` 前の末尾カンマを落とす。文字列リテラルは保護され、引用値内の
/// `//` や `,` は触らない。マルチバイト（日本語）コメントは破棄されるだけで出力を
/// 壊さない。
fn to_plain_json(src: &str) -> String {
    strip_trailing_commas(&strip_comments(src))
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                // 行コメント: 改行まで読み飛ばす（改行は残す）。
                chars.next();
                while let Some(&n) = chars.peek() {
                    if n == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                // ブロックコメント: 閉じ `*/` まで読み飛ばす。
                chars.next();
                let mut prev = '\0';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn strip_trailing_commas(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            // 次の非空白文字がオブジェクト/配列を閉じるカンマは末尾カンマ。厳格な
            // serde_json でも通るよう落とす。
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScrollJson {
    slop_px: Option<f32>,
    deceleration_rate: Option<f32>,
    max_release_velocity: Option<f32>,
    min_velocity: Option<f32>,
    sample_window_ms: Option<f64>,
    rubber_band_c: Option<f32>,
    spring_stiffness: Option<f32>,
    spring_damping: Option<f32>,
    spring_rest_offset: Option<f32>,
    spring_rest_velocity: Option<f32>,
}

impl ScrollJson {
    fn merged(&self) -> ScrollPhysicsTuning {
        let mut d = ScrollPhysicsTuning::default();
        overlay(&mut d.slop_px, self.slop_px);
        overlay(&mut d.deceleration_rate, self.deceleration_rate);
        overlay(&mut d.max_release_velocity, self.max_release_velocity);
        overlay(&mut d.min_velocity, self.min_velocity);
        overlay(&mut d.sample_window_ms, self.sample_window_ms);
        overlay(&mut d.rubber_band_c, self.rubber_band_c);
        overlay(&mut d.spring_stiffness, self.spring_stiffness);
        overlay(&mut d.spring_damping, self.spring_damping);
        overlay(&mut d.spring_rest_offset, self.spring_rest_offset);
        overlay(&mut d.spring_rest_velocity, self.spring_rest_velocity);
        d
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChromeJson {
    scrollbar_thickness: Option<f32>,
    scrollbar_track_margin: Option<f32>,
    scrollbar_min_thumb_length: Option<f32>,
    scrollbar_thumb_color: Option<[f64; 4]>,
    scrollbar_thumb_opacity: Option<f32>,
    scrollbar_indicator_thickness: Option<f32>,
    scrollbar_indicator_color: Option<[f64; 4]>,
    scrollbar_indicator_opacity: Option<f32>,
    selection_highlight_color: Option<[f32; 4]>,
    composition_underline_thin: Option<f32>,
    composition_underline_thick: Option<f32>,
    placeholder_alpha: Option<f64>,
    toolbar_corner_radius: Option<f32>,
}

impl ChromeJson {
    fn merged(&self) -> ChromeTuning {
        let mut d = ChromeTuning::default();
        overlay(&mut d.scrollbar_thickness, self.scrollbar_thickness);
        overlay(&mut d.scrollbar_track_margin, self.scrollbar_track_margin);
        overlay(&mut d.scrollbar_min_thumb_length, self.scrollbar_min_thumb_length);
        overlay(&mut d.scrollbar_thumb_color, self.scrollbar_thumb_color.map(color_from));
        overlay(&mut d.scrollbar_thumb_opacity, self.scrollbar_thumb_opacity);
        overlay(&mut d.scrollbar_indicator_thickness, self.scrollbar_indicator_thickness);
        overlay(&mut d.scrollbar_indicator_color, self.scrollbar_indicator_color.map(color_from));
        overlay(&mut d.scrollbar_indicator_opacity, self.scrollbar_indicator_opacity);
        overlay(&mut d.selection_highlight_color, self.selection_highlight_color);
        overlay(&mut d.composition_underline_thin, self.composition_underline_thin);
        overlay(&mut d.composition_underline_thick, self.composition_underline_thick);
        overlay(&mut d.placeholder_alpha, self.placeholder_alpha);
        overlay(&mut d.toolbar_corner_radius, self.toolbar_corner_radius);
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_yields_all_defaults() {
        let t = TuningJson::parse("{}").unwrap();
        assert_eq!(t.scroll_tuning(), ScrollPhysicsTuning::default());
        assert_eq!(t.chrome_tuning(), ChromeTuning::default());
    }

    #[test]
    fn only_present_keys_override_the_defaults() {
        let t = TuningJson::parse(r#"{ "scroll": { "deceleration_rate": 0.99 } }"#).unwrap();
        let s = t.scroll_tuning();
        // 存在する 1 キーは変わり…
        assert_eq!(s.deceleration_rate, 0.99);
        // …他はデフォルト const のまま。
        assert_eq!(s.slop_px, ScrollPhysicsTuning::default().slop_px);
        assert_eq!(s.spring_damping, ScrollPhysicsTuning::default().spring_damping);
    }

    #[test]
    fn color_arrays_become_core_colors() {
        let t = TuningJson::parse(r#"{ "chrome": { "scrollbar_thumb_color": [1.0, 0.0, 0.0, 1.0] } }"#).unwrap();
        assert_eq!(t.chrome_tuning().scrollbar_thumb_color, Color::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn malformed_json_is_rejected_so_the_caller_keeps_defaults() {
        assert!(TuningJson::parse("{ not json").is_err());
        // 未知キーも拒否し、黙って無視せずタイポを表面化させる。
        assert!(TuningJson::parse(r#"{ "scroll": { "typoed_key": 1 } }"#).is_err());
    }

    #[test]
    fn json5_lite_comments_and_trailing_commas_are_accepted() {
        let src = r#"{
            // 慣性の摩擦（行コメント）
            "scroll": {
                "deceleration_rate": 0.99, /* ブロックコメント */
                "min_velocity": 0.03,      // 末尾カンマも許容 ↓
            },
        }"#;
        let t = TuningJson::parse(src).expect("JSON5-lite must parse");
        let s = t.scroll_tuning();
        assert_eq!(s.deceleration_rate, 0.99);
        assert_eq!(s.min_velocity, 0.03);
        // 触れていないキーはデフォルトを保つ。
        assert_eq!(s.slop_px, ScrollPhysicsTuning::default().slop_px);
    }

    #[test]
    fn a_slash_or_comma_inside_a_string_is_not_treated_as_a_comment() {
        // 防御的: 文字列値はプリプロセッサを無傷で通る必要がある（現状文字列値
        // のキーは無いが、ストリッパは安全でなければならない）。
        assert_eq!(to_plain_json(r#"{"a":"http://x , y"}"#), r#"{"a":"http://x , y"}"#);
    }
}
