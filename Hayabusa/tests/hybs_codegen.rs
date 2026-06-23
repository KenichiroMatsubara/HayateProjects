//! `.hybs` build 時 codegen の統合（ADR-0008）：`components/counter.hybs` を build.rs が
//! コンパイルした `generated::counter::build` を instantiate し、**手組み counter
//! （tests/counter.rs）と同一に振る舞う**ことを実証する。
//!
//! これで「初回デモは `.hybs` をコンパイルした出力として動く」（ADR-0008）の経路が、
//! 既定の self-contained ビルド（外部依存ゼロ・ADR-0006）の上で通る。`feature` 不要。

use hayabusa::generated::counter;
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
