//! 手組み Template IR（ADR-0004 / ADR-0006）。
//!
//! `.hybs` の `<template>` はコンパイルされて Template IR — 要素ツリー・静的 prop・
//! reactive binding・制御フロー（`:if` / `:each`）を記述するデータ構造 — になる。
//! ランタイムがこの IR を instantiate / bind し、element ツリー（sink）を駆動する。
//! tracer bullet ではコンパイラを持たず、この IR を手で組み立てる。
//!
//! - [`TemplateNode`]：1 要素（`view` / `text` / `button` …）。静的テキスト・reactive
//!   text 束縛・click ハンドラ・子を持つ。
//! - [`Template`]：要素ノードまたは制御フローブロック。要素の子に並ぶ。
//! - [`IfBlock`]：`:if`。条件式が truthy のときだけ body を mount する。
//! - [`EachBlock`]：`:each ... by <式>`。keyed-only のリスト描画（ADR-0004）。

use crate::expr::Expr;
use crate::sink::ElementKind;

/// 副作用ハンドラ（`on_click` 等）への参照。instantiate 時に渡すハンドラ列の添字。
pub type HandlerId = usize;

/// 要素の子に並べられるテンプレート断片：要素そのものか、制御フローブロック。
#[derive(Clone)]
pub enum Template {
    /// 1 つの要素ノード。
    Element(TemplateNode),
    /// `:if` 条件ブロック。
    If(IfBlock),
    /// `:each` リストブロック（keyed-only）。
    Each(EachBlock),
}

impl From<TemplateNode> for Template {
    fn from(node: TemplateNode) -> Self {
        Template::Element(node)
    }
}

impl From<IfBlock> for Template {
    fn from(block: IfBlock) -> Self {
        Template::If(block)
    }
}

impl From<EachBlock> for Template {
    fn from(block: EachBlock) -> Self {
        Template::Each(block)
    }
}

/// Template IR の 1 要素。Hayate の element-kind に直接対応する（HTML タグは使わない）。
#[derive(Clone)]
pub struct TemplateNode {
    pub kind: ElementKind,
    /// 束縛のない固定テキスト（text-like 要素のみ意味を持つ）。
    pub static_text: Option<String>,
    /// reactive な text 束縛。`Some` なら signal 変化で再評価される。
    pub text_binding: Option<Expr>,
    /// クリックで起動する副作用ハンドラ。
    pub on_click: Option<HandlerId>,
    /// 子（要素・制御フロー）。
    pub children: Vec<Template>,
}

impl TemplateNode {
    pub fn new(kind: ElementKind) -> Self {
        TemplateNode {
            kind,
            static_text: None,
            text_binding: None,
            on_click: None,
            children: Vec::new(),
        }
    }

    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.static_text = Some(content.into());
        self
    }

    pub fn bind_text(mut self, expr: Expr) -> Self {
        self.text_binding = Some(expr);
        self
    }

    pub fn on_click(mut self, handler: HandlerId) -> Self {
        self.on_click = Some(handler);
        self
    }

    /// 子（要素・`:if`・`:each` のいずれか）を末尾に追加する。
    pub fn child(mut self, node: impl Into<Template>) -> Self {
        self.children.push(node.into());
        self
    }
}

/// `:if` 条件ブロック。`cond` が truthy のときだけ `body` を mount し、falsy に
/// 変わると body の Scope を teardown して要素を除去する（ADR-0003 / 0004）。
#[derive(Clone)]
pub struct IfBlock {
    pub cond: Expr,
    /// body の根は単一要素（ネストした制御フローはその要素の子に置く）。
    pub body: Box<TemplateNode>,
}

impl IfBlock {
    pub fn new(cond: Expr, body: TemplateNode) -> Self {
        IfBlock {
            cond,
            body: Box::new(body),
        }
    }
}

/// `:each ... by <式>` リストブロック（ADR-0004 keyed-only）。
///
/// - `items`：`Value::List` に評価される式。各要素が 1 行の item 値。
/// - `item_var`：各行で item 値（行ごとの signal）に束縛される名前。
/// - `key`：`item_var` を束縛した上で評価する **キー式**（必須）。同一キーの値更新は
///   再生成せず in-place patch、並べ替えは move で各行の Scope 状態を保つ。
/// - `body`：1 行ぶんのテンプレート（`item_var` がスコープに入る）。
#[derive(Clone)]
pub struct EachBlock {
    pub items: Expr,
    pub item_var: String,
    pub key: Expr,
    /// 1 行の根は単一要素（ネストした制御フローはその要素の子に置く）。
    pub body: Box<TemplateNode>,
}

impl EachBlock {
    pub fn new(items: Expr, item_var: impl Into<String>, key: Expr, body: TemplateNode) -> Self {
        EachBlock {
            items,
            item_var: item_var.into(),
            key,
            body: Box::new(body),
        }
    }
}
