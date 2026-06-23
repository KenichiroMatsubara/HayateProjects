//! `.hybs` → 生成 Rust のコンパイラ（build 時 codegen・Hayabusa ADR-0008）。
//!
//! `.hybs` は 3 セクション `<template>` / `<style>` / `<script>` を持つ単一ファイル
//! コンポーネント（HAYA-04）。本クレートはそれをパースし、ビルド時に **生成 Rust** を出す
//! （build.rs から呼ばれる）。生成物は cargo が通常コンパイルするので、`<script>` の Rust は
//! 型検査され、`<template>` の束縛・`on:click` は `<script>` が定義した名前へ配線される
//! （proc-macro でも手組みでもなく、`.hybs` を一次成果物にする・ADR-0008）。
//!
//! ## このスライスの範囲（tracer bullet）
//!
//! - `<template>`：要素のネスト・静的テキスト・`{expr}` text 束縛・`on:click={handler}`。
//! - `<script>`：Rust をそのまま `build` 関数本体へ差し込む（`rt: &Runtime` が見える）。
//! - `<style>`：受理するが出力しない（static style codegen は P3 の sink `set_style` 待ち）。
//!
//! `:if` / `:each` / コンポーネント / mixed text・複数 `{expr}` は後続（本パーサは明示的に
//! エラーにするか単純化する）。式の評価器（`Expr::parse`）は生成コード側（実行時）で使い、
//! 本 codegen は束縛の自由変数抽出のために識別子を軽くスキャンするだけ。
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

    // handler 識別子を出現順（重複除去）に集める。
    let mut handlers: Vec<String> = Vec::new();
    collect_handlers(&root, &mut handlers);

    // 束縛式の自由変数（根の識別子）を集める。
    let mut binding_vars: Vec<String> = Vec::new();
    collect_binding_vars(&root, &mut binding_vars);

    let template_expr = emit_element(&root, &handlers);

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

/// パース済みのテンプレート要素。
struct Element {
    /// 生成コードに出す `ElementKind` のバリアント名（例 `"View"`）。
    kind: String,
    on_click: Option<String>,
    /// `on:input={h}` の handler 識別子（ADR-0007 の「読み・主」）。
    on_input: Option<String>,
    /// `value={expr}` の束縛式テキスト（ADR-0007 の「書き・従」）。
    value: Option<String>,
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

    // 属性：`on:click={ident}` / `on:input={ident}` / `value={expr}`。
    let mut on_click = None;
    let mut on_input = None;
    let mut value = None;
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
                return Ok(Element {
                    kind,
                    on_click,
                    on_input,
                    value,
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
                    _ => {
                        return Err(CompileError::new(format!(
                            "unsupported attribute `{name}` on <{tag}> \
                             (this slice supports on:click / on:input / value)"
                        )))
                    }
                }
            }
            None => return Err(CompileError::new(format!("<{tag}> is not closed"))),
        }
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
        content,
        children,
    })
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

fn collect_binding_vars(el: &Element, out: &mut Vec<String>) {
    // text 束縛と value 束縛の両方の自由変数を signal として配線する。
    for expr in [as_bind_expr(&el.content), el.value.as_deref()]
        .into_iter()
        .flatten()
    {
        for v in free_root_idents(expr) {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    for child in &el.children {
        collect_binding_vars(child, out);
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

/// 1 要素を `TemplateNode::new(...)....` ビルダ式へ出す（子は再帰）。
fn emit_element(el: &Element, handlers: &[String]) -> String {
    let mut s = format!("TemplateNode::new(ElementKind::{})", el.kind);
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
        s.push_str(&format!(".child({})", emit_element(child, handlers)));
    }
    s
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
        collect_binding_vars(&root, &mut v);
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
        collect_binding_vars(&root, &mut v);
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
    fn errors_on_unknown_tag() {
        let bad = "<template><blink>x</blink></template>";
        assert!(compile_hybs(bad, "bad").is_err());
    }

    #[test]
    fn errors_on_missing_template() {
        assert!(compile_hybs("<script>let x = 1;</script>", "bad").is_err());
    }
}
