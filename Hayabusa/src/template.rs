//! 手組み Template IR（ADR-0004 / ADR-0006 tracer bullet）。
//!
//! `.hybs` の `<template>` はコンパイルされて Template IR — 要素ツリー・静的 prop・
//! reactive binding を記述するデータ構造 — になる。ランタイムがこの IR を
//! instantiate / bind し、element ツリー（sink）を駆動する。tracer bullet では
//! コンパイラを持たず、この IR を手で組み立てる。
//!
//! - `text_binding`：純粋式の AST。signal が変わると再評価され、text-prop に書かれる。
//! - `static_text`：束縛のない固定テキスト。
//! - `on_click`：副作用の本体（script のハンドラ）への参照。binding（純粋）と
//!   ハンドラ（副作用）を分けるのが ADR-0004 の責務線。

use crate::expr::Expr;
use crate::sink::ElementKind;

/// 副作用ハンドラ（`on_click` 等）への参照。instantiate 時に渡すハンドラ列の添字。
pub type HandlerId = usize;

/// Template IR の 1 ノード。Hayate の element-kind に直接対応する（HTML タグは使わない）。
pub struct TemplateNode {
    pub kind: ElementKind,
    /// 束縛のない固定テキスト（text-like 要素のみ意味を持つ）。
    pub static_text: Option<String>,
    /// reactive な text 束縛。`Some` なら signal 変化で再評価される。
    pub text_binding: Option<Expr>,
    /// クリックで起動する副作用ハンドラ。
    pub on_click: Option<HandlerId>,
    pub children: Vec<TemplateNode>,
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

    pub fn child(mut self, node: TemplateNode) -> Self {
        self.children.push(node);
        self
    }
}
