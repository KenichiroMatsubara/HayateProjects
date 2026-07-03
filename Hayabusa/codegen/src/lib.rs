//! `.hybs` → 生成 Rust のコンパイラ（build 時 codegen・Hayabusa ADR-0008）。
//!
//! `.hybs` は 3 セクション `<template>` / `<style>` / `<script>` を持つ単一ファイル
//! コンポーネント（HAYA-04）。本クレートはそれをパースし、ビルド時に **生成 Rust** を出す
//! （build.rs から呼ばれる）。生成物は cargo が通常コンパイルするので、`<script>` の Rust は
//! 型検査され、`<template>` の束縛・`on:click` は `<script>` が定義した名前へ配線される
//! （proc-macro でも手組みでもなく、`.hybs` を一次成果物にする・ADR-0008）。
//!
//! ## このスライスの範囲
//!
//! - `<template>`：要素のネスト・静的テキスト・`{expr}` text 束縛・`on:click` / `on:input`・
//!   `value={expr}`（ADR-0007）・インライン `style="..."`（ADR-0010）・制御フロー
//!   `:if="<cond>"` / `:each="<item> in <items>" :key="<expr>"`（ADR-0004 keyed-only）。
//! - `<script>`：Rust をそのまま `build` 関数本体へ差し込む（`rt: &Runtime` が見える）。
//!
//! 子コンポーネント・mixed text・複数 `{expr}`・`<style>` ブロック＋セレクタ・per-row への
//! item payload 配線は後続（本パーサは未対応構文を明示エラーにする）。式の評価器
//! （`Expr::parse`）は生成コード側（実行時）で使い、本 codegen は束縛の自由変数抽出のために
//! 識別子を軽くスキャンするだけ（`:each` の item 変数は scope signal から除外する）。
//!
//! ## 名前の配線規約（ADR-0008 の「script の名前へ配線」）
//!
//! - `{expr}` の自由変数（根の識別子）は **`<script>` が定義した `Signal`** とみなし、
//!   `Scope::with("v", Binding::Signal(v.clone()))` を生成する。
//! - `on:click={h}` の `h` は **`<script>` が定義した `Handler`（`FnMut(Value)`）** とみなし、
//!   出現順（重複は除去）に `Vec<Handler>` へ詰めて添字を `.on_click(i)` に配る。
//! - したがって `<script>` は、handler が捕捉する signal を内部で `clone` し、束縛参照する
//!   signal を move し切らないように書く（scope 配線が `v.clone()` を読むため）。

use std::fmt;

/// `.hybs` コンパイルのエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
}

