use hayate_core::{
    Color, CursorValue, Dimension, DocumentEventKind, ElementKind, ElementTree, Event,
    InputModality, PointerKind, PseudoState, StyleProp,
};

/// 200×200 ビューポートを埋める root View に `state` の擬似スタイルを与え、
/// ジェスチャ実行前に dirty が空になるよう一度レンダリングする。返す要素は
/// `:hover`/`:active`/`:focus` のボックス見た目を持つので、その擬似状態の
/// 無効化は visual-dirty として現れる。
fn pseudo_styled_root(state: PseudoState) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(100, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        state,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    // render は全 dirty を排出するので、ジェスチャ前は空の状態になる。
    tree.render(0.0);
    assert!(
        !tree.test_visual_dirty_contains(root),
        "render() must drain the dirty set so the gesture's mark is observable"
    );
    (tree, root)
}

/// `pseudo_styled_root` と同様だが、擬似ブロックに shape に影響するプロパティ
/// (`font-size`) を持たせ、無効化が *shape* set に入るようにする。focus 遷移は
/// カーソル点滅のため無条件に visual-dirty を立てる（ADR-0032）ので、shape set
/// 経由で検証することで `:active`/`:focus` の無効化をその visual マークから分離する。
fn pseudo_shaping_root(state: PseudoState) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(100, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_pseudo_style(root, state, &[StyleProp::FontSize(20.0)]);
    tree.render(0.0);
    assert!(
        !tree.test_shape_dirty_contains(root),
        "render() must drain the shape set so the gesture's mark is observable"
    );
    (tree, root)
}

#[test]
fn hover_enter_marks_hover_pseudo_dirty() {
    // HTML の mouseenter 経路は hover set を切り替える。ADR-0100 は対応する
    // `:hover` 無効化を同じ操作で行うことを要求し、要素が黙って乖離せず
    // hover の見た目で再 lower されるようにする。
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);

    tree.on_hover_enter(root);

    assert!(
        tree.test_visual_dirty_contains(root),
        "entering :hover must invalidate the element's :hover styling atomically"
    );
}

#[test]
fn hover_leave_marks_hover_pseudo_dirty() {
    // enter と対称: HTML の mouseleave 経路は要素を hover set から外し、
    // 同じ操作で `:hover` を無効化しなければならない（ADR-0100）。
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);
    tree.on_hover_enter(root);
    tree.render(0.0); // enter のマークを排出し leave のマークを分離する

    tree.on_hover_leave(root);

    assert!(
        tree.test_visual_dirty_contains(root),
        "leaving :hover must invalidate the element's :hover styling atomically"
    );
}

#[test]
fn pointer_move_hover_marks_hover_pseudo_dirty() {
    // pointer-move(canvas) の hover 経路は hover set 更新と同じステップで `:hover`
    // を無効化しなければならない。HTML mouseenter 経路と同じ原子性保証を、
    // 座標駆動の入力面から確認する。
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);

    assert!(tree.on_pointer_move(10.0, 10.0).moved);

    assert!(
        tree.test_visual_dirty_contains(root),
        "moving the pointer onto an element must invalidate its :hover styling"
    );
}

#[test]
fn pointer_down_marks_active_pseudo_dirty() {
    // 押下は `:active` を切り替える。対応する無効化は同じ操作に乗らねばならない
    // （ADR-0100）。focus 経路の無条件 visual マークが `:active` 無効化を覆い隠さない
    // よう、shape set 経由で検証する。
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Active);

    tree.on_pointer_down_on(root, 5.0, 5.0);

    assert!(
        tree.test_shape_dirty_contains(root),
        "a pointer-down must invalidate the pressed element's :active styling"
    );
}

