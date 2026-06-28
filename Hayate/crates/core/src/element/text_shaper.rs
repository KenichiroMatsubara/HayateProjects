//! テキスト整形器（ADR-0123）。
//!
//! font collection（[`FontContext`]）と Parley の [`LayoutContext`] を**単独所有**し、
//! 全シェイプ経路（IFC・text-input content・UA default width・toolbar ラベル）の唯一の
//! 入口になる。`TaffyProjection`（箱）と対をなし、グリフを所有する deep module。
//! `ElementTree` の public surface は不変で、`LayoutPass` の private field として抱える
//! （ADR-0075 と同型の「内部 module 抽出・public 不変」方針）。

use std::collections::HashMap;
use std::sync::Arc;

use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext};

use crate::element::id::ElementId;
use crate::element::style::FontStyleValue;
use crate::element::text::{self, TextBrush, TextLayout};
use crate::element::tree::Element;

/// font collection と `LayoutContext` を所有し、全テキストを整形する内部 module。
pub(crate) struct TextShaper {
    font_cx: FontContext,
    layout_cx: LayoutContext<TextBrush>,
}

impl TextShaper {
    pub(crate) fn new() -> Self {
        let mut font_cx = FontContext::new();
        init_bundled_fonts(&mut font_cx);
        Self {
            font_cx,
            layout_cx: LayoutContext::new(),
        }
    }

    /// 生バイトから family を font collection に登録する（[`ElementTree::register_font`] が委譲）。
    /// 要求された名前で登録し、バンドル既定の後ろにクラスタ単位のフォールバックとして組み込む。
    ///
    /// [`ElementTree::register_font`]: crate::element::tree::ElementTree::register_font
    pub(crate) fn register_font(&mut self, family_name: &str, bytes: Arc<Vec<u8>>) {
        text::register_collection_font(&mut self.font_cx.collection, family_name, bytes);
    }

    /// フォントファイル自身に埋め込まれた family 名を使って生バイトから登録する。
    pub(crate) fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        let blob = Blob::new(Arc::new(bytes));
        self.font_cx.collection.register_fonts(blob, None);
    }

    /// `family` が font collection に登録済みなら true。名前付き `font-family` の先読み
    /// 取得判定（未登録なら `FetchFont`）に使う。
    pub(crate) fn has_family(&mut self, family: &str) -> bool {
        self.font_cx.collection.family_id(family).is_some()
    }

    /// toolbar 等の単発ラベルを整形する（ADR-0097）。
    pub(crate) fn shape_label(&mut self, text: &str, font_size: f32) -> TextLayout {
        text::build_text_layout(
            &mut self.font_cx,
            &mut self.layout_cx,
            text,
            font_size,
            None,
            None,
            None,
            None,
        )
    }

    /// IFC ルートのサブツリーを単一の Parley レイアウト＋バイト→要素マップに整形する。
    pub(crate) fn shape_ifc(
        &mut self,
        elements: &HashMap<ElementId, Element>,
        ifc_root_id: ElementId,
        max_advance: Option<f32>,
        viewport: (f32, f32),
    ) -> TextLayout {
        crate::element::inline_text::shape(
            elements,
            ifc_root_id,
            max_advance,
            &mut self.font_cx,
            &mut self.layout_cx,
            viewport,
        )
    }

    /// 単一テキスト（text-input のコンテンツ/プレースホルダ）を整形する。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn shape_text(
        &mut self,
        text: &str,
        font_size: f32,
        max_advance: Option<f32>,
        font_family: Option<&str>,
        font_weight: Option<f32>,
        font_style: Option<FontStyleValue>,
    ) -> TextLayout {
        text::build_text_layout(
            &mut self.font_cx,
            &mut self.layout_cx,
            text,
            font_size,
            max_advance,
            font_family,
            font_weight,
            font_style,
        )
    }

    /// 幅未指定 `text-input` の UA 既定コンテンツ幅（ADR-0109）。フィールド自身のテキストに
    /// 依存しない、フォント相対の固有コンテンツ幅。
    pub(crate) fn text_input_default_width(
        &mut self,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<f32>,
        font_style: Option<FontStyleValue>,
    ) -> f32 {
        text::text_input_default_width(
            &mut self.font_cx,
            &mut self.layout_cx,
            font_size,
            font_family,
            font_weight,
            font_style,
        )
    }

    /// テスト用シーム（ADR-0042）。WASM ランタイムを模してフォントコレクションを再構築する。
    /// system_fonts なし、`default_font` をデフォルト family ＋ sans-serif generic として登録する。
    /// ホスト導入フォントに依存せず `.notdef → FetchFont → register_font` の実経路をテストできる。
    pub(crate) fn set_wasm_like_font_context(&mut self, default_font: Vec<u8>) {
        use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
        self.font_cx.collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        let blob = Blob::new(Arc::new(default_font));
        let override_info = FontInfoOverride {
            family_name: Some(text::DEFAULT_FONT_FAMILY),
            ..Default::default()
        };
        let registered = self.font_cx.collection.register_fonts(blob, Some(override_info));
        let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        if !ids.is_empty() {
            self.font_cx
                .collection
                .set_generic_families(GenericFamily::SansSerif, ids.into_iter());
        }
    }
}

impl Default for TextShaper {
    fn default() -> Self {
        Self::new()
    }
}

fn init_bundled_fonts(font_cx: &mut FontContext) {
    use fontique::{FontInfoOverride, GenericFamily};

    static NOTO_SANS_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSansJP.ttf");

    let blob = Blob::new(Arc::new(NOTO_SANS_BYTES));
    let override_info = FontInfoOverride {
        family_name: Some(text::DEFAULT_FONT_FAMILY),
        ..Default::default()
    };
    let registered = font_cx.collection.register_fonts(blob, Some(override_info));
    let family_ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
    if !family_ids.is_empty() {
        font_cx
            .collection
            .set_generic_families(GenericFamily::SansSerif, family_ids.into_iter());
    }
}