impl CompileError {
    fn new(message: impl Into<String>) -> Self {
        CompileError {
            message: message.into(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hybs compile error: {}", self.message)
    }
}

impl std::error::Error for CompileError {}

/// `.hybs` ソースを、コンポーネント `component` の生成 Rust（モジュール本体）へコンパイルする。
///
/// 戻り値は `pub fn build<S>(...) -> Instance<S> { ... }` を含むモジュール本体テキスト。
/// build.rs がこれを `pub mod <component> { ... }` で包む。
pub fn compile_hybs(source: &str, component: &str) -> Result<String, CompileError> {
    let sections = split_sections(source)?;
    let template_src = sections
        .template
        .ok_or_else(|| CompileError::new("missing <template> section"))?;

    let root = parse_template(&template_src)?;

    // ルートは単一の通常要素でなければならない（instantiate は `&TemplateNode` を取る）。
    if root.if_cond.is_some() || root.each.is_some() {
        return Err(CompileError::new(
            "control flow (:if / :each) is not allowed on the template root element",
        ));
    }

    // handler 識別子を出現順（重複除去）に集める。
    let mut handlers: Vec<String> = Vec::new();
    collect_handlers(&root, &mut handlers);

    // 束縛式の自由変数（根の識別子）を集める。`:each` の item 変数は runtime が束縛するので
    // signal scope からは除外する（bound セットで追跡）。
    let mut binding_vars: Vec<String> = Vec::new();
    collect_binding_vars(&root, &[], &mut binding_vars);

    // ルートは通常要素なので body emit をそのまま template に使う。
    let template_expr = emit_node(&root, &handlers);

    let script = sections.script.unwrap_or_default();

    Ok(emit_module(
        component,
        &script,
        &template_expr,
        &binding_vars,
        &handlers,
    ))
}

// ───────────────────────── セクション分割 ─────────────────────────

struct Sections {
    template: Option<String>,
    #[allow(dead_code)]
    style: Option<String>,
    script: Option<String>,
}

/// トップレベルの `<template>` / `<style>` / `<script>` ブロックを取り出す。各ブロックは
/// ネストしない（`<template>` 内の `<view>` 等は本検索の対象タグ名と一致しない）。
fn split_sections(source: &str) -> Result<Sections, CompileError> {
    Ok(Sections {
        template: extract_block(source, "template")?,
        style: extract_block(source, "style")?,
        script: extract_block(source, "script")?,
    })
}

/// `<tag> ... </tag>` の中身を取り出す（最初の 1 つ）。無ければ `None`。
fn extract_block(source: &str, tag: &str) -> Result<Option<String>, CompileError> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = match source.find(&open) {
        Some(i) => i + open.len(),
        None => return Ok(None),
    };
    let end = source[start..]
        .find(&close)
        .ok_or_else(|| CompileError::new(format!("<{tag}> is not closed by </{tag}>")))?;
    Ok(Some(source[start..start + end].to_string()))
}

// ───────────────────────── テンプレート markup パーサ ─────────────────────────

/// 要素の中身：無し / 静的テキスト / `{expr}` 束縛。
enum Content {
    None,
    Static(String),
    Bind(String),
}

/// `:each="<item> in <items>" :key="<expr>"` の指定（ADR-0004 keyed-only）。
struct EachSpec {
    item_var: String,
    items: String,
    key: String,
}

/// パース済みのテンプレート要素。
struct Element {
    /// 生成コードに出す `ElementKind` のバリアント名（例 `"View"`）。
    kind: String,
    on_click: Option<String>,
    /// `on:input={h}` の handler 識別子（ADR-0007 の「読み・主」）。
    on_input: Option<String>,
    /// `value={expr}` の束縛式テキスト（ADR-0007 の「書き・従」）。
    value: Option<String>,
    /// `style="..."` を解決した `StyleProp::...` コンストラクタ式の列（ADR-0010）。
    style: Vec<String>,
    /// `:if="<cond>"`。この要素を `IfBlock` の body として包む（ADR-0004）。
    if_cond: Option<String>,
    /// `:each="<item> in <items>" :key="<expr>"`。この要素を `EachBlock` の body として包む。
    each: Option<EachSpec>,
    content: Content,
    children: Vec<Element>,
}

/// タグ名（`view` / `text-input` …）→ `ElementKind` バリアント名。
fn tag_to_kind(tag: &str) -> Result<&'static str, CompileError> {
    match tag {
        "view" => Ok("View"),
        "text" => Ok("Text"),
        "image" => Ok("Image"),
        "button" => Ok("Button"),
        "text-input" => Ok("TextInput"),
        "scroll-view" => Ok("ScrollView"),
        other => Err(CompileError::new(format!("unknown element tag <{other}>"))),
    }
}

struct Cursor<'a> {
    chars: Vec<char>,
    pos: usize,
    _src: &'a str,
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Cursor {
            chars: s.chars().collect(),
            pos: 0,
            _src: s,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        let sc: Vec<char> = s.chars().collect();
        self.chars[self.pos..].starts_with(&sc)
    }
}

/// `<template>` の中身をパースし、単一のルート要素を返す。
fn parse_template(src: &str) -> Result<Element, CompileError> {
    let mut c = Cursor::new(src);
    c.skip_ws();
    let el = parse_element(&mut c)?;
    c.skip_ws();
    if c.peek().is_some() {
        return Err(CompileError::new(
            "<template> must contain exactly one root element",
        ));
    }
    Ok(el)
}

fn parse_element(c: &mut Cursor) -> Result<Element, CompileError> {
    c.skip_ws();
    if c.bump() != Some('<') {
        return Err(CompileError::new("expected `<` to start an element"));
    }
    let tag = read_name(c)?;
    let kind = tag_to_kind(&tag)?.to_string();

    // 属性：`on:click` / `on:input` / `value` / `style` / `:if` / `:each` / `:key`。
    let mut on_click = None;
    let mut on_input = None;
    let mut value = None;
    let mut style = Vec::new();
    let mut if_cond = None;
    let mut each_clause = None;
    let mut key = None;
    loop {
        c.skip_ws();
        match c.peek() {
            Some('>') => {
                c.bump();
                break;
            }
            Some('/') if c.peek2() == Some('>') => {
                // 自己終端 `<tag/>`：子も内容も無い。
                c.bump();
                c.bump();
                let each = build_each(&tag, each_clause, key)?;
                return Ok(Element {
                    kind,
                    on_click,
                    on_input,
                    value,
                    style,
                    if_cond,
                    each,
                    content: Content::None,
                    children: Vec::new(),
                });
            }
            Some(_) => {
                let (name, val) = read_attribute(c)?;
                match name.as_str() {
                    "on:click" => on_click = Some(val),
                    "on:input" => on_input = Some(val),
                    "value" => value = Some(val),
                    "style" => style = parse_inline_style(&val)?,
                    ":if" => if_cond = Some(val),
                    ":each" => each_clause = Some(val),
                    ":key" => key = Some(val),
                    _ => {
                        return Err(CompileError::new(format!(
                            "unsupported attribute `{name}` on <{tag}> (this slice supports \
                             on:click / on:input / value / style / :if / :each / :key)"
                        )))
                    }
                }
            }
            None => return Err(CompileError::new(format!("<{tag}> is not closed"))),
        }
    }
    let each = build_each(&tag, each_clause, key)?;
    if if_cond.is_some() && each.is_some() {
        return Err(CompileError::new(format!(
            "<{tag}> has both :if and :each (split into nested elements)"
        )));
    }

    // 内容：子要素 or テキ/束縛。
    let (content, children) = parse_content(c, &tag)?;

    // 終了タグ `</tag>`。
    let close = format!("</{tag}>");
    if !c.starts_with(&close) {
        return Err(CompileError::new(format!("expected closing </{tag}>")));
    }
    for _ in 0..close.chars().count() {
        c.bump();
    }

    Ok(Element {
        kind,
        on_click,
        on_input,
        value,
        style,
        if_cond,
        each,
        content,
        children,
    })
}