#[test]
fn pointer_up_marks_active_pseudo_dirty() {
    // 解放は `:active` をクリアする。状態クリアとスタイル無効化は一操作。
    // pointer-up は focus を変えないので、ここで visual-dirty になるのは
    // `:active` の無効化以外あり得ない。
    let (mut tree, root) = pseudo_styled_root(PseudoState::Active);
    tree.on_pointer_down_on(root, 5.0, 5.0);
    tree.render(0.0); // 押下のマークを排出し解放のマークを分離する

    tree.on_pointer_up_on(Some(root));

    assert!(
        tree.test_visual_dirty_contains(root),
        "a pointer-up must invalidate the released element's :active styling"
    );
}

#[test]
fn focus_marks_focus_pseudo_dirty() {
    // フォーカスは `:focus` を切り替え、無効化は同じ操作に乗る。shape set で
    // element_focus が出すカーソル点滅の visual マークから分離する。
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Focus);

    tree.on_focus(root);

    assert!(
        tree.test_shape_dirty_contains(root),
        "focusing must invalidate the focused element's :focus styling"
    );
}

#[test]
fn blur_marks_focus_pseudo_dirty() {
    // blur は `:focus` をクリアし、一操作でそのスタイルを無効化する。
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Focus);
    tree.on_focus(root);
    tree.render(0.0); // focus のマークを排出し blur のマークを分離する

    tree.on_blur(root);

    assert!(
        tree.test_shape_dirty_contains(root),
        "blurring must invalidate the blurred element's :focus styling"
    );
}

/// 200×200 ビューポートを埋める root View。ヒットテスト用に境界を持つ。
fn hoverable_root() -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(100, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.render(0.0);
    (tree, root)
}

#[test]
fn pointer_leave_delivers_hover_leave_and_re_hover_re_enters() {
    let (mut tree, root) = hoverable_root();
    let enter = tree.register_listener(root, DocumentEventKind::HoverEnter);
    let leave = tree.register_listener(root, DocumentEventKind::HoverLeave);

    // 面にホバーで入る — root に HoverEnter。
    assert!(tree.on_pointer_move(10.0, 10.0).moved);
    let entered: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverEnter { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(entered, vec![enter]);

    // ポインタが面を出る — 直前にホバーしていた root に HoverLeave。
    tree.on_pointer_leave();
    let left: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverLeave { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(left, vec![leave]);

    // 再ホバーで `:hover` が再適用され、HoverEnter が再発火する。
    assert!(tree.on_pointer_move(20.0, 20.0).moved);
    let re_entered: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverEnter { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(re_entered, vec![enter]);
}

#[test]
fn pointer_leave_resets_last_pointer_pos_so_repeat_coord_is_not_deduped() {
    let (mut tree, _root) = hoverable_root();

    // 最初の move が last-pointer-position を確立し、同一座標の move は合体される。
    assert!(tree.on_pointer_move(30.0, 30.0).moved);
    assert!(!tree.on_pointer_move(30.0, 30.0).moved);

    // 面を出ると保存位置がクリアされ、全く同じ座標で再入場しても 1px dedup に
    // 飲まれず配送される。
    tree.on_pointer_leave();
    assert!(tree.on_pointer_move(30.0, 30.0).moved);
}

#[test]
fn pointer_leave_does_not_push_phantom_pointer_move() {
    let (mut tree, _root) = hoverable_root();
    assert!(tree.on_pointer_move(40.0, 40.0).moved);
    let _ = tree.poll_events(); // enter による move を排出する

    tree.on_pointer_leave();
    assert!(
        !tree
            .poll_events()
            .iter()
            .any(|e| matches!(e, Event::PointerMove { .. })),
        "on_pointer_leave must not fabricate a PointerMove"
    );
}

#[test]
fn pointer_release_dispatches_click_to_listener() {
    // クリックはリリースで確定する（ADR-0082）。Click は押下座標を載せる（タップなら
    // down≈up）。press だけでは配信されない（`tap_delivers_click_on_release_not_on_press`）。
    let mut tree = ElementTree::new();
    let btn = tree.element_create(1, ElementKind::Button);
    tree.set_root(btn);
    let listener = tree.register_listener(btn, DocumentEventKind::Click);

    tree.on_pointer_down_on(btn, 10.0, 20.0);
    tree.on_pointer_up_on(Some(btn));

    let clicks: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(d.event, Event::Click { .. }))
        .collect();
    assert_eq!(clicks.len(), 1);
    assert_eq!(clicks[0].listener_id, listener);
    assert!(matches!(
        &clicks[0].event,
        Event::Click { target_id, x, y }
            if *target_id == btn && (*x - 10.0).abs() < f32::EPSILON && (*y - 20.0).abs() < f32::EPSILON
    ));
}

#[test]
fn tap_delivers_click_on_release_not_on_press() {
    // タップは「押して離す」で確定する。クリックはリリース（pointer-up）で配信され、
    // 押下だけ（pointer-down）では配信されない。これにより slop を越えてスクロールに
    // 化けた押下を、リリース前にキャンセルしてクリックを抑止できる（ADR-0082）。
    let mut tree = ElementTree::new();
    let btn = tree.element_create(1, ElementKind::Button);
    tree.set_root(btn);
    let listener = tree.register_listener(btn, DocumentEventKind::Click);

    tree.on_pointer_down_on(btn, 10.0, 20.0);
    let after_down: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(d.event, Event::Click { .. }))
        .collect();
    assert!(
        after_down.is_empty(),
        "press alone must not deliver a click (got {after_down:?})"
    );

    tree.on_pointer_up(10.0, 20.0);
    let after_up: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(d.event, Event::Click { .. }))
        .collect();
    assert_eq!(after_up.len(), 1, "release resolves the tap into one click");
    assert_eq!(after_up[0].listener_id, listener);
    assert!(matches!(
        &after_up[0].event,
        Event::Click { target_id, .. } if *target_id == btn
    ));
}

