//! App Host — platform 非依存の最上位協調層であり mount 先（ADR-0117）。
//!
//! App Host は `ElementTree` 実体を所有し、フレームループ（[`AppHost::tick`]）・
//! `Event Delivery` の drain・描画オーケストレーションを担う。OS フレームループ自体は
//! 所有せず、Platform Front（web `requestAnimationFrame` / Android `Choreographer`）が
//! 毎フレーム [`AppHost::tick`] を呼ぶ。フレームを起こす唯一の入口は構築時に受け取る
//! `request_redraw` クロージャで、`tick` 末尾に pending visual work（進行中 transition 等）が
//! 残るときだけ次フレームを要求し、無ければ idle に落ちる。
//!
//! consumer（in-process の Hayabusa / wire の Tsubame Canvas Renderer）は [`DeliverySink`]
//! を mount 時に渡す。App Host は drain を所有し続け、`tick` で drain した delivery バッチ
//! （空のこともある）を毎フレーム sink へ同期 push する。`ListenerId → handler` の解決と
//! reactive flush は consumer 側の責務で、App Host は consumer 非依存を保つ。
//!
//! このクレートは ADR-0068 の Render Host を共有層へ hoist する作業に先行する headless な
//! 骨組みで、描画の present は [`Surface`] trait の裏に置く（テストは no-op 実装を渡す）。

use hayate_core::{ElementTree, EventDelivery, SceneGraph};

mod font_mailbox;
pub mod renderer_selection;

pub use font_mailbox::{FontFetchResult, FontMailbox, FontMailboxHandle};

/// 1 フレーム分の `SceneGraph` の提示先。Render Host（ADR-0068）の present サーフェスを
/// App Host から見た最小 seam。headless・テストでは no-op 実装を渡す。
pub trait Surface {
    /// 本フレームの scene graph を提示する。
    fn present(&mut self, scene: &SceneGraph);
}

/// `()` を提示先とする no-op `Surface`。headless 実行やテストで使う。
pub struct HeadlessSurface;

impl Surface for HeadlessSurface {
    fn present(&mut self, _scene: &SceneGraph) {}
}

/// consumer が mount 時に渡す push 型 delivery 受け口（ADR-0117）。
///
/// App Host は drain を所有し、drain した delivery バッチ（空のこともある）を毎フレーム
/// [`DeliverySink::handle`] へ同期 push する。consumer は受け取った delivery の handler を
/// 実行し、handler 由来・非同期由来を問わず reactive graph を flush して、結果の Element
/// Layer mutation を `tree` へ発行し、**return する前にそのフレーム分を出し切る**。
pub trait DeliverySink {
    /// drain 済みの delivery バッチを処理し、結果の mutation を `tree` へ発行する。
    /// `deliveries` は空のこともある（毎フレーム無条件に呼ばれる）。
    fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree);
}

/// platform 非依存の最上位協調層。`ElementTree` 実体を所有し、`tick` でフレームを進める。
pub struct AppHost<S: Surface> {
    tree: ElementTree,
    surface: S,
    request_redraw: Box<dyn Fn()>,
    sink: Option<Box<dyn DeliverySink>>,
    font_mailbox: FontMailbox,
}

impl<S: Surface> AppHost<S> {
    /// 空の `ElementTree` で App Host を構築する。`request_redraw` は Platform Front が
    /// 供給するフレーム要求クロージャ（唯一の wake 入口・ADR-0117）。
    pub fn new(surface: S, request_redraw: Box<dyn Fn()>) -> Self {
        Self {
            tree: ElementTree::new(),
            surface,
            request_redraw,
            sink: None,
            font_mailbox: FontMailbox::new(),
        }
    }

    /// アダプタの `impl FontFetcher`（ADR-0132 スライス2）へ渡す、フォント取得完了
    /// 報告用の clone 可能なハンドル。非同期取得クロージャがここへ結果を push し、
    /// `tick` が毎フレーム layout より前に drain する。
    pub fn font_mailbox_handle(&self) -> FontMailboxHandle {
        self.font_mailbox.handle()
    }

    /// consumer が tree を組み立てるための可変参照。App Host が tree を所有するので、
    /// consumer は mount 前にここで element を作り、listener を登録する。
    pub fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    /// 所有する `ElementTree` への不変参照。
    pub fn tree(&self) -> &ElementTree {
        &self.tree
    }

    /// 所有する `Surface` への可変参照。`tree_mut` と対称の seam。App Host が surface を
    /// 所有しても、Platform Front は resize 時に present サーフェス（wgpu surface 等）を
    /// 再 configure する必要があるため、ここで具体型 `S` へ可変アクセスする。
    pub fn surface_mut(&mut self) -> &mut S {
        &mut self.surface
    }

    /// consumer の [`DeliverySink`] を登録する（mount）。
    pub fn mount(&mut self, sink: Box<dyn DeliverySink>) {
        self.sink = Some(sink);
    }

