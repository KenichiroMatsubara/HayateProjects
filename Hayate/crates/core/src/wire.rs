//! Hayate wire プロトコルの decode と中立 `apply_mutations` dispatch（生成物。
//! ADR-0052 / ADR-0112）。
//!
//! Tsubame ↔ Hayate の wire 契約（op レコード・style packet・event エンコード）の
//! decode と、パース済み op を適用する中立 dispatch を、Hayate 側で単一所有する
//! （Hayate Protocol Contract: decode は Hayate 側が正本）。
//! `proto/generator` が `protocol.rs`（wire 定数・`Op` decode・style packet decode・
//! event-wire encode）・`mutation_sink.rs`（mode 非依存の [`MutationSink`] trait）・
//! `dispatch.rs`（wire を decode して `MutationSink` を駆動する中立 decode）を
//! 生成し、ここに include する。
//!
//! 適用先は [`MutationSink`] で抽象化する。即時 `&mut ElementTree` 適用（Canvas
//! Mode / Android）は [`TreeSink`] が、遅延コマンド enqueue（HTML Mode）は web
//! アダプタの sink が実装する。decode 自体は一度だけ生成され、sink が「即時木適用 /
//! 遅延 enqueue」という irreducible な差だけを供給する。
//!
//! 各プラットフォームアダプタは本モジュールの公開 API（[`apply_mutations`]・
//! [`apply_mutations_to_sink`]・[`MutationSink`]・[`TreeSink`]）を使い、
//! decode/dispatch を再実装も再 include もしない。

use crate::element::tree::ElementTree;
use crate::{
    ElementId, ElementKind, PseudoState, StyleProp, StylePropKind, UserSelectValue,
    ViewportCondition,
};

/// wire 定数・`Op`/`StyleTag` decode・style packet decode・event-wire encode（生成物）。
pub mod protocol {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/protocol.rs"
    ));
}

/// mode 非依存のバッチミューテーション sink trait（生成物。opcodes.json から1メソッド/op）。
pub mod mutation_sink {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/mutation_sink.rs"
    ));
}

/// wire を decode して [`MutationSink`] を駆動する中立 dispatch（生成物）。
mod dispatch {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/dispatch.rs"
    ));
}

pub use dispatch::{apply_mutations_to_sink, unset_kind_from_u32};
pub use mutation_sink::MutationSink;
pub use protocol::*;

/// 即時 `&mut ElementTree` 適用 sink（Canvas Mode / Android）。中立 decode が以前
/// インライン展開していた op→tree 本体をここに集約する。木へ直接適用する唯一の
/// sink 形であり、Web Canvas と Android が共有する（どちらも retained ツリーへ即時
/// 適用する＝同一形のため core が単一所有する。ADR-0112 / ADR-0117）。
pub struct TreeSink<'a> {
    pub tree: &'a mut ElementTree,
}

impl MutationSink for TreeSink<'_> {
    fn append_child(&mut self, parent: ElementId, child: ElementId) {
        self.tree.element_append_child(parent, child);
    }

    fn insert_before(&mut self, parent: ElementId, child: ElementId, before: ElementId) {
        self.tree.element_insert_before(parent, child, before);
    }

    fn remove(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    fn set_root(&mut self, id: ElementId) {
        self.tree.set_root(id);
    }

    fn set_style(&mut self, id: ElementId, props: Vec<StyleProp>) {
        self.tree.element_set_style(id, &props);
    }

    fn set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        self.tree.element_set_transform(id, matrix);
    }

    fn set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        self.tree.element_set_scroll_offset(id, x, y);
    }

    fn apply_focus(&mut self, id: ElementId) {
        self.tree.on_focus(id);
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.tree.on_blur(id);
    }

    fn create(&mut self, id: ElementId, kind: ElementKind) {
        self.tree.element_create(id.to_u64(), kind);
    }

    fn set_text(&mut self, id: ElementId, text: &str) {
        self.tree.element_set_text(id, text);
    }

    fn unset_style(&mut self, id: ElementId, kind: StylePropKind) {
        self.tree.element_unset_style(id, &[kind]);
    }

    fn set_text_content(&mut self, id: ElementId, text: &str) {
        // controlled text-input の value 書き戻し経路（ADR-0007）。Tsubame（React /
        // Solid）は `value={state}` を毎レンダー echo するため、無条件 `set()` だと
        // 変化のない echo でもキャレットを末尾へ collapse し、IME 組成中なら preedit を
        // 破壊する（ユーザー報告の謎挙動）。Hayabusa の in-process sink と同じく
        // idle ガード（差分あり かつ 非組成中のみ適用）を通す。
        self.tree.element_set_text_content_if_idle(id, text);
    }

    fn set_disabled(&mut self, id: ElementId, disabled: bool) {
        self.tree.element_set_disabled(id, disabled);
    }

    fn set_src(&mut self, id: ElementId, url: &str) {
        self.tree.element_set_src(id, url);
    }

    fn set_pseudo_style(&mut self, id: ElementId, state: PseudoState, props: Vec<StyleProp>) {
        self.tree.element_set_pseudo_style(id, state, &props);
    }

    fn set_style_variant(&mut self, id: ElementId, condition: ViewportCondition, prop: StyleProp) {
        self.tree.element_set_style_variant(id, condition, prop);
    }

    fn set_user_select(&mut self, id: ElementId, value: UserSelectValue) {
        // ADR-0108: Selection Region boolean を `user-select` 語彙の橋渡しとして併走
        // させる（`none` は選択不可、`text` / `contains` は選択可）。
        self.tree
            .element_set_selectable(id, value != UserSelectValue::None);
        self.tree.element_set_user_select(id, value);
    }

    fn set_multiline(&mut self, id: ElementId, multiline: bool) {
        self.tree.element_set_multiline(id, multiline);
    }

    fn set_aria_label(&mut self, id: ElementId, label: &str) {
        self.tree.element_set_aria_label(id, label);
    }

    fn set_role(&mut self, id: ElementId, role: &str) {
        self.tree.element_set_role(id, role);
    }

    fn set_font_family(&mut self, id: ElementId, family: &str) {
        self.tree.element_set_font_family(id, family);
    }

    fn set_draw(&mut self, id: ElementId, commands: Vec<protocol::DrawCommand>) {
        self.tree.element_set_draw(id, commands);
    }
}