#[test]
fn scroll_takeover_before_release_delivers_no_click() {
    // スクロールとクリックの区別: scroll-view 上のボタンを押してから指がスロップを
    // 越えると、アダプタは押下をキャンセルして以降をスクロールに振り向ける（ADR-0082）。
    // 押下がキャンセルされていれば、その後のリリースはクリックを発火してはならない。
    let mut tree = ElementTree::new();
    let btn = tree.element_create(1, ElementKind::Button);
    tree.set_root(btn);
    let _listener = tree.register_listener(btn, DocumentEventKind::Click);

    tree.on_pointer_down_on(btn, 10.0, 20.0);
    // slop 超過でアダプタが押下をキャンセル（スクロール乗っ取り）。
    tree.on_pointer_cancel();
    // スクロール後の指上げ。
    tree.on_pointer_up(10.0, 80.0);

    let clicks: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(d.event, Event::Click { .. }))
        .collect();
    assert!(
        clicks.is_empty(),
        "a press that became a scroll must not click (got {clicks:?})"
    );
}

#[test]
fn pointer_down_sets_focus_on_tree_only() {
    let mut tree = ElementTree::new();
    let btn = tree.element_create(2, ElementKind::Button);
    tree.set_root(btn);

    tree.on_pointer_down_on(btn, 0.0, 0.0);

    assert_eq!(tree.focused_element(), Some(btn));
    assert_eq!(tree.active_element(), Some(btn));
}

#[test]
fn pointer_move_skips_duplicate_coordinates() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[hayate_core::StyleProp::Width(hayate_core::Dimension::px(
            200.0,
        ))],
    );
    tree.render(0.0);

    assert!(tree.on_pointer_move(1.0, 2.0).moved);
    assert!(!tree.on_pointer_move(1.0, 2.0).moved);
    assert!(tree.on_pointer_move(2.0, 2.0).moved);
}

