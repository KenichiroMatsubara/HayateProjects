//! `render()` フレームの「保留中の render_scale を適用してから present する」手順（#666）。
//!
//! DOM/GPU に依存しない純粋なモジュール（`image_decode` と同じ理由でホストでテストできる）。
//! `canvas.rs` の `HayateElementRenderer` が [`FrameSurface`] を実装し、この [`advance_frame`]
//! を呼ぶことで resize→present の順序を型で固定する。`canvas.set_width`/`set_height` は HTML5
//! 仕様で即座にバッファをクリアするため、present の後に resize すると直前に描画した内容がそのまま
//! 消え、次の `render()` まで canvas が空白になる。

/// 保留中の render_scale 適用と present の継ぎ目。`Present`（present の戻り値型）を関連型にして
/// おくことで、実装（`canvas.rs`）は `Result<(), JsValue>` を返しつつ、このモジュール自体は
/// wasm_bindgen に依存しない。
pub(crate) trait FrameSurface {
    type Present;

    fn apply_render_scale(&mut self, render_scale: f32);
    fn present(&mut self) -> Self::Present;
}

/// 保留中の render_scale（前フレームで測ったスケール変更、あれば）を、このフレームの present
/// より前に適用してから present する。resize→present の順序をここに固定することで、呼び出し側
/// （`render()`）の呼び出し順が将来入れ替わって回帰することを防ぐ。
pub(crate) fn advance_frame<S: FrameSurface>(
    surface: &mut S,
    pending_render_scale: Option<f32>,
) -> S::Present {
    if let Some(render_scale) = pending_render_scale {
        surface.apply_render_scale(render_scale);
    }
    surface.present()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeSurface {
        log: Vec<&'static str>,
        applied_scale: Option<f32>,
    }

    impl FrameSurface for FakeSurface {
        type Present = ();

        fn apply_render_scale(&mut self, render_scale: f32) {
            self.log.push("resize");
            self.applied_scale = Some(render_scale);
        }

        fn present(&mut self) {
            self.log.push("present");
        }
    }

    #[test]
    fn resize_happens_before_present_when_a_render_scale_change_is_pending() {
        let mut surface = FakeSurface::default();
        advance_frame(&mut surface, Some(0.7));
        assert_eq!(surface.log, vec!["resize", "present"]);
        assert_eq!(surface.applied_scale, Some(0.7));
    }

    #[test]
    fn present_runs_alone_when_no_render_scale_change_is_pending() {
        let mut surface = FakeSurface::default();
        advance_frame(&mut surface, None);
        assert_eq!(surface.log, vec!["present"]);
    }
}
