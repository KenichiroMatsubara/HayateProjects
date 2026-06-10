# タッチ対応は Pointer Events への統一で行う

**Status: accepted**

現状 `attachPointerInput`（Web Adapter, ADR-0080により将来 `hayate-adapter-web` 自己配線へ移行）は `mousemove`/`mousedown`/`mouseup`/`wheel` のみを購読しており、タッチドラッグによるスクロールは一切処理されない。ADR-0046は「タッチジェスチャーの受信とスクロール物理（慣性・rubber-band）はPlatform Adapterの責務」と定義しているが未実装(open)である。

タッチ対応にあたり、Mouse Events群はそのまま残しTouch Eventsを別途追加する案と、Pointer Events（`pointerdown`/`pointermove`/`pointerup`/`pointercancel`、`event.pointerType`でmouse/touch/pen判別）に統一する案を比較し、**Pointer Events統一**を採用する。Mouse EventsとTouch Eventsを併用すると同一ジェスチャーに対し両方が発火する二重発火問題への対処が必要になり、Core側の `on_pointer_*` という命名（既にmouse/touch非依存）とも整合しないため。

タッチドラッグ→スクロールオフセット変換とスクロール物理（慣性・rubber-band）は ADR-0046 通り Platform Adapter（`hayate-adapter-web`）が担い、`element_set_scroll_offset`（SCR-02, プログラマティック専用API）経由でCoreに反映する。`touch-action: none` とPointer Captureの設定が必要。

## Consequences

- `mousemove`/`mousedown`/`mouseup`/`wheel` の購読は `pointerdown`/`pointermove`/`pointerup`/`pointercancel` + 既存`wheel`に置き換わる（ADR-0080の自己配線実装と合わせて移行）。
- ADR-0046で"open"だったタッチジェスチャー＋スクロール物理（慣性・rubber-band）の実装が必要になる。
- `on_pointer_down/up/move` のCore側シグネチャ自体は変更不要（座標ベースで既にpointerType非依存）。
