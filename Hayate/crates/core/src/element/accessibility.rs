//! ElementTree から AccessKit ツリーを生成する（ADR-0041）。

use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate,
};

use super::taffy_projection::TraversalStep;
use super::tree::Element;
use super::{DocumentEventKind, ElementId, ElementKind, ElementTree, Event};

fn node_id(id: ElementId) -> NodeId {
    NodeId(id.to_u64())
}

/// 1 軸について、対象スパン `[content_pos, content_pos + size]` がビューポート
/// `[offset, offset + viewport]` 内に収まる最小の新オフセットを `[0, max]` に
/// クランプして返す。
///
/// 対象が既に完全表示（またはビューポート全体を覆う）なら現在の `offset` を
/// そのまま返し、表示済みの対象は動かさない。それ以外は近い側の端に揃える：
/// ビューポートより手前なら先頭端、はみ出すなら末尾端。オフセット計算のみで
/// 慣性は持たない（ADR-0098）。
fn scroll_axis_to_reveal(content_pos: f32, size: f32, viewport: f32, offset: f32, max: f32) -> f32 {
    let lead = content_pos;
    let trail = content_pos + size;
    let view_lead = offset;
    let view_trail = offset + viewport;
    let new_offset = if lead >= view_lead && trail <= view_trail {
        offset
    } else if lead < view_lead && trail > view_trail {
        // 対象がビューポート全体を覆う場合、最も近い解は「動かさない」。
        offset
    } else if lead < view_lead {
        lead
    } else {
        trail - viewport
    };
    new_offset.clamp(0.0, max)
}

/// Core が扱う、受信 AccessKit アクションのサポート済みサブセット（ADR-0098）。
///
/// AccessKit の `Action` は広いプロトコル語彙だが、Core は実際に駆動する操作だけに
/// 写像し、それ以外は `Ignored` に畳む。これで受信面は全域となり、ランタイムが
/// ネイティブ固有の概念を見ることはない。写像は Core（Rust API）に閉じ、proto の
/// ワイヤには載せない（ADR-0098）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityAction {
    /// 既存のフォーカス状態機械を駆動して `target` にフォーカスを移す。
    Focus { target: ElementId },
    /// `target` に直接 `Click` イベントを発行して起動する。タップと意味的に等価で
    /// （ADR-0098）、合成ポインタの再生ではない。ヒットテスト・`:active`・
    /// マルチクリック計数・フォーカスジェスチャを飛ばす。
    Click { target: ElementId },
    /// `target` のテキスト入力値を `value` で置換する（ADR-0098）。置換前に進行中の
    /// preedit を確定し、変換が置換をまたいで残らないようにする（`element_paste` と
    /// 同じ整合性）。`text-input` 以外の対象では何もしない。
    SetValue { target: ElementId, value: String },
    /// 最寄りの祖先 `scroll-view` の Scroll Offset を調整して `target` を表示に入れる
    /// （ADR-0098）。Core は基本オフセットのみ設定し、慣性・スナップ・ラバーバンドの
    /// 物理は Platform Adapter に委ねる（AT 駆動のスクロールはそれらを伴わない）。
    /// `target` に scroll-view 祖先がない、または既に完全表示なら何もしない。
    ScrollIntoView { target: ElementId },
    /// 非サポートのアクション。何もせず、観測可能な状態は変えない。
    Ignored,
}

/// AccessKit `ActionRequest` から Core アクションサブセットへの純粋な写像
/// （ADR-0098）。受信 `NodeId` は v1 では要素のみ解決し、送信側
/// `NodeId(ElementId.to_u64())` の逆、すなわち `ElementId::from_u64` を用いる。
pub fn map_action_request(req: &ActionRequest) -> AccessibilityAction {
    let target = ElementId::from_u64(req.target_node.0);
    match req.action {
        Action::Focus => AccessibilityAction::Focus { target },
        // AccessKit は「デフォルト起動」を `Action::Click` に畳み、独立した
        // `Default` バリアントを持たないため、ADR の「Click/Default」はここに対応する。
        Action::Click => AccessibilityAction::Click { target },
        // `SetValue` は新テキストを `data` に持つ。文字列 `Value` ペイロードのみが
        // text-input を指し、欠落や非文字列（例: 数値スライダ値）は設定対象がないため
        // `Ignored` に畳む。
        Action::SetValue => match &req.data {
            Some(ActionData::Value(value)) => AccessibilityAction::SetValue {
                target,
                value: value.to_string(),
            },
            _ => AccessibilityAction::Ignored,
        },
        Action::ScrollIntoView => AccessibilityAction::ScrollIntoView { target },
        _ => AccessibilityAction::Ignored,
    }
}