/// `:each="<item> in <items>"` ＋ `:key="<expr>"` から [`EachSpec`] を組む。`:each` 無しで
/// `:key` だけは誤り。`:each` には `:key` 必須（keyed-only・ADR-0004）。
fn build_each(
    tag: &str,
    each_clause: Option<String>,
    key: Option<String>,
) -> Result<Option<EachSpec>, CompileError> {
    match (each_clause, key) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(CompileError::new(format!(
            "<{tag}> has :key without :each"
        ))),
        (Some(_), None) => Err(CompileError::new(format!(
            "<{tag}> :each requires :key (keyed-only)"
        ))),
        (Some(clause), Some(key)) => {
            let (item_var, items) = clause.split_once(" in ").ok_or_else(|| {
                CompileError::new(format!(
                    "<{tag}> :each must be `<item> in <items>`, got `{clause}`"
                ))
            })?;
            let item_var = item_var.trim().to_string();
            if item_var.is_empty() || !item_var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(CompileError::new(format!(
                    "<{tag}> :each item name `{item_var}` is not a valid identifier"
                )));
            }
            Ok(Some(EachSpec {
                item_var,
                items: items.trim().to_string(),
                key: key.trim().to_string(),
            }))
        }
    }
}

/// 開きタグ直後から終了タグ直前までの内容を読む。子要素の列か、単一のテキスト/束縛。
fn parse_content(c: &mut Cursor, tag: &str) -> Result<(Content, Vec<Element>), CompileError> {
    c.skip_ws();
    // 子要素列：次が `<` で、その次が `/` でない。
    if c.peek() == Some('<') && c.peek2() != Some('/') {
        let mut children = Vec::new();
        loop {
            c.skip_ws();
            if c.peek() == Some('<') && c.peek2() == Some('/') {
                break;
            }
            if c.peek() == Some('<') {
                children.push(parse_element(c)?);
            } else if c.peek().is_none() {
                return Err(CompileError::new(format!("<{tag}> is not closed")));
            } else {
                return Err(CompileError::new(format!(
                    "<{tag}> mixes child elements and text (not supported in this slice)"
                )));
            }
        }
        return Ok((Content::None, children));
    }

    // 空（即終了タグ）。
    if c.peek() == Some('<') && c.peek2() == Some('/') {
        return Ok((Content::None, Vec::new()));
    }

    // テキスト or 束縛：`<` まで読む。
    let mut text = String::new();
    while let Some(ch) = c.peek() {
        if ch == '<' {
            break;
        }
        text.push(ch);
        c.bump();
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok((Content::None, Vec::new()));
    }
    if let Some(expr) = as_single_binding(trimmed)? {
        Ok((Content::Bind(expr), Vec::new()))
    } else {
        if trimmed.contains('{') || trimmed.contains('}') {
            return Err(CompileError::new(format!(
                "text content `{trimmed}` mixes literal and {{expr}} (not supported in this slice)"
            )));
        }
        Ok((Content::Static(trimmed.to_string()), Vec::new()))
    }
}

/// `{expr}` 単体なら中身（trim 済み）を返す。リテラルなら `None`。
fn as_single_binding(s: &str) -> Result<Option<String>, CompileError> {
    if s.starts_with('{') && s.ends_with('}') && s.len() >= 2 {
        let inner = s[1..s.len() - 1].trim();
        if inner.is_empty() {
            return Err(CompileError::new("empty `{}` binding"));
        }
        if inner.contains('{') || inner.contains('}') {
            return Err(CompileError::new(format!("malformed binding `{s}`")));
        }
        return Ok(Some(inner.to_string()));
    }
    Ok(None)
}

/// 識別子（要素タグ・属性名・`on:click` の `:` を許す）。
fn read_name(c: &mut Cursor) -> Result<String, CompileError> {
    let mut s = String::new();
    while let Some(ch) = c.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == ':' {
            s.push(ch);
            c.bump();
        } else {
            break;
        }
    }
    if s.is_empty() {
        return Err(CompileError::new("expected a name"));
    }
    Ok(s)
}

