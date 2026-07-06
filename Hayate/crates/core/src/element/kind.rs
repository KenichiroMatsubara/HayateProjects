use crate::element::style::{CursorValue, UserSelectValue};

/// `proto/spec/element_kinds.json` から生成される要素種別テーブル。種別ごとの UA デフォルト
/// の唯一の出典（ADR-0105/ADR-0108）。生成モジュールの `super::` スコープに `ElementKind`・
/// `CursorValue`・`UserSelectValue` を取り込む。
mod tables {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/element_kind_tables.rs"
    ));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    View,
    Text,
    Image,
    Button,
    TextInput,
    ScrollView,
}

impl ElementKind {
    /// 明示的な `cursor` がないときのこの種別の UA デフォルトカーソル（ADR-0105）:
    /// `button` → pointer、`text-input` → text（I-beam）、その他 → default。
    /// `proto/spec/element_kinds.json` 由来で、Canvas と DOM が同一テーブルを共有し、
    /// どちらのレンダラも再宣言しない。
    pub fn default_cursor(self) -> CursorValue {
        tables::default_cursor(self)
    }

    /// この種別の UA デフォルト*レイアウト*。要素生成時の基底 `taffy::Style` として使い、
    /// 後から適用される明示プロパティ（`element_set_style`）が上に重なる。解決順は
    /// `default_cursor` と同じ: 明示 > 種別デフォルト > Taffy デフォルト（ADR-0109）。
    ///
    /// `button` はブラウザの `<button>` を模す: クロス軸で中央寄せ（`align-items: center`、縦）、
    /// メイン軸で左寄せ（`justify-content: flex-start`、横）。横を flex-start に保つのは意図的で、
    /// 中央寄せにすると左寄せのボタンラベル（例: todo の行）が崩れ、DOM の `text-align: inherit`
    /// と乖離するため。横中央寄せが欲しいボタンは `justify-content: center` を明示する（ADR-0109）。
    /// 他の種別は素の Taffy デフォルトのまま。
    ///
    /// `default_cursor` と違い `element_kinds.json` 由来ではない: 種別のレイアウトデフォルトは
    /// TS/DOM 側に消費者のいない Taffy-`Style` の関心事（DOM は `<button>` の中央寄せをブラウザ
    /// UA から無償で得る）で、共生成するものがないため `enum` ローカルのデフォルトとする（ADR-0109）。
    pub fn base_layout_style(self) -> taffy::Style {
        match self {
            Self::Button => taffy::Style {
                align_items: Some(taffy::AlignItems::Center),
                justify_content: Some(taffy::JustifyContent::FlexStart),
                ..taffy::Style::default()
            },
            // scroll-view 種別の UA デフォルト: CSS スクロールコンテナであり、DOM レンダラが
            // scroll-view に `overflow: auto` タグデフォルトを与えるのに対応する Canvas 側。
            // スクロールコンテナの flex `min-{width,height}: auto` 自動最小サイズはコンテンツ
            // サイズではなく 0 に解決される。よって `height: 100%`（や `flex-grow: 1`）の
            // scroll-view は、コンテンツ分だけ親をはみ出すのではなく兄弟が残した領域に縮む。
            // はみ出すと、膨らんだボックス高がスクロールビューポート（`element_scroll_max_offset`）
            // でもあるため、その固定帯のコンテンツが到達不能になる。
            // 他の種別の明示 `overflow` プロパティも `apply_overflow_to_style` で同じ経路を通る。
            // これは種別デフォルト（scroll-view に `overflow` プロパティは設定しない）。
            // デフォルト `scrollbar_width: 0` の `Scroll` は領域を確保せず `Hidden` のように
            // レイアウトされる。クリップ/スクロール機構は scene_build / canvas.rs にある。
            Self::ScrollView => taffy::Style {
                overflow: taffy::Point {
                    x: taffy::Overflow::Scroll,
                    y: taffy::Overflow::Scroll,
                },
                ..taffy::Style::default()
            },
            _ => taffy::Style::default(),
        }
    }

    /// 明示的な `user-select` がないときのこの種別の UA デフォルト（ADR-0108）:
    /// `view` / `text` / `scroll-view` / `text-input` は選択可（`Text`）、
    /// `image` / `button` は不可（`None`）。`proto/spec/element_kinds.json` 由来で、
    /// Canvas と DOM が同一テーブルを共有し、どちらのレンダラも再宣言しない。
    pub fn default_user_select(self) -> UserSelectValue {
        tables::default_user_select(self)
    }

    /// この種別がテキスト入力を受け付け、フォーカス時にプラットフォームのソフトキーボード/
    /// IME を出すべきか。`text-input` のみ `true`。素の `text` はスタイルを運ぶ
    /// （Text-Local Carrier）が編集不可。`proto/spec/element_kinds.json` 由来で、各アダプタが
    /// プラットフォームごとに「テキストフィールドか」を再導出せず同一テーブルを共有する。
    pub fn accepts_text_input(self) -> bool {
        tables::accepts_text_input(self)
    }

    /// この種別が `draw` display list property を運ぶか（Draw Carrier・#724 /
    /// ADR-0141）。`view` のみ `true`。carrier 以外への draw は no-op
    /// （carriesTextLocal と同じ carrier 文化）。`proto/spec/element_kinds.json` 由来。
    pub fn carries_draw(self) -> bool {
        tables::carries_draw(self)
    }
}

impl ElementKind {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::View),
            1 => Some(Self::Text),
            2 => Some(Self::Image),
            3 => Some(Self::Button),
            4 => Some(Self::TextInput),
            5 => Some(Self::ScrollView),
            _ => None,
        }
    }

    pub fn is_text_like(self) -> bool {
        matches!(self, Self::Text | Self::Button | Self::TextInput)
    }
}