fn aria_role(role: &str) -> Option<Role> {
    match role {
        "button" => Some(Role::Button),
        "label" => Some(Role::Label),
        "text-input" | "textbox" => Some(Role::TextInput),
        "scroll-view" => Some(Role::ScrollView),
        "image" | "img" => Some(Role::Image),
        "list" => Some(Role::List),
        "list-item" | "listitem" => Some(Role::ListItem),
        "heading" => Some(Role::Heading),
        "link" => Some(Role::Link),
        "navigation" => Some(Role::Navigation),
        "main" => Some(Role::Main),
        "dialog" => Some(Role::Dialog),
        "alert-dialog" => Some(Role::AlertDialog),
        "generic-container" => Some(Role::GenericContainer),
        _ => None,
    }
}

fn implicit_role(kind: ElementKind) -> Role {
    match kind {
        ElementKind::View => Role::GenericContainer,
        ElementKind::Text => Role::Label,
        ElementKind::Image => Role::Image,
        ElementKind::Button => Role::Button,
        ElementKind::TextInput => Role::TextInput,
        ElementKind::ScrollView => Role::ScrollView,
    }
}

fn resolve_role(el: &Element, is_root: bool) -> Role {
    if is_root {
        return Role::Window;
    }
    if let Some(role) = el.role.as_deref().and_then(aria_role) {
        return role;
    }
    implicit_role(el.kind)
}

fn element_value(el: &Element) -> Option<String> {
    match el.kind {
        ElementKind::Text => el.text.clone(),
        ElementKind::TextInput => el.edit.as_ref().map(|edit| edit.display_text()),
        ElementKind::Button => el.text.clone(),
        _ => None,
    }
}

fn build_node(el: &Element, bounds: (f32, f32, f32, f32), is_root: bool) -> Node {
    let (x, y, w, h) = bounds;
    let mut node = Node::new(resolve_role(el, is_root));
    node.set_bounds(Rect {
        x0: x as f64,
        y0: y as f64,
        x1: (x + w) as f64,
        y1: (y + h) as f64,
    });
    if let Some(label) = &el.aria_label {
        node.set_label(label.clone());
    }
    if let Some(value) = element_value(el) {
        node.set_value(value);
    }
    node
}

/// Canonical Tree を辿って AccessKit ノードを構築し、`id` のサブツリーから生成した
/// トップレベルノードの id を返す（呼び出し側が子として接続できるように）。
///
/// Taffy ノードを持たない要素（例: IFC 内のインラインテキスト）はスキップするが、
/// その子へは再帰し、トップレベルノードは最寄りの Taffy ノードを持つ祖先まで
/// 浮上させる。これが IFC サブツリーの脱落を防ぐ。
fn walk_accessibility(
    tree: &ElementTree,
    id: ElementId,
    root_id: ElementId,
    nodes: &mut Vec<(NodeId, Node)>,
) -> Vec<NodeId> {
    let step = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(step) => step,
        None => return Vec::new(),
    };

    let el = match step {
        TraversalStep::Skip(el) => {
            let mut top_ids = Vec::new();
            for &child in &el.children {
                top_ids.extend(walk_accessibility(tree, child, root_id, nodes));
            }
            return top_ids;
        }
        TraversalStep::Visit(_, el) => el,
    };

    let Some((x, y, w, h)) = tree.layout.geometry(id) else {
        return Vec::new();
    };

    let mut node = build_node(el, (x, y, w, h), id == root_id);
    let this_id = node_id(id);

    for &child in &el.children {
        for child_id in walk_accessibility(tree, child, root_id, nodes) {
            node.push_child(child_id);
        }
    }

    nodes.push((this_id, node));
    vec![this_id]
}