/// `name={ident}` または `name="literal"` 属性を読む。値の中身（`{}`/`""` の内側）を返す。
fn read_attribute(c: &mut Cursor) -> Result<(String, String), CompileError> {
    let name = read_name(c)?;
    c.skip_ws();
    if c.bump() != Some('=') {
        return Err(CompileError::new(format!("expected `=` after attribute `{name}`")));
    }
    c.skip_ws();
    let value = match c.peek() {
        Some('{') => {
            c.bump();
            let mut v = String::new();
            while let Some(ch) = c.peek() {
                if ch == '}' {
                    break;
                }
                v.push(ch);
                c.bump();
            }
            if c.bump() != Some('}') {
                return Err(CompileError::new(format!("attribute `{name}` `{{...}}` is not closed")));
            }
            v.trim().to_string()
        }
        Some('"') => {
            c.bump();
            let mut v = String::new();
            while let Some(ch) = c.peek() {
                if ch == '"' {
                    break;
                }
                v.push(ch);
                c.bump();
            }
            if c.bump() != Some('"') {
                return Err(CompileError::new(format!("attribute `{name}` string is not closed")));
            }
            v
        }
        _ => return Err(CompileError::new(format!("attribute `{name}` needs a `{{expr}}` or \"string\" value"))),
    };
    Ok((name, value))
}

// ───────────────────────── inline style（`style="..."`・ADR-0010） ─────────────────────────

/// `style="k: v; k: v"` を `StyleProp::...` コンストラクタ式の列へコンパイルする。
/// 生成コードは prelude の `StyleProp` / `Length` / `Rgba` / `Display` / `FlexDirection` /
/// `Align` / `Justify` を参照する（cargo が型検査）。
fn parse_inline_style(s: &str) -> Result<Vec<String>, CompileError> {
    let mut out = Vec::new();
    for decl in s.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let (key, val) = decl
            .split_once(':')
            .ok_or_else(|| CompileError::new(format!("style declaration `{decl}` is missing `:`")))?;
        let key = key.trim();
        let val = val.trim();
        let emit = match key {
            "width" => format!("StyleProp::Width({})", style_length(val)?),
            "height" => format!("StyleProp::Height({})", style_length(val)?),
            "padding" => format!("StyleProp::Padding({})", style_length(val)?),
            "margin" => format!("StyleProp::Margin({})", style_length(val)?),
            "gap" => format!("StyleProp::Gap({})", style_length(val)?),
            "font-size" => format!("StyleProp::FontSize({})", style_f32_px(val)?),
            "background-color" | "background" => {
                format!("StyleProp::BackgroundColor({})", style_color(val)?)
            }
            "color" => format!("StyleProp::TextColor({})", style_color(val)?),
            "display" => format!("StyleProp::Display({})", enum_variant(key, val)?),
            "flex-direction" => format!("StyleProp::FlexDirection({})", enum_variant(key, val)?),
            "align-items" => format!("StyleProp::AlignItems({})", enum_variant(key, val)?),
            "justify-content" => format!("StyleProp::JustifyContent({})", enum_variant(key, val)?),
            other => {
                return Err(CompileError::new(format!(
                    "unsupported style property `{other}`"
                )))
            }
        };
        out.push(emit);
    }
    Ok(out)
}

/// `px` / `%` / `auto` → `Length::...`。
fn style_length(val: &str) -> Result<String, CompileError> {
    if val == "auto" {
        return Ok("Length::Auto".to_string());
    }
    if let Some(p) = val.strip_suffix('%') {
        return Ok(format!("Length::Percent({})", parse_f32_lit(p.trim())?));
    }
    if let Some(p) = val.strip_suffix("px") {
        return Ok(format!("Length::Px({})", parse_f32_lit(p.trim())?));
    }
    Err(CompileError::new(format!(
        "length `{val}` needs a unit (`px` / `%`) or `auto`"
    )))
}

/// `font-size` 値（`px` 任意）→ f32 リテラル。
fn style_f32_px(val: &str) -> Result<String, CompileError> {
    let num = val.strip_suffix("px").unwrap_or(val).trim();
    parse_f32_lit(num)
}

/// 数値テキスト → `<n>f32` リテラル。
fn parse_f32_lit(s: &str) -> Result<String, CompileError> {
    s.parse::<f64>()
        .map(|v| format!("{v}f32"))
        .map_err(|_| CompileError::new(format!("invalid number `{s}`")))
}