    /// 1 フレーム進める。Platform Front が毎フレーム呼ぶ（ADR-0117 のフェーズ順）。
    ///
    /// 0. **font mailbox drain**：アダプタが非同期取得を完了した font を、layout（3+4）
    ///    より前に `tree.register_font` / `tree.font_fetch_failed` へ流し込む（ADR-0132
    ///    スライス2、「フォント登録は layout より前」という順序不変条件）。
    /// 1. **drain**：App Host が `poll_deliveries()` を drain する（delivery 所有）。
    /// 2. **advance**：DeliverySink を毎フレーム無条件に呼ぶ（空 batch でも）。consumer が
    ///    handler 実行＋reactive flush＋mutation 発行を return 前に済ます。
    /// 3+4. **commit_frame ＋ render**：`ElementTree::render` が内部で `commit_frame()` を
    ///    呼ぶので（layout settling → scene lowering）、App Host は `render` 一回で両フェーズを
    ///    回し、得た scene を `Surface` へ present する。
    /// 5. **再要求判定**：pending visual work（進行中 transition 等）が残れば `request_redraw`。
    pub fn tick(&mut self, timestamp_ms: f64) {
        // 0. font mailbox drain — layout より前。
        for result in self.font_mailbox.drain() {
            match result {
                FontFetchResult::Loaded { family, bytes } => self.tree.register_font(&family, bytes),
                FontFetchResult::Failed { family } => {
                    self.tree.font_fetch_failed(&family);
                }
            }
        }

        // 1. drain — poll_deliveries は所有 Vec を返すので以降 tree は再借用できる。
        let deliveries = self.tree.poll_deliveries();

        // 2. advance — sink へ同期 push（disjoint なフィールド借用）。
        if let Some(sink) = self.sink.as_mut() {
            sink.handle(&deliveries, &mut self.tree);
        }

        // 3+4. commit_frame（render 内）→ scene lowering → present。
        let scene = self.tree.render(timestamp_ms);
        self.surface.present(scene);

        // 5. 再要求判定 — 残っていれば次フレームを要求し、無ければ idle。
        if self.tree.has_pending_visual_work() {
            (self.request_redraw)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{DocumentEventKind, ElementKind, Event};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// present 回数を数える `Surface`。
    struct CountingSurface {
        present_count: Rc<RefCell<u32>>,
    }

    impl Surface for CountingSurface {
        fn present(&mut self, _scene: &SceneGraph) {
            *self.present_count.borrow_mut() += 1;
        }
    }

    /// 可変フィールドを持つ最小 `Surface`。Platform Front の resize 経路を模す。
    struct ReconfigurableSurface {
        configured_size: (u32, u32),
    }

    impl Surface for ReconfigurableSurface {
        fn present(&mut self, _scene: &SceneGraph) {}
    }

    #[test]
    fn surface_mut_exposes_owned_surface_for_platform_front_resize() {
        // App Host が surface を所有しても、Platform Front は resize で wgpu surface を
        // 再 configure するため可変アクセスが要る（`tree_mut` と対称の seam）。
        let mut app = AppHost::new(
            ReconfigurableSurface {
                configured_size: (0, 0),
            },
            Box::new(|| {}),
        );
        app.surface_mut().configured_size = (1920, 1080);
        assert_eq!(app.surface_mut().configured_size, (1920, 1080));
    }

    /// テスト用の consumer sink。受け取った delivery バッチを記録し、Click delivery を
    /// 受けたら（mutation 発行の代理として）登録済みの text 要素へテキストを書き込む。
    struct RecordingSink {
        /// 各 `handle` 呼び出しで受け取った delivery 件数（空フレームも 0 として記録）。
        batches: Rc<RefCell<Vec<usize>>>,
        /// Click を受けたら text を書き込む対象。
        text_target: hayate_core::ElementId,
    }

    impl DeliverySink for RecordingSink {
        fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree) {
            self.batches.borrow_mut().push(deliveries.len());
            for d in deliveries {
                if matches!(d.event, Event::Click { .. }) {
                    tree.element_set_text(self.text_target, "clicked");
                }
            }
        }
    }

    /// root view に button と text 子を持つ最小ツリーを組み、button へ Click listener を登録する。
    /// 戻り値は (button, text)。
    fn build_min_tree(tree: &mut ElementTree) -> (hayate_core::ElementId, hayate_core::ElementId) {
        let root = tree.element_create(0, ElementKind::View);
        let button = tree.element_create(1, ElementKind::Button);
        let text = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(root, button);
        tree.element_append_child(root, text);
        tree.set_root(root);
        tree.register_listener(button, DocumentEventKind::Click);
        (button, text)
    }

    #[test]
    fn tick_drains_delivery_and_sink_mutates_tree() {
        let present_count = Rc::new(RefCell::new(0));
        let surface = CountingSurface {
            present_count: present_count.clone(),
        };
        let mut app = AppHost::new(surface, Box::new(|| {}));

        let (button, text) = build_min_tree(app.tree_mut());

        let batches = Rc::new(RefCell::new(Vec::new()));
        app.mount(Box::new(RecordingSink {
            batches: batches.clone(),
            text_target: text,
        }));

        // Click を合成 → bubble dispatch で button の listener に delivery が積まれる。
        app.tree_mut().dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: button,
                x: 0.0,
                y: 0.0,
            },
        );