impl ElementTree {
    /// 受信 AccessKit アクション面（ADR-0098）。送信側 `accessibility_update` の鏡像。
    /// Platform Adapter は AT リクエストをここへ橋渡しし、Core は既存のランタイム意図に
    /// 写像する（Flutter 流の意味的アクション。合成ポインタやキー再生は使わない）。
    /// 非サポートのアクションは `Ignored` に畳まれ何もしない。
    pub fn on_accessibility_action(&mut self, req: ActionRequest) {
        match map_action_request(&req) {
            AccessibilityAction::Focus { target } => self.transition_focus(target),
            AccessibilityAction::Click { target } => self.emit_semantic_click(target),
            AccessibilityAction::SetValue { target, value } => self.apply_set_value(target, &value),
            AccessibilityAction::ScrollIntoView { target } => self.scroll_into_view(target),
            AccessibilityAction::Ignored => {}
        }
    }

    /// 最寄りの祖先 `scroll-view` の Scroll Offset を設定して `target` を表示に入れる
    /// （ADR-0098）。対象の境界が見えるようになる最小オフセットを計算し（ビューポート
    /// より手前なら先頭端、過ぎていれば末尾端）、既に完全表示なら触れない。オフセットが
    /// 実際に動いたときに限り scroll-view へ `Scroll` を配送する。慣性やスナップは持たず、
    /// AT 駆動のスクロールは基本オフセットのみ設定する。scroll-view 祖先がなければ何もしない。
    fn scroll_into_view(&mut self, target: ElementId) {
        let Some(scroll_view) = super::tree::next_ancestor_scroll_view(self, target) else {
            return;
        };
        let (Some((sx, sy, sw, sh)), Some((tx, ty, tw, th))) = (
            self.element_layout_rect(scroll_view),
            self.element_layout_rect(target),
        ) else {
            return;
        };

        let (ox, oy) = self.element_get_scroll_offset(scroll_view);
        let (max_x, max_y) = self.element_scroll_max_offset(scroll_view);

        // layout_cache は未スクロールのコンテンツ空間座標を保持し、オフセットは下流の
        // 変換として適用される（ADR-0022）。よってコンテンツ内の対象位置は
        // `(tx - sx, ty - sy)` でオフセットに依存しない。
        let new_x = scroll_axis_to_reveal(tx - sx, tw, sw, ox, max_x);
        let new_y = scroll_axis_to_reveal(ty - sy, th, sh, oy, max_y);

        if (new_x - ox).abs() < 1e-3 && (new_y - oy).abs() < 1e-3 {
            return;
        }
        self.element_set_scroll_offset(scroll_view, new_x, new_y);
        self.dispatch_event(
            DocumentEventKind::Scroll,
            Event::Scroll {
                target_id: scroll_view,
                delta_x: new_x - ox,
                delta_y: new_y - oy,
            },
        );
    }