/// 色（`#rgb` / `#rrggbb` / `#rrggbbaa` / 名前付き）→ `Rgba::new(r, g, b, a)`。
fn style_color(val: &str) -> Result<String, CompileError> {
    if let Some(hex) = val.strip_prefix('#') {
        return hex_color(hex);
    }
    let (r, g, b, a) = match val {
        "white" => (255, 255, 255, 255),
        "black" => (0, 0, 0, 255),
        "red" => (255, 0, 0, 255),
        "green" => (0, 128, 0, 255),
        "blue" => (0, 0, 255, 255),
        "gray" | "grey" => (128, 128, 128, 255),
        "lightgray" | "lightgrey" => (211, 211, 211, 255),
        "transparent" => (0, 0, 0, 0),
        other => {
            return Err(CompileError::new(format!(
                "unsupported color `{other}` (use #hex or a known name)"
            )))
        }
    };
    Ok(rgba_lit(r, g, b, a))
}

/// `#rgb` / `#rrggbb` / `#rrggbbaa` をパースして `Rgba::new(...)`。
fn hex_color(hex: &str) -> Result<String, CompileError> {
    let bytes: Vec<u8> = match hex.len() {
        3 => hex
            .chars()
            .map(|c| u8::from_str_radix(&format!("{c}{c}"), 16))
            .collect::<Result<_, _>>()
            .map_err(|_| CompileError::new(format!("invalid hex color `#{hex}`")))?,
        6 | 8 => (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<_, _>>()
            .map_err(|_| CompileError::new(format!("invalid hex color `#{hex}`")))?,
        _ => {
            return Err(CompileError::new(format!(
                "hex color `#{hex}` must be 3, 6, or 8 digits"
            )))
        }
    };
    let a = if bytes.len() == 4 { bytes[3] } else { 255 };
    Ok(rgba_lit(bytes[0], bytes[1], bytes[2], a))
}

/// 0..255 のチャンネルを 0..1 の `Rgba::new(...)` リテラルへ。
fn rgba_lit(r: u8, g: u8, b: u8, a: u8) -> String {
    let f = |x: u8| format!("{}f32", x as f64 / 255.0);
    format!("Rgba::new({}, {}, {}, {})", f(r), f(g), f(b), f(a))
}

/// キーワード → 完全修飾 variant 式（例: `"align-items"`, `"flex-start"` → `"Align::FlexStart"`）。
/// 語彙は `hayabusa-style-vocab`（Hayate の proto/spec が正本・ADR-0011）から読む——このクレートで
/// キーワードや variant 名を再宣言しない。
fn enum_variant(prop: &str, val: &str) -> Result<String, CompileError> {
    let spec = hayabusa_style_vocab::ENUM_KEYWORDS
        .iter()
        .find(|s| s.prop == prop)
        .unwrap_or_else(|| panic!("`{prop}` is not a style_enum prop (parse_inline_style's match should have rejected it first)"));
    match spec.variants.iter().find(|(keyword, _)| *keyword == val) {
        Some((_, variant)) => Ok(format!("{}::{variant}", spec.enum_name)),
        None => Err(CompileError::new(format!(
            "unsupported value `{val}` for `{prop}`"
        ))),
    }
}

// ───────────────────────── 収集（handler / 自由変数） ─────────────────────────

fn collect_handlers(el: &Element, out: &mut Vec<String>) {
    // 同一要素では on:click → on:input の順（文書順・重複除去）。
    for h in [&el.on_click, &el.on_input].into_iter().flatten() {
        if !out.contains(h) {
            out.push(h.clone());
        }
    }
    for child in &el.children {
        collect_handlers(child, out);
    }
}

fn collect_binding_vars(el: &Element, bound: &[String], out: &mut Vec<String>) {
    // 制御フローの式は外側スコープで評価される（`:if` cond・`:each` items）。
    if let Some(cond) = &el.if_cond {
        add_free_vars(cond, bound, out);
    }
    // `:each` は item 変数を内側へ束縛する：items は外側 signal、key / 本文 / 子は item 込み。
    let mut inner: Vec<String> = bound.to_vec();
    if let Some(each) = &el.each {
        add_free_vars(&each.items, bound, out);
        inner.push(each.item_var.clone());
        add_free_vars(&each.key, &inner, out);
    }
    // text 束縛と value 束縛の自由変数（item 変数は除外）を signal として配線する。
    for expr in [as_bind_expr(&el.content), el.value.as_deref()]
        .into_iter()
        .flatten()
    {
        add_free_vars(expr, &inner, out);
    }
    for child in &el.children {
        collect_binding_vars(child, &inner, out);
    }
}

/// `expr` の自由変数のうち `bound` に無いものを `out` へ（重複除去）。
fn add_free_vars(expr: &str, bound: &[String], out: &mut Vec<String>) {
    for v in free_root_idents(expr) {
        if !bound.contains(&v) && !out.contains(&v) {
            out.push(v);
        }
    }
}

/// `Content::Bind` の式テキスト（それ以外は `None`）。
fn as_bind_expr(content: &Content) -> Option<&str> {
    match content {
        Content::Bind(e) => Some(e),
        _ => None,
    }
}

/// 式テキストから「根の識別子」を抽出する（member の field 名 `.label` は除く）。
/// `count + 1` → [count]、`item.label` → [item]、`a + b` → [a, b]。
fn free_root_idents(expr: &str) -> Vec<String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    // 直前の非空白文字が `.` なら member field なので根ではない。
    let mut prev_nonspace: Option<char> = None;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let is_member = prev_nonspace == Some('.');
            let is_keyword = word == "true" || word == "false";
            if !is_member && !is_keyword && !out.contains(&word) {
                out.push(word);
            }
            prev_nonspace = chars.get(i - 1).copied();
            continue;
        }
        if !ch.is_whitespace() {
            prev_nonspace = Some(ch);
        }
        i += 1;
    }
    out
}