#[test]
fn pointer_move_resolves_cursor_from_hovered_element() {
    let (mut tree, root) = hoverable_root();
    tree.element_set_style(root, &[StyleProp::Cursor(CursorValue::Pointer)]);
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert!(result.moved, "a real move must not be coalesced");
    assert_eq!(result.resolved_cursor, CursorValue::Pointer);
}

#[test]
fn pointer_move_resolves_default_cursor_when_unset() {
    let (mut tree, _root) = hoverable_root();

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Default);
}

#[test]
fn pointer_move_inherits_cursor_from_ancestor() {
    // 自前の cursor を持たない子は祖先の `cursor` を継承する（CSS の継承と同じ）。
    // ボタンのテキストをホバーしてもボタンに設定した pointer カーソルが出る。
    let mut tree = ElementTree::new();
    let root = tree.element_create(40, ElementKind::View);
    let child = tree.element_create(41, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, child);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::Cursor(CursorValue::Pointer),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Pointer);
}

#[test]
fn pointer_move_resolves_pointer_cursor_over_button_by_kind() {
    // 明示的な `cursor` のないボタンも、要素種別の UA デフォルト（ADR-0105）で
    // pointer カーソルを出す。ブラウザの `<button>` と一致する。
    let mut tree = ElementTree::new();
    let root = tree.element_create(50, ElementKind::View);
    let button = tree.element_create(51, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, button);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Pointer);
}

#[test]
fn pointer_move_resolves_text_cursor_over_text_input_by_kind() {
    // 明示的な `cursor` のない text-input は、要素種別の UA デフォルト（ADR-0105）で
    // I ビーム（text）カーソルを出す。ブラウザの `<input>` と一致する。
    let mut tree = ElementTree::new();
    let root = tree.element_create(60, ElementKind::View);
    let input = tree.element_create(61, ElementKind::TextInput);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, input);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Text);
}

#[test]
fn pointer_move_resolves_text_cursor_over_selectable_text() {
    // 選択可能テキストは明示的な `cursor` がなくても I ビームを出す（ADR-0105）。
    // text-input と同じ UA デフォルトで、読み取り専用の選択領域も text として扱う。
    let mut tree = ElementTree::new();
    let root = tree.element_create(70, ElementKind::View);
    let text = tree.element_create(71, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, text);
    tree.element_set_selectable(text, true);
    tree.element_set_text(text, "hello");
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        text,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Text);
}

#[test]
fn explicit_cursor_overrides_element_kind_default() {
    // 明示的な `cursor` は要素種別デフォルトに常に優先する（ADR-0105）。
    // `not-allowed` を付けたボタンは pointer デフォルトでなく not-allowed になる。
    let mut tree = ElementTree::new();
    let root = tree.element_create(80, ElementKind::View);
    let button = tree.element_create(81, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, button);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::Cursor(CursorValue::NotAllowed),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::NotAllowed);
}

#[test]
fn pointer_down_on_miss_blurs_focused_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let btn = tree.element_create(11, ElementKind::Button);
    tree.set_root(root);
    tree.on_pointer_down_on(btn, 0.0, 0.0);
    assert_eq!(tree.focused_element(), Some(btn));

    tree.on_pointer_down(999.0, 999.0);

    assert_eq!(tree.focused_element(), None);
}

#[test]
fn pointer_cancel_ends_active_press_and_clears_hover() {
    let (mut tree, root) = hoverable_root();
    let l_hover_leave = tree.register_listener(root, DocumentEventKind::HoverLeave);
    let l_active_end = tree.register_listener(root, DocumentEventKind::ActiveEnd);

    // ホバー（要素上のポインタ）と active な押下を確立する。
    tree.on_pointer_move(10.0, 10.0);
    tree.on_pointer_down(10.0, 10.0);
    assert_eq!(tree.active_element(), Some(root));
    let _ = tree.poll_deliveries(); // enter/start の配送を排出する

    tree.on_pointer_cancel();

    let deliveries = tree.poll_deliveries();
    assert!(
        deliveries.iter().any(|d| d.listener_id == l_active_end
            && matches!(&d.event, Event::ActiveEnd { target_id } if *target_id == root)),
        "expected ActiveEnd delivered to the active element"
    );
    assert!(
        deliveries.iter().any(|d| d.listener_id == l_hover_leave
            && matches!(&d.event, Event::HoverLeave { target_id } if *target_id == root)),
        "expected HoverLeave delivered for the cleared hover chain"
    );
    assert_eq!(tree.active_element(), None);
}