    /// 意味的起動として `target` に直接 `Click` を発行する（ADR-0098）。座標は対象の
    /// レイアウト中心とし、座標を読む既存リスナーをワイヤ変更なしに互換に保つ。イベントは
    /// 通常の `Click` と同様にバブルしディスパッチされる。ポインタパイプラインを完全に
    /// 迂回し、ヒットテスト・`:active`・マルチクリック計数・フォーカスジェスチャを行わない。
    fn emit_semantic_click(&mut self, target: ElementId) {
        let (x, y) = self
            .layout
            .geometry(target)
            .map(|(x, y, w, h)| (x + w / 2.0, y + h / 2.0))
            .unwrap_or((0.0, 0.0));
        self.dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: target,
                x,
                y,
            },
        );
    }

    /// AccessKit `SetValue` を意味的な値置換として `target` に適用する（ADR-0098）。
    /// 進行中の preedit を確定し、text-input の内容を `value` で置換し、`TextInput`
    /// イベントをキューしてアプリのリスナーが通常の編集と同様に観測できるようにする。
    /// `text-input` 以外の対象では何もせず、表示値が変わらない場合も無音。
    fn apply_set_value(&mut self, target: ElementId, value: &str) {
        let el = match self.elements.get_mut(&target) {
            Some(e) if e.kind == ElementKind::TextInput => e,
            _ => return,
        };
        let Some(edit) = el.edit.as_mut() else {
            return;
        };
        if !edit.set_value(value) {
            return;
        }
        self.dispatch_event(
            DocumentEventKind::TextInput,
            Event::TextInput {
                target_id: target,
                text: value.to_string(),
            },
        );
    }

    /// 現在の要素ツリーとレイアウトキャッシュから AccessKit `TreeUpdate` を構築する。
    ///
    /// レイアウト未実行、またはツリーにルートがなければ `None` を返す。
    pub fn accessibility_update(&self) -> Option<TreeUpdate> {
        let root_id = self.root?;
        if !self.layout.has_geometry() {
            return None;
        }

        let mut nodes = Vec::new();
        walk_accessibility(self, root_id, root_id, &mut nodes);

        let focus = self
            .focused_element
            .map(node_id)
            .unwrap_or_else(|| node_id(root_id));

        Some(TreeUpdate {
            nodes,
            tree: Some(Tree::new(node_id(root_id))),
            tree_id: TreeId::ROOT,
            focus,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{
        Dimension, DisplayValue, DocumentEventKind, Event, PositionValue, StyleProp,
    };
    use accesskit::{Action, ActionData, ActionRequest};

    /// 縦スクロールビュー（200×100 のビューポート）。500px の高さのコンテンツ内に、
    /// ビューポートのはるか下、content-y 300 に固定した 50px の `target` を持つ。
    /// `ScrollIntoView` の検証用。`(tree, scroll, target)` を返す。
    fn scroll_into_view_scene() -> (ElementTree, ElementId, ElementId) {
        let mut tree = ElementTree::new();
        let scroll = tree.element_create(1, ElementKind::ScrollView);
        let content = tree.element_create(2, ElementKind::View);
        let target = tree.element_create(3, ElementKind::View);
        tree.set_root(scroll);
        tree.set_viewport(400.0, 400.0);
        tree.element_set_style(
            scroll,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.element_set_style(
            content,
            &[
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(500.0)),
            ],
        );
        tree.element_append_child(scroll, content);
        tree.element_set_style(
            target,
            &[
                StyleProp::Position(PositionValue::Absolute),
                StyleProp::Top(Dimension::px(300.0)),
                StyleProp::Left(Dimension::px(0.0)),
                StyleProp::Width(Dimension::px(200.0)),
                StyleProp::Height(Dimension::px(50.0)),
            ],
        );
        tree.element_append_child(content, target);
        tree.render(0.0);
        (tree, scroll, target)
    }

    fn action_request(action: Action, node: NodeId) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        }
    }

    #[test]
    fn maps_focus_action_to_core_focus_resolving_element_id() {
        let target = ElementId::from_u64(7);
        let mapped = map_action_request(&action_request(Action::Focus, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::Focus { target });
    }

    #[test]
    fn maps_click_action_to_core_click_resolving_element_id() {
        let target = ElementId::from_u64(9);
        let mapped = map_action_request(&action_request(Action::Click, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::Click { target });
    }

    #[test]
    fn maps_set_value_action_with_value_payload() {
        let target = ElementId::from_u64(11);
        let req = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node_id(target),
            data: Some(ActionData::Value("hello".into())),
        };
        assert_eq!(
            map_action_request(&req),
            AccessibilityAction::SetValue {
                target,
                value: "hello".to_string(),
            }
        );
    }

    #[test]
    fn maps_set_value_without_value_payload_to_ignored() {
        let node = node_id(ElementId::from_u64(5));
        let req = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        };
        assert_eq!(
            map_action_request(&req),
            AccessibilityAction::Ignored,
            "SetValue with no string value has nothing to set",
        );
    }

    #[test]
    fn maps_scroll_into_view_action_to_core_scroll_into_view_resolving_element_id() {
        let target = ElementId::from_u64(13);
        let mapped = map_action_request(&action_request(Action::ScrollIntoView, node_id(target)));
        assert_eq!(mapped, AccessibilityAction::ScrollIntoView { target });
    }

    #[test]
    fn folds_unsupported_actions_to_ignored() {
        let node = node_id(ElementId::from_u64(3));
        for action in [
            Action::Increment,
            Action::ShowContextMenu,
            Action::CustomAction,
        ] {
            assert_eq!(
                map_action_request(&action_request(action, node)),
                AccessibilityAction::Ignored,
                "{action:?} should fold to Ignored"
            );
        }
    }

    #[test]
    fn on_accessibility_action_focus_drives_focus_state_machine() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let a = tree.element_create(2, ElementKind::TextInput);
        let b = tree.element_create(3, ElementKind::TextInput);
        tree.element_append_child(root, a);
        tree.element_append_child(root, b);

        let la_focus = tree.register_listener(a, DocumentEventKind::Focus);
        let la_blur = tree.register_listener(a, DocumentEventKind::Blur);
        let lb_focus = tree.register_listener(b, DocumentEventKind::Focus);

        tree.on_accessibility_action(action_request(Action::Focus, node_id(a)));
        assert_eq!(tree.focused_element(), Some(a));
        let first: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(first, vec![la_focus]);

        // b へのフォーカスは、先にフォーカス中だった a を blur してから b を focus する。
        tree.on_accessibility_action(action_request(Action::Focus, node_id(b)));
        assert_eq!(tree.focused_element(), Some(b));
        let second: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert_eq!(second, vec![la_blur, lb_focus]);
    }

    fn set_value_request(node: NodeId, value: &str) -> ActionRequest {
        ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: Some(ActionData::Value(value.into())),
        }
    }

    #[test]
    fn on_accessibility_action_set_value_replaces_content_and_delivers_text_input() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let input = tree.element_create(2, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "old");

        let listener = tree.register_listener(input, DocumentEventKind::TextInput);
        tree.on_accessibility_action(set_value_request(node_id(input), "new value"));

        assert_eq!(
            tree.element_get_text_content(input),
            "new value",
            "SetValue must replace the text-input's content",
        );
        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(
            ids,
            vec![listener],
            "the replacement fires a TextInput delivery"
        );
        assert!(
            matches!(
                &deliveries[0].event,
                Event::TextInput { text, target_id } if text == "new value" && *target_id == input
            ),
            "delivered event carries the new value and targets the input",
        );
    }

    #[test]
    fn on_accessibility_action_set_value_finalizes_active_preedit_then_replaces() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let input = tree.element_create(2, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "abc");
        tree.element_set_preedit(input, "DEF"); // 進行中の IME 変換

        tree.on_accessibility_action(set_value_request(node_id(input), "xyz"));

        // preedit は置換の一部として確定され、壊れた中間状態を残さない
        // （先行事例: `element_paste` の preedit 確定）。
        assert_eq!(tree.element_get_text_content(input), "xyz");
        // 確定済みの preedit を後からクリアしても何も変わらず、変換が置換をまたいで
        // 残らなかったことを示す。
        tree.element_set_preedit(input, "");
        assert_eq!(tree.element_get_text_content(input), "xyz");
    }

    #[test]
    fn on_accessibility_action_set_value_on_non_text_input_is_noop() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let view = tree.element_create(2, ElementKind::View);
        tree.element_append_child(root, view);
        tree.register_listener(view, DocumentEventKind::TextInput);

        tree.on_accessibility_action(set_value_request(node_id(view), "nope"));
        assert!(
            tree.poll_deliveries().is_empty(),
            "SetValue on a non-text-input target must emit nothing",
        );
    }

    #[test]
    fn on_accessibility_action_ignores_unsupported_action() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let a = tree.element_create(2, ElementKind::TextInput);
        let b = tree.element_create(3, ElementKind::Button);
        tree.element_append_child(root, a);
        tree.element_append_child(root, b);
        tree.register_listener(b, DocumentEventKind::Focus);

        tree.on_accessibility_action(action_request(Action::Focus, node_id(a)));
        let _ = tree.poll_deliveries();

        // b を狙う非サポートアクションは、フォーカスを動かさず何も発行しない。
        tree.on_accessibility_action(action_request(Action::Increment, node_id(b)));
        assert_eq!(tree.focused_element(), Some(a));
        assert!(tree.poll_deliveries().is_empty());
    }

    #[test]
    fn on_accessibility_action_click_emits_bubbling_click_to_listeners() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.render(0.0);

        let l_btn = tree.register_listener(button, DocumentEventKind::Click);
        let l_root = tree.register_listener(root, DocumentEventKind::Click);

        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(ids, vec![l_btn, l_root], "Click must bubble target → root");
        assert!(
            matches!(deliveries[0].event, Event::Click { target_id, .. } if target_id == button),
            "delivered event must be a Click targeting the requested node"
        );
    }

    #[test]
    fn on_accessibility_action_click_uses_target_layout_center() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.element_set_style(
            button,
            &[
                StyleProp::Width(Dimension::px(120.0)),
                StyleProp::Height(Dimension::px(40.0)),
            ],
        );
        tree.render(0.0);

        let (rx, ry, rw, rh) = tree.element_layout_rect(button).expect("button layout");
        let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);

        tree.register_listener(button, DocumentEventKind::Click);
        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let delivery = tree.poll_deliveries().pop().expect("a click delivery");
        match delivery.event {
            Event::Click { x, y, .. } => {
                assert_eq!((x, y), (cx, cy), "click must land at the target's layout center");
            }
            other => panic!("expected a Click event, got {other:?}"),
        }
    }

    #[test]
    fn on_accessibility_action_click_does_not_flush_active_state() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.render(0.0);

        let l_active_start = tree.register_listener(button, DocumentEventKind::ActiveStart);
        let l_active_end = tree.register_listener(button, DocumentEventKind::ActiveEnd);

        tree.on_accessibility_action(action_request(Action::Click, node_id(button)));

        let fired: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert!(
            !fired.contains(&l_active_start) && !fired.contains(&l_active_end),
            "semantic click must not fire :active (ActiveStart/ActiveEnd)"
        );
        assert_eq!(
            tree.active_element(),
            None,
            "semantic click must leave no element in the :active state"
        );
    }

    #[test]
    fn on_accessibility_action_click_does_not_hit_test() {
        use crate::element::PositionValue;

        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);

        // 対象は左上に 100x100 でレイアウトされ、中心は (50, 50)。
        let target = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, target);
        tree.element_set_style(
            target,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );

        // 絶対配置のオーバーレイが対象の中心を覆うため、(50, 50) の座標ヒットテストは
        // オーバーレイに解決される。
        let overlay = tree.element_create(3, ElementKind::View);
        tree.element_append_child(root, overlay);
        tree.element_set_style(
            overlay,
            &[
                StyleProp::Position(PositionValue::Absolute),
                StyleProp::Top(Dimension::px(0.0)),
                StyleProp::Left(Dimension::px(0.0)),
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        tree.render(0.0);

        // 前提：対象中心のヒットテストはオーバーレイを選ぶため、対象へ配送されれば
        // AT 経路がヒットテストしていない証拠になる。
        assert_eq!(
            tree.hit_test(50.0, 50.0),
            Some(overlay),
            "test setup: overlay must cover the target's centre",
        );

        let l_target = tree.register_listener(target, DocumentEventKind::Click);
        let l_overlay = tree.register_listener(overlay, DocumentEventKind::Click);

        tree.on_accessibility_action(action_request(Action::Click, node_id(target)));

        let ids: Vec<_> = tree
            .poll_deliveries()
            .into_iter()
            .map(|d| d.listener_id)
            .collect();
        assert!(
            ids.contains(&l_target),
            "the AT-targeted element must receive the click"
        );
        assert!(
            !ids.contains(&l_overlay),
            "the overlay over the centre must not receive it — no hit-test runs"
        );
    }

    #[test]
    fn on_accessibility_action_click_does_not_advance_multi_click_counter() {
        fn paragraph() -> (ElementTree, ElementId) {
            let mut tree = ElementTree::new();
            let view = tree.element_create(1, ElementKind::View);
            let text = tree.element_create(2, ElementKind::Text);
            tree.set_root(view);
            tree.set_viewport(400.0, 200.0);
            tree.element_set_style(
                view,
                &[
                    StyleProp::Width(Dimension::px(400.0)),
                    StyleProp::Height(Dimension::px(200.0)),
                ],
            );
            tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
            tree.element_append_child(view, text);
            tree.element_set_text(text, "Hello world");
            tree.element_set_selectable(view, true);
            tree.render(0.0);
            (tree, text)
        }
        let (px, py) = (10.0, 8.0);
        fn range(tree: &ElementTree, text: ElementId) -> Option<(usize, usize)> {
            tree.selection().and_then(|s| s.range_within(text))
        }

        // ポインタのマルチクリック計数は循環する：同一地点 2 連打で単語選択、
        // 3 連打で行全体選択。フェーズが異なることを確かめる。
        let (mut t2, text2) = paragraph();
        t2.on_pointer_down(px, py);
        t2.on_pointer_down(px, py);
        let word = range(&t2, text2);

        let (mut t3, text3) = paragraph();
        t3.on_pointer_down(px, py);
        t3.on_pointer_down(px, py);
        t3.on_pointer_down(px, py);
        let line = range(&t3, text3);

        assert!(word.is_some() && line.is_some(), "presses must select");
        assert_ne!(word, line, "the counter must cycle word → line");

        // 実プレス 2 回の間に挟んだ意味的クリックは計数を進めてはならず、2 回目の
        // プレスは行フェーズではなく単語フェーズに留まる。
        let (mut t, text) = paragraph();
        t.on_pointer_down(px, py);
        t.on_accessibility_action(action_request(Action::Click, node_id(text)));
        t.on_pointer_down(px, py);
        assert_eq!(
            range(&t, text),
            word,
            "semantic click must not advance the multi-click counter",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_reveals_offscreen_target() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        assert_eq!(
            tree.element_get_scroll_offset(scroll),
            (0.0, 0.0),
            "precondition: scroll-view starts unscrolled",
        );

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        // 対象は 100px ビューポート内の content-y 300..350 にあり、最小スクロールは
        // その下端をビューポート下端に揃える：オフセット 250。
        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert!(
            (oy - 250.0).abs() < 0.5,
            "scroll offset must reveal the target, got {oy}",
        );

        // スクロール後、対象はビューポート内に完全に収まる。
        let (_, ty, _, th) = tree.element_layout_rect(target).expect("target layout");
        let (_, sy, _, sh) = tree.element_layout_rect(scroll).expect("scroll layout");
        let rel_top = (ty - sy) - oy;
        assert!(
            rel_top >= -0.5 && rel_top + th <= sh + 0.5,
            "target must be fully visible: rel_top={rel_top}, th={th}, sh={sh}",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_scrolls_up_to_reveal_target_above_viewport() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        // 最下部（最大オフセット 400）までスクロール：ビューポートは content 400..500 を
        // 表示し、対象（300..350）は上方の画面外になる。
        tree.element_set_scroll_offset(scroll, 0.0, 400.0);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        // 最小の上スクロールは対象の先頭（上）端をビューポート上端に揃える：オフセット 300。
        let (_, oy) = tree.element_get_scroll_offset(scroll);
        assert!(
            (oy - 300.0).abs() < 0.5,
            "scrolling up must align the target's top edge, got {oy}",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_emits_scroll_delivery_on_scroll_view() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        let listener = tree.register_listener(scroll, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        let deliveries = tree.poll_deliveries();
        let ids: Vec<_> = deliveries.iter().map(|d| d.listener_id).collect();
        assert_eq!(
            ids,
            vec![listener],
            "the offset change must fire a Scroll delivery on the scroll-view",
        );
        assert!(
            matches!(
                deliveries[0].event,
                Event::Scroll { target_id, .. } if target_id == scroll
            ),
            "delivered event must be a Scroll targeting the scroll-view",
        );
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_leaves_visible_target_untouched() {
        let (mut tree, scroll, target) = scroll_into_view_scene();
        // 事前にスクロールし、対象（content-y 300..350）がオフセット 260 で 100px
        // ビューポート内に完全に収まる状態にする（相対 40..90）。
        tree.element_set_scroll_offset(scroll, 0.0, 260.0);
        let listener = tree.register_listener(scroll, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        assert_eq!(
            tree.element_get_scroll_offset(scroll),
            (0.0, 260.0),
            "an already-visible target must not move the offset",
        );
        assert!(
            tree.poll_deliveries().is_empty(),
            "no offset change means no Scroll delivery",
        );
        let _ = listener;
    }

    #[test]
    fn on_accessibility_action_scroll_into_view_without_scroll_view_ancestor_is_noop() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let target = tree.element_create(2, ElementKind::View);
        tree.element_append_child(root, target);
        tree.render(0.0);

        let l_root = tree.register_listener(root, DocumentEventKind::Scroll);
        let l_target = tree.register_listener(target, DocumentEventKind::Scroll);

        tree.on_accessibility_action(action_request(Action::ScrollIntoView, node_id(target)));

        assert!(
            tree.poll_deliveries().is_empty(),
            "ScrollIntoView on a target with no scroll-view ancestor must do nothing",
        );
        let _ = (l_root, l_target);
    }

    #[test]
    fn accessibility_update_includes_bounds_and_roles() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);
        let button = tree.element_create(2, ElementKind::Button);
        tree.element_append_child(root, button);
        tree.element_set_aria_label(button, "Confirm");
        tree.element_set_role(button, "button");
        let input = tree.element_create(3, ElementKind::TextInput);
        tree.element_append_child(root, input);
        tree.element_set_text_content(input, "hello");
        tree.render(0.0);

        let update = tree.accessibility_update().expect("tree update");
        assert_eq!(update.tree_id, TreeId::ROOT);
        assert_eq!(update.focus, node_id(root));
        assert!(update.nodes.len() >= 3);

        let button_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(button))
            .map(|(_, n)| n)
            .expect("button node");
        assert_eq!(button_node.role(), Role::Button);
        assert_eq!(button_node.label(), Some("Confirm"));

        let input_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(input))
            .map(|(_, n)| n)
            .expect("input node");
        assert_eq!(input_node.role(), Role::TextInput);
        assert_eq!(input_node.value(), Some("hello"));
    }

    #[test]
    fn accessibility_update_does_not_drop_ifc_inline_text_subtree() {
        use std::collections::HashSet;

        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        tree.set_viewport(400.0, 300.0);
        tree.element_set_style(root, &[StyleProp::Display(DisplayValue::Flex)]);

        // IFC ルート：非テキスト親の下の `text` 要素。
        let ifc_root = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(root, ifc_root);
        tree.element_set_text(ifc_root, "Hello ");

        // インラインテキスト要素：`text` 親の下の `text` 要素で、Taffy ノードを
        // 持たない（ADR-0063/0064）。
        let inline = tree.element_create(3, ElementKind::Text);
        tree.element_append_child(ifc_root, inline);
        tree.element_set_text(inline, "world");

        tree.render(0.0);

        let update = tree.accessibility_update().expect("tree update");

        // IFC ルート自体は AccessKit ツリーに残っていなければならない。
        assert!(
            update.nodes.iter().any(|(id, _)| *id == node_id(ifc_root)),
            "IFC root subtree was dropped from the AccessKit tree"
        );

        // どのノードも、対応ノードのない子 id を参照してはならない。参照があれば
        // サブツリーの脱落を意味する。
        let node_ids: HashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
        for (_, node) in &update.nodes {
            for child in node.children() {
                assert!(
                    node_ids.contains(child),
                    "dangling child reference: {child:?}"
                );
            }
        }
    }
}