// ───────────────────────── 出力（emit） ─────────────────────────

/// 1 要素を `TemplateNode::new(...)....` ビルダ式（body）へ出す。制御フロー（`:if`/`:each`）は
/// 含めない（それは [`emit_child`] が `IfBlock`/`EachBlock` で包む）。子は [`emit_child`] 経由。
fn emit_node(el: &Element, handlers: &[String]) -> String {
    let mut s = format!("TemplateNode::new(ElementKind::{})", el.kind);
    if !el.style.is_empty() {
        s.push_str(&format!(".style(vec![{}])", el.style.join(", ")));
    }
    match &el.content {
        Content::Static(t) => s.push_str(&format!(".text({})", rust_str(t))),
        Content::Bind(expr) => {
            s.push_str(&format!(".bind_text(Expr::parse({}).unwrap())", rust_str(expr)))
        }
        Content::None => {}
    }
    if let Some(expr) = &el.value {
        s.push_str(&format!(".bind_value(Expr::parse({}).unwrap())", rust_str(expr)));
    }
    if let Some(h) = &el.on_click {
        let idx = handlers.iter().position(|x| x == h).unwrap();
        s.push_str(&format!(".on_click({idx}usize)"));
    }
    if let Some(h) = &el.on_input {
        let idx = handlers.iter().position(|x| x == h).unwrap();
        s.push_str(&format!(".on_input({idx}usize)"));
    }
    for child in &el.children {
        s.push_str(&format!(".child({})", emit_child(child, handlers)));
    }
    s
}

/// 子として `.child(..)` に渡す式を出す。`:if`/`:each` があれば body を `IfBlock`/`EachBlock` で
/// 包む（ADR-0004）。両方は parse 段で排他済み。
fn emit_child(el: &Element, handlers: &[String]) -> String {
    let body = emit_node(el, handlers);
    if let Some(cond) = &el.if_cond {
        return format!("IfBlock::new(Expr::parse({}).unwrap(), {body})", rust_str(cond));
    }
    if let Some(each) = &el.each {
        return format!(
            "EachBlock::new(Expr::parse({}).unwrap(), {}, Expr::parse({}).unwrap(), {body})",
            rust_str(&each.items),
            rust_str(&each.item_var),
            rust_str(&each.key),
        );
    }
    body
}

/// Rust の文字列リテラルへ安全にエスケープする。
fn rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// コンポーネントのモジュール本体（`build` 関数）を生成する。
fn emit_module(
    component: &str,
    script: &str,
    template_expr: &str,
    binding_vars: &[String],
    handlers: &[String],
) -> String {
    let mut scope_wiring = String::from("    let scope = Scope::new()");
    if binding_vars.is_empty() {
        scope_wiring.push(';');
    } else {
        for v in binding_vars {
            scope_wiring.push_str(&format!(
                "\n        .with({}, Binding::Signal({}.clone()))",
                rust_str(v),
                v
            ));
        }
        scope_wiring.push(';');
    }

    let handlers_wiring = if handlers.is_empty() {
        "    let handlers: Vec<Handler> = Vec::new();".to_string()
    } else {
        let items: Vec<String> = handlers.iter().map(|h| format!("Box::new({h})")).collect();
        format!(
            "    let handlers: Vec<Handler> = vec![{}];",
            items.join(", ")
        )
    };

    format!(
        r#"// 生成コード（`.hybs` → build 時 codegen・ADR-0008）。手で編集しない。
// source component: `{component}.hybs`

use crate::prelude::*;

/// `{component}.hybs` をコンパイルした構築関数。`rt` 上で `<script>` を実行して signal /
/// handler を定義し、`<template>` 由来の Template IR を instantiate する。
#[allow(clippy::all)]
pub fn build<S: crate::sink::ElementSink + 'static>(
    rt: &Runtime,
    sink: std::rc::Rc<std::cell::RefCell<S>>,
) -> Instance<S> {{
    // ───── <script>（verbatim・cargo が型検査する） ─────
{script_block}
    // ───── </script> ─────

    let template = {template_expr};
{scope_wiring}
{handlers_wiring}
    instantiate(rt, &template, &scope, handlers, sink)
}}
"#,
        component = component,
        script_block = indent_script(script),
        template_expr = template_expr,
        scope_wiring = scope_wiring,
        handlers_wiring = handlers_wiring,
    )
}

