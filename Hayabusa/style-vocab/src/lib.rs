//! Hayabusa の `<style>` DSL が受け付ける enum 語彙（ADR-0011）。
//!
//! 正本は `Hayate/proto/spec/enums.json` ＋ `style_tags.json` で、`build.rs` が唯一の
//! JSON パース地点としてそれを読み、[`ENUM_KEYWORDS`] を生成する。
//!
//! - `hayabusa` の `build.rs`（build-dependency）はこれを読んで `style.rs` の実 enum
//!   （`Display` / `FlexDirection` / `Align` / `Justify`）を生成する。
//! - `hayabusa-codegen`（通常の dependency）はこれを読んで `.hybs` の `<style>` 属性を
//!   コンパイルする際のキーワード→variant名の変換に使う。
//!
//! どちらの消費者も、この語彙をそれぞれ手書きで再宣言しない。

/// 1 つの style tag（例：`flex-direction`）の enum 語彙。
pub struct EnumSpec {
    /// `<style>` DSL のプロパティ名（kebab-case。例：`"flex-direction"`）。
    pub prop: &'static str,
    /// 生成される Rust enum の型名（PascalCase。例：`"FlexDirection"`）。
    pub enum_name: &'static str,
    /// `(キーワード, variant名)` の対。キーワードは Hayate の spec 値（snake_case）を
    /// kebab-case へ機械変換したもの（Tsubame の generator と同じ規則）。
    pub variants: &'static [(&'static str, &'static str)],
}

include!(concat!(env!("OUT_DIR"), "/enum_keywords_generated.rs"));