#[test]
fn pointer_cancel_does_not_fabricate_pointer_move() {
    let (mut tree, _root) = hoverable_root();

    tree.on_pointer_move(40.0, 40.0);
    let _ = tree.poll_events(); // 上の move による実 PointerMove を排出する

    tree.on_pointer_cancel();

    let events = tree.poll_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PointerMove { .. })),
        "on_pointer_cancel must not push a phantom PointerMove"
    );
}

#[test]
fn click_bubbles_to_ancestors() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let leaf = tree.element_create(21, ElementKind::Button);
    tree.set_root(root);
    tree.element_append_child(root, leaf);

    let l_root = tree.register_listener(root, DocumentEventKind::Click);
    let l_leaf = tree.register_listener(leaf, DocumentEventKind::Click);

    // クリックはリリースで確定する（ADR-0082）。押して離すとタップが leaf で
    // 解決し、祖先 root まで bubble する。
    tree.on_pointer_down_on(leaf, 4.0, 5.0);
    tree.on_pointer_up_on(Some(leaf));

    let ids: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::Click { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(ids, vec![l_leaf, l_root]);
}

#[test]
fn last_pointer_kind_tracks_the_device_per_interaction() {
    // PointerKind { Mouse, Touch, Pen } は各ポインタ操作に乗る。Core は最新の
    // 種別を保持し、利用側がそれで分岐できるようにする。ポインタイベント前は
    // Mouse がデフォルト。
    let (mut tree, _root) = hoverable_root();
    assert_eq!(tree.last_pointer_kind(), PointerKind::Mouse);

    // touch 押下は Touch を記録する。
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);

    // 同じ面での pen move は種別を更新する（ハイブリッド端末はセッション途中で
    // 切り替わる。起動時にラッチされない）。
    tree.on_pointer_move_with_kind(20.0, 20.0, PointerKind::Pen);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Pen);

    // mouse 解放は再び Mouse を記録する。
    tree.on_pointer_up_with_kind(20.0, 20.0, PointerKind::Mouse);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Mouse);
}

#[test]
fn input_modality_is_a_separate_axis_from_pointer_kind() {
    // `:focus-visible` の InputModality (Pointer/Keyboard) は PointerKind と直交する。
    // touch 押下も InputModality::Pointer のままで、キー押下は保持中の pointer kind を
    // 変えずに modality だけを切り替える。
    let (mut tree, root) = hoverable_root();
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    assert_eq!(tree.last_input_modality(), InputModality::Pointer);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);

    tree.on_focus(root);
    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.last_input_modality(), InputModality::Keyboard);
    // キーボード操作は保持中の pointer kind を変えていない。
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);
}

#[test]
fn pointer_move_event_carries_the_pointer_kind() {
    // 発行される PointerMove wire イベントはデバイスを運ぶので、ホストは座標だけでなく
    // どのポインタが move を駆動したかを知れる。
    let (mut tree, _root) = hoverable_root();
    assert!(
        tree.on_pointer_move_with_kind(10.0, 10.0, PointerKind::Pen)
            .moved
    );
    let saw_pen = tree
        .poll_events()
        .into_iter()
        .any(|e| matches!(e, Event::PointerMove { pointer_kind, .. } if pointer_kind == PointerKind::Pen));
    assert!(saw_pen, "PointerMove must carry PointerKind::Pen");
}