/// `<script>` 本文を build 関数本体のインデントに合わせて差し込む（中身は変えない）。
fn indent_script(script: &str) -> String {
    script
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("    {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTER: &str = r#"
<template>
  <view>
    <text>{count}</text>
    <button on:click={increment}>+1</button>
  </view>
</template>

<script>
let count = rt.signal(Value::number(0));
let increment = {
    let count = count.clone();
    move |_: Value| count.update(|v| Value::number(v.as_number().unwrap() + 1.0))
};
</script>
"#;

    #[test]
    fn splits_sections() {
        let s = split_sections(COUNTER).unwrap();
        assert!(s.template.is_some());
        assert!(s.script.is_some());
        assert!(s.style.is_none());
        assert!(s.script.unwrap().contains("rt.signal"));
    }

    #[test]
    fn parses_counter_template() {
        let root = parse_template(&extract_block(COUNTER, "template").unwrap().unwrap()).unwrap();
        assert_eq!(root.kind, "View");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].kind, "Text");
        assert!(matches!(root.children[0].content, Content::Bind(ref e) if e == "count"));
        assert_eq!(root.children[1].kind, "Button");
        assert_eq!(root.children[1].on_click.as_deref(), Some("increment"));
        assert!(matches!(root.children[1].content, Content::Static(ref t) if t == "+1"));
    }

    #[test]
    fn collects_handlers_and_binding_vars() {
        let root = parse_template(&extract_block(COUNTER, "template").unwrap().unwrap()).unwrap();
        let mut h = Vec::new();
        collect_handlers(&root, &mut h);
        assert_eq!(h, vec!["increment".to_string()]);
        let mut v = Vec::new();
        collect_binding_vars(&root, &[], &mut v);
        assert_eq!(v, vec!["count".to_string()]);
    }

    #[test]
    fn free_root_idents_skips_members_and_keywords() {
        assert_eq!(free_root_idents("count + 1"), vec!["count"]);
        assert_eq!(free_root_idents("item.label"), vec!["item"]);
        assert_eq!(free_root_idents("a && !b || true"), vec!["a", "b"]);
    }

    #[test]
    fn emits_compilable_looking_module() {
        let code = compile_hybs(COUNTER, "counter").unwrap();
        assert!(code.contains("pub fn build<S:"));
        assert!(code.contains("TemplateNode::new(ElementKind::View)"));
        assert!(code.contains(".bind_text(Expr::parse(\"count\").unwrap())"));
        assert!(code.contains(".on_click(0usize)"));
        assert!(code.contains(".text(\"+1\")"));
        assert!(code.contains(".with(\"count\", Binding::Signal(count.clone()))"));
        assert!(code.contains("vec![Box::new(increment)]"));
        assert!(code.contains("rt.signal(Value::number(0))"));
    }

    const FIELD: &str = r#"
<template>
  <view>
    <text-input value={draft} on:input={edit}/>
    <button on:click={add}>add</button>
  </view>
