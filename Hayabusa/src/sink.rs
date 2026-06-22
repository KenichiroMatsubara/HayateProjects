//! ElementSink — ランタイムが叩く mutation サーフェス（ADR-0002 の host-ABI 線）。
//!
//! ランタイムは reconcile の出力（要素の作成・prop 書き込み・ツリー操作）を、構造化
//! 呼び出しの連なりではなく **mutation 列**として一方向に sink へ流す。これは Hayate の
//! `apply_mutations`「1 バッチ／frame」哲学（ADR-0002）と同型で、各メソッドは
//! `hayate_core::ElementTree` の対応 API（`element_create` / `element_set_text` /
//! `element_append_child` / `set_root`）に 1:1 で写る。実際の core を駆動する
//! `HayateSink` は、この trait をそのまま `ElementTree` に転送する薄い後続実装になる。
//!
//! テスト用の [`RecordingSink`] は全 mutation を記録し、fine-grained patch が
//! 「テキストノードだけを patch する」ことを検証可能にする（ADR-0006 tracer bullet）。

/// Hayate の element-kind 語彙（CONTEXT.md）。判別子は `hayate_core::ElementKind`
/// に一致させてあり、後続の `HayateSink` でそのまま写せる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    View = 0,
    Text = 1,
    Image = 2,
    Button = 3,
    TextInput = 4,
    ScrollView = 5,
}

/// sink が払い出す不透明な要素ハンドル。`hayate_core::ElementId` の代役。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElId(pub u64);

/// ランタイムが element ツリーを駆動するための mutation サーフェス。
pub trait ElementSink {
    /// 要素を作り、ハンドルを払い出す。
    fn create_element(&mut self, kind: ElementKind) -> ElId;
    /// text-like 要素の内容を設定する（fine-grained patch の唯一の text 経路）。
    fn set_text(&mut self, id: ElId, text: &str);
    /// 子を親の末尾に追加する。
    fn append_child(&mut self, parent: ElId, child: ElId);
    /// ルート要素を設定する。
    fn set_root(&mut self, id: ElId);
}

/// テスト用の sink。全 mutation を順序付きで記録する。
#[derive(Default)]
pub struct RecordingSink {
    next_id: u64,
    log: Vec<Mutation>,
}

/// 記録された 1 件の mutation。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mutation {
    Create { id: ElId, kind: ElementKind },
    SetText { id: ElId, text: String },
    AppendChild { parent: ElId, child: ElId },
    SetRoot { id: ElId },
}

impl RecordingSink {
    pub fn new() -> Self {
        RecordingSink::default()
    }

    /// 記録された mutation 列を参照する。
    pub fn log(&self) -> &[Mutation] {
        &self.log
    }

    /// 記録をクリアする（初期 instantiate 後に呼び、以降の patch だけを観測する）。
    pub fn clear_log(&mut self) {
        self.log.clear();
    }

    /// `set_text` mutation だけを `(id, text)` で抽出する。
    pub fn text_mutations(&self) -> Vec<(ElId, String)> {
        self.log
            .iter()
            .filter_map(|m| match m {
                Mutation::SetText { id, text } => Some((*id, text.clone())),
                _ => None,
            })
            .collect()
    }
}

impl ElementSink for RecordingSink {
    fn create_element(&mut self, kind: ElementKind) -> ElId {
        let id = ElId(self.next_id);
        self.next_id += 1;
        self.log.push(Mutation::Create { id, kind });
        id
    }

    fn set_text(&mut self, id: ElId, text: &str) {
        self.log.push(Mutation::SetText {
            id,
            text: text.to_string(),
        });
    }

    fn append_child(&mut self, parent: ElId, child: ElId) {
        self.log.push(Mutation::AppendChild { parent, child });
    }

    fn set_root(&mut self, id: ElId) {
        self.log.push(Mutation::SetRoot { id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_sink_assigns_sequential_ids_and_logs() {
        let mut sink = RecordingSink::new();
        let a = sink.create_element(ElementKind::View);
        let b = sink.create_element(ElementKind::Text);
        sink.append_child(a, b);
        sink.set_text(b, "hi");
        sink.set_root(a);

        assert_eq!(a, ElId(0));
        assert_eq!(b, ElId(1));
        assert_eq!(
            sink.log(),
            &[
                Mutation::Create {
                    id: a,
                    kind: ElementKind::View
                },
                Mutation::Create {
                    id: b,
                    kind: ElementKind::Text
                },
                Mutation::AppendChild {
                    parent: a,
                    child: b
                },
                Mutation::SetText {
                    id: b,
                    text: "hi".into()
                },
                Mutation::SetRoot { id: a },
            ]
        );
    }
}