        app.tick(16.0);

        // sink は 1 件の delivery を受け取り、tree へ mutation を発行した。
        assert_eq!(*batches.borrow(), vec![1]);
        assert_eq!(app.tree().element_get_text(text), "clicked");
        // present は毎 tick 1 回。
        assert_eq!(*present_count.borrow(), 1);
    }

    #[test]
    fn sink_called_every_tick_even_without_deliveries() {
        let surface = HeadlessSurface;
        let mut app = AppHost::new(surface, Box::new(|| {}));
        let (_button, text) = build_min_tree(app.tree_mut());

        let batches = Rc::new(RefCell::new(Vec::new()));
        app.mount(Box::new(RecordingSink {
            batches: batches.clone(),
            text_target: text,
        }));

        // delivery を一切積まずに 3 フレーム回す。
        app.tick(16.0);
        app.tick(32.0);
        app.tick(48.0);

        // 毎フレーム空 batch で sink が呼ばれている（ADR-0117「毎 tick flush」）。
        assert_eq!(*batches.borrow(), vec![0, 0, 0]);
    }

    #[test]
    fn idle_when_no_pending_visual_work_does_not_request_redraw() {
        let redraws = Rc::new(RefCell::new(0));
        let r = redraws.clone();
        let surface = HeadlessSurface;
        let mut app = AppHost::new(surface, Box::new(move || *r.borrow_mut() += 1));
        let (_button, _text) = build_min_tree(app.tree_mut());
        app.mount(Box::new(RecordingSink {
            batches: Rc::new(RefCell::new(Vec::new())),
            text_target: _text,
        }));

        // transition も blink も無い静的ツリー：render 後に pending visual work は残らない。
        app.tick(16.0);

        assert_eq!(*redraws.borrow(), 0, "静止フレームでは request_redraw を出さない");
    }

    /// WASM 相当のバンドル代役（CI 常設の DejaVu Sans、Latin のみ）。
    fn latin_only_default() -> Vec<u8> {
        std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .expect("DejaVuSans.ttf present for the test")
    }

    static NOTO_SANS_JP: &[u8] = include_bytes!("../../core/assets/fonts/NotoSansJP.ttf");

    /// Latin のみのバンドル上に日本語 Text を 1 つ置く。戻り値は (app, label)。
    fn app_with_missing_cjk_font() -> (AppHost<HeadlessSurface>, hayate_core::ElementId) {
        let mut app = AppHost::new(HeadlessSurface, Box::new(|| {}));
        let root = app.tree_mut().element_create(0, ElementKind::View);
        let label = app.tree_mut().element_create(1, ElementKind::Text);
        app.tree_mut().element_append_child(root, label);
        app.tree_mut().set_root(root);
        app.tree_mut().set_viewport(400.0, 300.0);
        app.tree_mut().test_set_wasm_like_fonts(latin_only_default());
        app.tree_mut().element_set_text(label, "あ");
        app.tree_mut().element_set_font_family(label, "Noto Sans JP");
        (app, label)
    }

    #[test]
    fn tick_drains_mailbox_and_registers_the_font_before_layout() {
        // ADR-0132 スライス2: mailbox に積まれた成功報告は、その tick の layout より前に
        // `register_font` されるので、同じフレームで正しいグリフにシェイプされる。
        let (mut app, label) = app_with_missing_cjk_font();

        app.tick(0.0);
        assert!(
            app.tree().test_element_glyph_ids(label).iter().any(|&id| id == 0),
            "font がまだ届く前は .notdef（tofu）のはず"
        );

        app.font_mailbox_handle()
            .report_loaded("Noto Sans JP".to_string(), NOTO_SANS_JP.to_vec());
        app.tick(16.0);

        let glyphs = app.tree().test_element_glyph_ids(label);
        assert!(!glyphs.is_empty());
        assert!(
            glyphs.iter().all(|&id| id != 0),
            "tick は mailbox を layout より前に drain して font を登録しなければならない: {glyphs:?}"
        );
    }

    #[test]
    fn tick_drains_mailbox_failures_so_core_retries_on_a_later_frame() {
        let (mut app, _label) = app_with_missing_cjk_font();

        app.tick(0.0);
        let first: Vec<String> = app
            .tree_mut()
            .poll_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::FetchFont { family } => Some(family),
                _ => None,
            })
            .collect();
        assert_eq!(first, vec!["Noto Sans JP".to_string()]);

        app.font_mailbox_handle()
            .report_failed("Noto Sans JP".to_string());
        app.tick(16.0);

        let retried: Vec<String> = app
            .tree_mut()
            .poll_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::FetchFont { family } => Some(family),
                _ => None,
            })
            .collect();
        assert_eq!(
            retried,
            vec!["Noto Sans JP".to_string()],
            "mailbox の失敗報告は core へ伝わり、再要求されなければならない"
        );
    }
}