</template>
<script>
let draft = rt.signal(Value::string(""));
</script>
"#;

    #[test]
    fn parses_input_value_and_handlers() {
        let root = parse_template(&extract_block(FIELD, "template").unwrap().unwrap()).unwrap();
        let field = &root.children[0];
        assert_eq!(field.kind, "TextInput");
        assert_eq!(field.value.as_deref(), Some("draft"));
        assert_eq!(field.on_input.as_deref(), Some("edit"));

        // handler 列は文書順（input の `edit`、次に click の `add`）。
        let mut h = Vec::new();
        collect_handlers(&root, &mut h);
        assert_eq!(h, vec!["edit".to_string(), "add".to_string()]);

        // value 束縛の自由変数も signal として配線される。
        let mut v = Vec::new();
        collect_binding_vars(&root, &[], &mut v);
        assert_eq!(v, vec!["draft".to_string()]);
    }

    #[test]
    fn emits_input_value_wiring() {
        let code = compile_hybs(FIELD, "field").unwrap();
        assert!(code.contains(".bind_value(Expr::parse(\"draft\").unwrap())"));
        assert!(code.contains(".on_input(0usize)")); // edit = index 0
        assert!(code.contains(".on_click(1usize)")); // add  = index 1
        assert!(code.contains(".with(\"draft\", Binding::Signal(draft.clone()))"));
    }

    #[test]
    fn parses_inline_style_lengths_layout_and_colors() {
        let props = parse_inline_style(
            "width: 120px; padding: 8px; gap: 50%; height: auto; \
             display: flex; flex-direction: column; align-items: center; \
             justify-content: space-between; font-size: 16px; \
             background-color: #ffffff; color: #000",
        )
        .unwrap();
        assert!(props.contains(&"StyleProp::Width(Length::Px(120f32))".to_string()));
        assert!(props.contains(&"StyleProp::Padding(Length::Px(8f32))".to_string()));
        assert!(props.contains(&"StyleProp::Gap(Length::Percent(50f32))".to_string()));
        assert!(props.contains(&"StyleProp::Height(Length::Auto)".to_string()));
        assert!(props.contains(&"StyleProp::Display(Display::Flex)".to_string()));
        assert!(props.contains(&"StyleProp::FlexDirection(FlexDirection::Column)".to_string()));
        assert!(props.contains(&"StyleProp::AlignItems(Align::Center)".to_string()));
        assert!(props.contains(&"StyleProp::JustifyContent(Justify::SpaceBetween)".to_string()));
        assert!(props.contains(&"StyleProp::FontSize(16f32)".to_string()));
        assert!(props.contains(&"StyleProp::BackgroundColor(Rgba::new(1f32, 1f32, 1f32, 1f32))".to_string()));
        assert!(props.contains(&"StyleProp::TextColor(Rgba::new(0f32, 0f32, 0f32, 1f32))".to_string()));
    }

    #[test]
    fn emits_style_call_on_element() {
        let src = r#"<template><view style="width: 10px; background-color: #ff0000"><text>{x}</text></view></template>
<script>let x = rt.signal(Value::number(0));</script>"#;
        let code = compile_hybs(src, "styled").unwrap();
        assert!(code.contains(".style(vec!["));
        assert!(code.contains("StyleProp::Width(Length::Px(10f32))"));
        assert!(code.contains("StyleProp::BackgroundColor(Rgba::new(1f32, 0f32, 0f32, 1f32))"));
    }

    #[test]
    fn errors_on_unknown_style_property() {
        assert!(parse_inline_style("wibble: 3px").is_err());
        assert!(parse_inline_style("width: 3").is_err()); // 単位なし
        assert!(parse_inline_style("color: chartreuse").is_err()); // 未知の名前
    }

    const LIST: &str = r#"
<template>
  <view>
    <text :if={loading}>loading...</text>
    <view :each={todo in todos} :key={todo.id}>
      <text>{todo.label}</text>
    </view>
  </view>
</template>
<script>
let loading = rt.signal(Value::Bool(false));
let todos = rt.signal(Value::list([]));
</script>
"#;

    #[test]
    fn parses_if_and_each_directives() {
        let root = parse_template(&extract_block(LIST, "template").unwrap().unwrap()).unwrap();
        let if_el = &root.children[0];
        assert_eq!(if_el.if_cond.as_deref(), Some("loading"));
        let each_el = &root.children[1];
        let each = each_el.each.as_ref().expect("each");
        assert_eq!(each.item_var, "todo");
        assert_eq!(each.items, "todos");
        assert_eq!(each.key, "todo.id");
    }

    #[test]
    fn each_item_var_is_not_wired_as_a_signal() {
        let root = parse_template(&extract_block(LIST, "template").unwrap().unwrap()).unwrap();
        let mut v = Vec::new();
        collect_binding_vars(&root, &[], &mut v);
        // `loading` と `todos` は signal、`todo`（each の item 変数）は scope に入れない。
        assert!(v.contains(&"loading".to_string()));
        assert!(v.contains(&"todos".to_string()));
        assert!(!v.contains(&"todo".to_string()));
    }

    #[test]
    fn emits_if_and_each_blocks() {
        let code = compile_hybs(LIST, "list").unwrap();
        assert!(code.contains("IfBlock::new(Expr::parse(\"loading\").unwrap(),"));
        assert!(code.contains(
            "EachBlock::new(Expr::parse(\"todos\").unwrap(), \"todo\", Expr::parse(\"todo.id\").unwrap(),"
        ));
        // item 変数は scope wiring に出ない。
        assert!(!code.contains(".with(\"todo\","));
        assert!(code.contains(".with(\"todos\", Binding::Signal(todos.clone()))"));
    }

    #[test]
    fn errors_on_each_without_key() {
        let bad = "<template><view><text :each={x in xs}>{x}</text></view></template>";
        assert!(compile_hybs(bad, "bad").is_err());
    }

    #[test]
    fn errors_on_control_flow_on_root() {
        let bad = "<template><view :if={show}><text>x</text></view></template>";
        assert!(compile_hybs(bad, "bad").is_err());
    }

    #[test]
    fn errors_on_unknown_tag() {
        let bad = "<template><blink>x</blink></template>";
        assert!(compile_hybs(bad, "bad").is_err());
    }

    #[test]
    fn errors_on_missing_template() {
        assert!(compile_hybs("<script>let x = 1;</script>", "bad").is_err());
    }
}
