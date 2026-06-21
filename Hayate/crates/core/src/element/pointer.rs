//! 物理ポインタデバイス軸（ADR-0104）。`PointerKind` はポインタ操作の背後にあるデバイス
//! （マウス・タッチ・ペン）を区別する。Platform Adapter の `PointerEvent.pointerType` から
//! `on_pointer_down` / `on_pointer_move` / `on_pointer_up` を経て Core に渡され、Core は操作ごとに
//! 直近の種別を保持する（`last_pointer_kind`）。pointer proto/wire 契約に乗る。
//!
//! `:focus-visible` を駆動する Pointer/Keyboard 軸 [`InputModality`](super::interaction::InputModality)
//! とは直交する。タッチ押下とマウス押下はどちらも `InputModality::Pointer` だが `PointerKind` は
//! 異なる。2 軸は共存し、決して混同しない。

/// ポインタ操作を生んだ物理デバイス。Platform Adapter 境界で DOM `PointerEvent.pointerType`
/// から変換され、pointer wire イベントに乗せて伝搬する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

impl PointerKind {
    /// DOM `PointerEvent.pointerType` 文字列を変換する。未知の値（一部エンジンが報告する
    /// 空文字を含む）は `Mouse` にフォールバックし、touch/pen ジェスチャを乗っ取らず
    /// マウスの選択/ドラッグパスを維持する。
    pub fn from_dom(value: &str) -> Self {
        match value {
            "touch" => PointerKind::Touch,
            "pen" => PointerKind::Pen,
            _ => PointerKind::Mouse,
        }
    }

    /// pointer proto/wire 契約の wire 判別子（`mouse=0`, `touch=1`, `pen=2`）。
    /// [`from_u32`](Self::from_u32) と対をなす。
    pub fn to_u32(self) -> u32 {
        match self {
            PointerKind::Mouse => 0,
            PointerKind::Touch => 1,
            PointerKind::Pen => 2,
        }
    }

    /// [`to_u32`](Self::to_u32) の逆変換。未知の判別子は `Mouse` にフォールバックする
    /// （`from_dom` が未知種別に使うのと同じ安全なデフォルト）。
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => PointerKind::Touch,
            2 => PointerKind::Pen,
            _ => PointerKind::Mouse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dom_pointer_type_strings_and_defaults_to_mouse() {
        assert_eq!(PointerKind::from_dom("mouse"), PointerKind::Mouse);
        assert_eq!(PointerKind::from_dom("touch"), PointerKind::Touch);
        assert_eq!(PointerKind::from_dom("pen"), PointerKind::Pen);
        // 未知 / 空の pointerType はマウスパスを維持する。
        assert_eq!(PointerKind::from_dom(""), PointerKind::Mouse);
        assert_eq!(PointerKind::from_dom("eraser"), PointerKind::Mouse);
    }

    #[test]
    fn wire_discriminant_round_trips() {
        for kind in [PointerKind::Mouse, PointerKind::Touch, PointerKind::Pen] {
            assert_eq!(PointerKind::from_u32(kind.to_u32()), kind);
        }
        // 固定された wire 値。proto/wire 契約は安定でなければならない。
        assert_eq!(PointerKind::Mouse.to_u32(), 0);
        assert_eq!(PointerKind::Touch.to_u32(), 1);
        assert_eq!(PointerKind::Pen.to_u32(), 2);
        // 範囲外の判別子は安全なデフォルトにデコードされる。
        assert_eq!(PointerKind::from_u32(99), PointerKind::Mouse);
    }
}
