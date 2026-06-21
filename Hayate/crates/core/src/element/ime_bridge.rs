/// IME 候補ウィンドウ配置用の、スクリーン空間における文字境界（ADR-0069）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// プラットフォーム IME がこのフレームで提示すべき内容。編集可能性から core
/// （[`ElementTree::drive_ime`](crate::ElementTree::drive_ime)）が一度だけ算出する。
/// アダプタはこれを反映するのみで、ソフトキーボードの表示判定をプラットフォーム
/// 個別に再導出しない。ゲートの修正は全プラットフォームに一度で効く。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImePresentation {
    /// 編集可能な要素がフォーカスされていない。アダプタはソフトキーボードを閉じる。
    /// タップは当たった要素を何でもフォーカスする（ボタン・プレーンテキスト・ビュー
    /// — Chromium 互換、ADR-0102）が、編集可能なのは `text-input` だけなので、
    /// プレーンなタップはここに来てキーボードを上げてはならない。
    Hidden,
    /// `text-input` がフォーカスされている。アダプタはソフトキーボードを表示し、
    /// IME 候補ウィンドウを `bounds` に向ける。
    Shown { bounds: CharacterBounds },
}

/// プラットフォーム IME 配線のシーム（ADR-0069）。アダプタは EditContext（web）/
/// GameTextInput（Android）/ TSF / TSM / IBus をラップし、[`ImePresentation`] を
/// 反映する以外のことはしない。編集可能性の判断 — キーボードを*出すか*、候補
/// ウィンドウを*どこに*置くか — は core に置く。この判断をアダプタから締め出す
/// ことが、プラットフォーム個別の振る舞い乖離を防ぐ。
pub trait ImeBridge {
    fn present(&mut self, presentation: ImePresentation);
}