/// 中立 `apply_mutations`（ADR-0052 / ADR-0112）。wire ops（op レコード列・style
/// packet の f32 列・`OP_SET_TEXT` 等が参照する文字列テーブル・`OP_SET_DRAW` が
/// 参照する draw display list の f32 列）を decode して `ElementTree` に即時適用
/// する。Web Canvas（wasm 境界）と Android（埋め込み Hermes）の両アダプタが共有
/// する木即時適用の経路。HTML Mode は自身の遅延 sink を
/// [`apply_mutations_to_sink`] に渡す。
pub fn apply_mutations(
    tree: &mut ElementTree,
    ops: &[f64],
    styles: &[f32],
    texts: &[String],
    draws: &[f32],
) -> Result<(), String> {
    let mut sink = TreeSink { tree };
    apply_mutations_to_sink(&mut sink, ops, styles, texts, draws)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementTree;

    use crate::element::kind::ElementKind;

    // 空バッチは no-op で Ok（境界の配線確認）。
    #[test]
    fn empty_batch_is_ok() {
        let mut tree = ElementTree::new();
        let texts: Vec<String> = Vec::new();
        assert!(apply_mutations(&mut tree, &[], &[], &texts, &[]).is_ok());
    }

    /// 回帰（ユーザー報告: React/Tsubame の controlled text-input が謎挙動）。
    ///
    /// controlled input は `value={state}` を毎レンダー書き戻す。ユーザーがフィールド
    /// 途中にキャレットを置いた状態で、変化のない value が wire 経由で echo され戻る
    /// と、`SetTextContent` op がキャレットを末尾へ collapse してはいけない（差分が
    /// 無いので no-op であるべき）。`TreeSink`（Canvas / Android の wire sink）は
    /// Hayabusa と同じく idle ガード経路を通す（ADR-0007）。
    #[test]
    fn wire_value_echo_of_unchanged_text_does_not_move_the_caret() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        tree.element_set_text_content(input, "helloworld");
        tree.element_set_selection(input, 5, 5); // hello|world

        // React が onInput で受けた現在値をそのまま wire で書き戻す（controlled echo）。
        {
            let mut sink = TreeSink { tree: &mut tree };
            sink.set_text_content(input, "helloworld");
        }

        assert_eq!(
            tree.element_caret_byte_index(input),
            Some(5),
            "unchanged value echo must be a no-op, not collapse the caret to the end",
        );

        // ただし本物の value 変化（「追加」で draft を "" に戻す）はちゃんと適用される。
        {
            let mut sink = TreeSink { tree: &mut tree };
            sink.set_text_content(input, "");
        }
        assert_eq!(
            tree.element_get_text_content(input),
            "",
            "a real programmatic value change (clear-on-add) still applies",
        );
    }

    /// 回帰: IME 組成中（preedit あり）に value が echo されても、preedit を確定/破棄
    /// してはいけない（「入力確定」中に変換が壊れる謎挙動）。
    #[test]
    fn wire_value_echo_while_composing_does_not_clobber_preedit() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        tree.element_set_preedit(input, "へんかん"); // 変換中

        // 表示テキスト（content + preedit）が wire で echo され戻る。
        {
            let mut sink = TreeSink { tree: &mut tree };
            sink.set_text_content(input, "へんかん");
        }

        // 確定 text_content は空のまま（preedit は生きている）。
        assert_eq!(
            tree.element_caret_byte_index(input),
            Some(0),
            "composition must survive the echo; committed content stays empty",
        );
    }

    /// ADR-0054 RecordingPainter と同型の記録 sink。木も DOM も持たず、decode が
    /// 生む意味的ミューテーション列だけを観測する。pure ops → semantic mutations。
    #[derive(Default)]
    struct RecordingSink {
        calls: Vec<String>,
    }

    impl MutationSink for RecordingSink {
        fn append_child(&mut self, parent: ElementId, child: ElementId) {
            self.calls
                .push(format!("append_child({}, {})", parent.to_u64(), child.to_u64()));
        }
        fn insert_before(&mut self, parent: ElementId, child: ElementId, before: ElementId) {
            self.calls.push(format!(
                "insert_before({}, {}, {})",
                parent.to_u64(),
                child.to_u64(),
                before.to_u64()
            ));
        }
        fn remove(&mut self, id: ElementId) {
            self.calls.push(format!("remove({})", id.to_u64()));
        }
        fn set_root(&mut self, id: ElementId) {
            self.calls.push(format!("set_root({})", id.to_u64()));
        }
        fn set_style(&mut self, id: ElementId, props: Vec<StyleProp>) {
            self.calls
                .push(format!("set_style({}, {} props)", id.to_u64(), props.len()));
        }
        fn set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
            self.calls
                .push(format!("set_transform({}, {})", id.to_u64(), matrix.is_some()));
        }
        fn set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
            self.calls
                .push(format!("set_scroll_offset({}, {x}, {y})", id.to_u64()));
        }
        fn apply_focus(&mut self, id: ElementId) {
            self.calls.push(format!("apply_focus({})", id.to_u64()));
        }
        fn apply_blur(&mut self, id: ElementId) {
            self.calls.push(format!("apply_blur({})", id.to_u64()));
        }
        fn create(&mut self, id: ElementId, kind: ElementKind) {
            self.calls
                .push(format!("create({}, {kind:?})", id.to_u64()));
        }
        fn set_text(&mut self, id: ElementId, text: &str) {
            self.calls.push(format!("set_text({}, {text:?})", id.to_u64()));
        }
        fn unset_style(&mut self, id: ElementId, kind: StylePropKind) {
            self.calls
                .push(format!("unset_style({}, {kind:?})", id.to_u64()));
        }
        fn set_text_content(&mut self, id: ElementId, text: &str) {
            self.calls
                .push(format!("set_text_content({}, {text:?})", id.to_u64()));
        }
        fn set_disabled(&mut self, id: ElementId, disabled: bool) {
            self.calls
                .push(format!("set_disabled({}, {disabled})", id.to_u64()));
        }
        fn set_src(&mut self, id: ElementId, url: &str) {
            self.calls.push(format!("set_src({}, {url:?})", id.to_u64()));
        }
        fn set_pseudo_style(&mut self, id: ElementId, state: PseudoState, props: Vec<StyleProp>) {
            self.calls.push(format!(
                "set_pseudo_style({}, {state:?}, {} props)",
                id.to_u64(),
                props.len()
            ));
        }
        fn set_style_variant(
            &mut self,
            id: ElementId,
            _condition: ViewportCondition,
            _prop: StyleProp,
        ) {
            self.calls
                .push(format!("set_style_variant({})", id.to_u64()));
        }
        fn set_user_select(&mut self, id: ElementId, value: UserSelectValue) {
            self.calls
                .push(format!("set_user_select({}, {value:?})", id.to_u64()));
        }
        fn set_multiline(&mut self, id: ElementId, multiline: bool) {
            self.calls
                .push(format!("set_multiline({}, {multiline})", id.to_u64()));
        }
        fn set_aria_label(&mut self, id: ElementId, label: &str) {
            self.calls
                .push(format!("set_aria_label({}, {label:?})", id.to_u64()));
        }
        fn set_role(&mut self, id: ElementId, role: &str) {
            self.calls.push(format!("set_role({}, {role:?})", id.to_u64()));
        }
        fn set_font_family(&mut self, id: ElementId, family: &str) {
            self.calls
                .push(format!("set_font_family({}, {family:?})", id.to_u64()));
        }
        fn set_draw(&mut self, id: ElementId, commands: Vec<protocol::DrawCommand>) {
            self.calls
                .push(format!("set_draw({}, {} commands)", id.to_u64(), commands.len()));
        }
    }

    // decode は木も DOM も無く意味的ミューテーションへ落ちる。CREATE→SET_TEXT を
    // 記録 sink で観測し、text-table 参照と op 順序が保たれることを確認する。
    #[test]
    fn recording_sink_observes_decoded_mutations() {
        let mut sink = RecordingSink::default();
        let ops = vec![
            OP_CREATE as f64,
            7.0,
            ELEMENT_KIND_TEXT as f64,
            OP_SET_TEXT as f64,
            7.0,
            0.0,
        ];
        let texts = vec!["hello".to_string()];
        apply_mutations_to_sink(&mut sink, &ops, &[], &texts, &[]).unwrap();
        assert_eq!(
            sink.calls,
            vec!["create(7, Text)".to_string(), "set_text(7, \"hello\")".to_string()]
        );
    }

    // 新規 opcode（aria-label / role / font-family）が wire を1往復し、文字列
    // テーブル参照込みで sink の意味メソッドへ届く。
    #[test]
    fn recording_sink_decodes_new_string_opcodes() {
        let mut sink = RecordingSink::default();
        let ops = vec![
            OP_SET_ARIA_LABEL as f64,
            1.0,
            0.0,
            OP_SET_ROLE as f64,
            1.0,
            1.0,
            OP_SET_FONT_FAMILY as f64,
            1.0,
            2.0,
        ];
        let texts = vec!["Close".to_string(), "button".to_string(), "Inter".to_string()];
        apply_mutations_to_sink(&mut sink, &ops, &[], &texts, &[]).unwrap();
        assert_eq!(
            sink.calls,
            vec![
                "set_aria_label(1, \"Close\")".to_string(),
                "set_role(1, \"button\")".to_string(),
                "set_font_family(1, \"Inter\")".to_string(),
            ]
        );
    }

    // draws チャネル（texts と同格の Float32Array・#724）: SET_DRAW が draws バッファの
    // オフセット/長さ参照を decode 済み DrawCommand 列へ解決して sink の意味メソッドへ
    // 届ける（display list の中身は draw_codec_fixtures が検証）。
    #[test]
    fn recording_sink_decodes_set_draw_from_draws_channel() {
        let mut sink = RecordingSink::default();
        // id=5, draw_offset=3, draw_len=14（先頭3スロットはオフセット参照確認用のパディング）
        let ops = vec![OP_SET_DRAW as f64, 5.0, 3.0, 14.0];
        let draws = vec![
            9.0,
            9.0,
            9.0,
            DRAW_OP_MOVE_TO as f32,
            0.0,
            0.0,
            DRAW_OP_LINE_TO as f32,
            10.0,
            0.0,
            DRAW_OP_CLOSE as f32,
            DRAW_OP_FILL as f32,
            5.0,
            DRAW_PAINT_COLOR as f32,
            1.0,
            0.0,
            0.0,
            1.0,
        ];
        apply_mutations_to_sink(&mut sink, &ops, &[], &[], &draws).unwrap();
        assert_eq!(sink.calls, vec!["set_draw(5, 1 commands)".to_string()]);
    }

    // draws バッファ外を指す SET_DRAW はエラー（黙って無視しない）。
    #[test]
    fn set_draw_out_of_bounds_is_an_error() {
        let mut sink = RecordingSink::default();
        let ops = vec![OP_SET_DRAW as f64, 5.0, 0.0, 4.0];
        assert!(apply_mutations_to_sink(&mut sink, &ops, &[], &[], &[]).is_err());
    }

    // user-select の wire 値（text=0 / none=1 / contains=2）が意味 enum へ decode される。
    #[test]
    fn recording_sink_decodes_user_select_vocabulary() {
        let mut sink = RecordingSink::default();
        let ops = vec![
            OP_SET_USER_SELECT as f64,
            1.0,
            1.0,
            OP_SET_USER_SELECT as f64,
            2.0,
            2.0,
            OP_SET_USER_SELECT as f64,
            3.0,
            0.0,
        ];
        apply_mutations_to_sink(&mut sink, &ops, &[], &[], &[]).unwrap();
        assert_eq!(
            sink.calls,
            vec![
                "set_user_select(1, None)".to_string(),
                "set_user_select(2, Contains)".to_string(),
                "set_user_select(3, Text)".to_string(),
            ]
        );
    }
}
