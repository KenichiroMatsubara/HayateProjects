# skia-safe を Native Scene Renderer の既定にする

**Status: accepted**

**Date: 2026-07-14**

OPPO A101OP（Adreno 620）と Nothing Phone (3a)（Adreno 810）で、通常の SolidJS TODO / CSS
Gallery を skia-safe raster と Ganesh/EGL GL の両方で実機検証し、いずれも描画異常なく動作
した（[issue #804 調査記録](../investigations/2026-07-14-android-skia-safe-real-device-default.md)）。
ADR-0147 が skia-safe を Android の主力へ格上げした判断を issue #819 で既定値へ反映し、Native Renderer
Selection Order を **skia-safe → vello** にする。Android の skia surface は既存の **GL** を
既定に据え、GL 初期化失敗時だけ raster、Skia 自体の初期化失敗時だけ vello を boot 中の次候補
として試す。明示指定による vello / raster の比較口は残し、Web の選択順序は変更しない。

## Consequences

- `NATIVE_RENDERER_ORDER` は desktop / Android 共通で `Skia, Vello` になる。
- Android は指定なしで skia-safe + Ganesh/EGL GL を選ぶ。
- ADR-0146 の「vello preferred / skia alternative」という暫定順序と、ADR-0147 の
  「既定は issue #804 で確定する」という保留を本 ADR が更新する。
- 選択後の renderer 障害は別 renderer へ runtime fallback せず、ADR-0148 の terminal failure
  方針を維持する。
