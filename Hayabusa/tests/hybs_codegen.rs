//! `.hybs` build 時 codegen の統合（ADR-0008）：`components/counter.hybs` を build.rs が
//! コンパイルした `generated::counter::build` を instantiate し、**手組み counter
//! （tests/counter.rs）と同一に振る舞う**ことを実証する。
//!
//! これで「初回デモは `.hybs` をコンパイルした出力として動く」（ADR-0008）の経路が、
//! 既定の self-contained ビルド（外部依存ゼロ・ADR-0006）の上で通る。`feature` 不要。

use hayabusa::generated::{counter, text_field};
use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// 生成テンプレートの作成順（深さ優先）で払い出される ElId。
const TEXT: ElId = ElId(1);
const BUTTON: ElId = ElId(2);

#[test]
fn generated_component_renders_initial_text() {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = counter::build(&rt, sink.clone());

    assert_eq!(app.root(), ElId(0));
    // 束縛 Effect の初回実行で text ノードに "0"、静的ボタンに "+1"。
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(TEXT, "0".to_string()), (BUTTON, "+1".to_string())]
    );
}

#[test]
fn clicking_the_generated_button_patches_only_the_text_node() {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = counter::build(&rt, sink.clone());

    sink.borrow_mut().clear_log();
    assert!(app.click(BUTTON), "generated button has an on:click handler");

    // 手組み counter と同じく、increment でテキストノードだけが patch される。
    assert_eq!(
        sink.borrow().log(),
        &[Mutation::SetText {
            id: TEXT,
            text: "1".into()
        }]
    );
}

#[test]
fn repeated_clicks_keep_patching_only_the_text_node() {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = counter::build(&rt, sink.clone());
    sink.borrow_mut().clear_log();

    for _ in 0..3 {
        app.click(BUTTON);
    }

    assert_eq!(
        sink.borrow().text_mutations(),
        vec![
            (TEXT, "1".to_string()),
            (TEXT, "2".to_string()),
            (TEXT, "3".to_string()),
        ]
    );
    assert_eq!(sink.borrow().log().len(), 3, "no structural mutations after build");
}

// ───────────────────────── text_field.hybs（value 束縛 ＋ on:input・ADR-0007） ─────────────────────────

// text_field の作成順：view(0), text-input(1), button(2)。
const FIELD_INPUT: ElId = ElId(1);

#[test]
fn generated_text_field_compiles_and_wires_value_binding() {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = text_field::build(&rt, sink.clone());

    // value 束縛 Effect の初回実行で、空の programmatic set が text-input に出る。
    assert_eq!(
        sink.borrow().value_mutations(),
        vec![(FIELD_INPUT, "".to_string())]
    );
}

#[test]
fn generated_text_field_on_input_drives_value_set() {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = text_field::build(&rt, sink.clone());
    sink.borrow_mut().clear_log();

    // 生成された on:input ハンドラ（draft = payload）→ value 束縛が set_value を再発行。
    assert!(app.input(FIELD_INPUT, "hi"));
    assert_eq!(
        sink.borrow().value_mutations(),
        vec![(FIELD_INPUT, "hi".to_string())]
    );
}
