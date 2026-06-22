//! # Hayabusa（隼）
//!
//! Hayate の Element Layer 上に構築された Signal ベースの SFC（Single-File Component）
//! フレームワークのランタイム。リアクティブランタイム（Signal グラフ・伝播・reconcile・
//! スケジューリング）を Rust で**単独所有**する点が、各言語の既存ランタイムを再利用する
//! Tsubame と対をなす（CONTEXT.md / Hayabusa ADR-0001）。
//!
//! ## このクレートの現在地（tracer bullet・ADR-0006）
//!
//! 最初の vertical slice として、カウンタ例を自作部品だけで通す：
//!
//! - [`reactive`]：自作 fine-grained リアクティブコア（Signal / Memo / Effect、
//!   glitch-free、flush 合体・ADR-0003）
//! - [`value`]：閉じた値モデル（number / string / bool / list / record・ADR-0003）
//! - [`expr`]：最小の純粋式評価器（binding は純粋式・ADR-0004）
//! - [`template`]：手組み Template IR（ADR-0004 / ADR-0006）
//! - [`sink`]：`ElementSink` mutation サーフェス。`hayate_core::ElementTree` に 1:1 で
//!   写る host-ABI 線（ADR-0002）。`RecordingSink` で fine-grained patch を観測できる。
//! - [`instantiate`]：Template IR を instantiate / bind し、`count` 変化時に
//!   **テキストノードだけが patch される**ことを成立させる。
//!
//! `.hybs` / コンパイラ / 他言語ゲスト / router は含めない（後続）。`ElementSink` を
//! 実際の `hayate_core::ElementTree` に転送する `HayateSink` も後続の薄い実装になる。
//!
//! ## カウンタ例
//!
//! ```
//! use hayabusa::prelude::*;
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! let rt = Runtime::new();
//! let count = rt.signal(Value::number(0));
//!
//! // <view><text>{count}</text><button on:click=increment>+1</button></view>
//! let template = TemplateNode::new(ElementKind::View)
//!     .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
//!     .child(TemplateNode::new(ElementKind::Button).text("+1").on_click(0));
//!
//! let scope = Scope::new().with("count", Binding::Signal(count.clone()));
//!
//! // increment ハンドラ（副作用の本体・script の代役）。signal を触るだけ。
//! // ハンドラは payload（click では未使用）を 1 つ受け取る。
//! let inc = count.clone();
//! let handlers: Vec<Handler> =
//!     vec![Box::new(move |_| inc.update(|v| Value::number(v.as_number().unwrap() + 1.0)))];
//!
//! let sink = Rc::new(RefCell::new(RecordingSink::new()));
//! let app = instantiate(&rt, &template, &scope, handlers, sink.clone());
//!
//! // 初期 instantiate 後の mutation を捨て、以降の patch だけを観測する。
//! sink.borrow_mut().clear_log();
//!
//! // ボタンの ElId を引いてクリックする。
//! let button = ElId(2);
//! assert!(app.click(button));
//!
//! // テキストノード（ElId(1)）の text だけが patch される。
//! assert_eq!(sink.borrow().text_mutations(), vec![(ElId(1), "1".to_string())]);
//! ```

pub mod component;
pub mod expr;
pub mod instantiate;
pub mod reactive;
pub mod sink;
pub mod template;
pub mod value;

/// よく使う型をまとめて取り込むための prelude。
pub mod prelude {
    pub use crate::component::{Component, ComponentSlot, ComponentView, Emit, Handler, SetupCx};
    pub use crate::expr::{BinOp, Binding, Expr, Scope};
    pub use crate::instantiate::{instantiate, Instance};
    pub use crate::reactive::{Memo, Runtime, ScopeId, Signal};
    pub use crate::sink::{ElId, ElementKind, ElementSink, Mutation, RecordingSink};
    pub use crate::template::{EachBlock, HandlerId, IfBlock, Template, TemplateNode};
    pub use crate::value::Value;
}
