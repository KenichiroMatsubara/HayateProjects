//! 第一段階デモの統合：`components/todo.hybs`（`.hybs` codegen ＋ value 束縛 ＋ `on:input` ＋
//! `:if` / `:each` ＋ static style）を **実 App Host ＋ 実 `ElementTree`** で動かす。
//! 入力 → add → 行が増え、空状態が消える、までを実ツリーで観測する。
//!
//! これで第一段階（単一コンポーネントの Todo を `.hybs` コンパイル出力として画面に出す）が
//! 縦に通る。`feature = "app-host"` 専用。

#![cfg(feature = "app-host")]

use hayabusa::generated::todo;
use hayabusa::prelude::*;
use hayate_app_host::{AppHost, HeadlessPresentTarget};
use hayate_core::{DocumentEventKind, ElementId, Event};
use std::cell::RefCell;
use std::rc::Rc;

// 初期ツリーの作成順（深さ優先）：
//   view(0) / text-input(1) / button(2) / empty-state text(3)
const INPUT: u64 = 1;
const ADD_BUTTON: u64 = 2;
const EMPTY_TEXT: u64 = 3;

fn eid(raw: u64) -> ElementId {
    ElementId::from_u64(raw)
}

/// 文字入力（on:input）を 1 件配送する。
fn type_text(host: &mut AppHost<HeadlessPresentTarget>, text: &str, ts: f64) {
    host.tree_mut().dispatch_event(
        DocumentEventKind::TextInput,
        Event::TextInput {
            target_id: eid(INPUT),
            text: text.to_string(),
        },
    );
    host.tick(ts);
}

/// add ボタンをクリックする。
fn click_add(host: &mut AppHost<HeadlessPresentTarget>, ts: f64) {
    host.tree_mut().dispatch_event(
        DocumentEventKind::Click,
        Event::Click {
            target_id: eid(ADD_BUTTON),
            x: 0.0,
            y: 0.0,
        },
    );
    host.tick(ts);
}

fn mount_todo() -> AppHost<HeadlessPresentTarget> {
    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let instance = todo::build(&rt, sink);
    let mut host = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
    host.mount(Box::new(HayabusaApp::new(instance)));
    host.tick(0.0);
    host
}

#[test]
fn todo_starts_empty_with_the_empty_state_shown() {
    let host = mount_todo();
    // 初期は todos 空 → `:if={!todos}` が空状態テキストを mount する。
    assert_eq!(host.tree().element_get_text(eid(EMPTY_TEXT)), "no todos yet");
}

#[test]
fn typing_and_add_appends_a_row_and_hides_the_empty_state() {
    let mut host = mount_todo();

    type_text(&mut host, "milk", 16.0);
    click_add(&mut host, 32.0);

    // 行が 1 つ増え、その text に "milk" が入る。新規行はツリー末尾に作られる
    //（view 0 の子: input, button, [empty-state は除去], 行 view → 行 text）。
    // 空状態は :if が falsy になって unmount される（要素が消える）。
    assert_eq!(
        host.tree().element_get_text(eid(EMPTY_TEXT)),
        "",
        "empty-state text should be unmounted once a todo exists"
    );

    // 追加された行の text を探す：作成順で空状態(3) の後に 行view(4)/行text(5)。
    let row_text = host.tree().element_get_text(eid(5));
    assert_eq!(row_text, "milk", "the new row should render the typed label");
}

#[test]
fn adding_two_todos_renders_both_rows() {
    let mut host = mount_todo();

    type_text(&mut host, "milk", 16.0);
    click_add(&mut host, 32.0);
    type_text(&mut host, "eggs", 48.0);
    click_add(&mut host, 64.0);

    // 2 行ぶんのラベルが実ツリーに出ている（keyed `:each`）。行 text は 5 と 7。
    let labels: Vec<String> = [5u64, 7]
        .iter()
        .map(|&id| host.tree().element_get_text(eid(id)))
        .collect();
    assert!(labels.contains(&"milk".to_string()), "got {labels:?}");
    assert!(labels.contains(&"eggs".to_string()), "got {labels:?}");
}

#[test]
fn empty_input_is_ignored_by_add() {
    let mut host = mount_todo();
    // 何も入力せず add → 空入力は無視され、空状態のまま。
    click_add(&mut host, 16.0);
    assert_eq!(host.tree().element_get_text(eid(EMPTY_TEXT)), "no todos yet");
}
