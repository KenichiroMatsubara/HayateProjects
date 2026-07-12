# Android ネイティブ描画で vello/wgpu 経路を Adreno で放棄し skia-safe を採用する

**Status: accepted**

**Date: 2026-07-12**

## Context

Nothing Phone 3a（実 GPU = **Adreno 810**。[ADR-0145](0145-android-vello-aa-and-wgpu-backend-runtime-switch.md) / [ADR-0146](0146-skia-safe-native-scene-renderer.md) の「710」表記は誤り）で、複雑なシーン（Tsubame Task Studio の「CSS Gallery」ページ）の**パス描画（角丸・シャドウ）が破綻**する。単純なページ（Tasks）は正常。同一端末の Chrome（WebGPU / Dawn）では vello が正常描画する（ADR-0145 で検証済み）。

ADR-0145 は「容疑は wgpu-native の **Vulkan 経路** × Adreno ドライバ」と絞り、再ビルド不要の切替スイッチ（backend Vulkan/GL × AA Area/MSAA8/MSAA16）を入れ、恒久対策の確定を実機実験（issue #796）に委ねていた。ADR-0146 は保険として skia-safe 導入を決めていた（実装は未着手・凍結）。

2026-07-12、NP3a が物理接続で adb 到達可能になり、**バグ本体の上で総当たり実機検証**を実施した。詳細な一次証拠と推論の全記録は別ファイル **[docs/investigations/2026-07-12-vello-wgpu-adreno-breakage.md](../investigations/2026-07-12-vello-wgpu-adreno-breakage.md)**（スクリーンショット付き）に残す。判明した事実:

1. **バグは backend 非依存。** GL に切り替えても Adreno 810 では Vulkan とピクセル同一に破綻する（Adreno 620 では GL は `max_storage_buffers_per_shader_stage 8>4` で device 生成すら不能）。→ ADR-0145 の「Vulkan 経路が犯人」仮説は**誤り**。
2. **AA 切替は救済にならない。** MSAA8/16 は NP3a で画面全体を砂嵐状に総崩れさせ、Area より悪化する。
3. **シェーダ変換（Naga）は犯人ではない。** Dawn と同じ **Tint** で vello の WGSL を SPIR-V 化し、wgpu の `create_shader_module_passthrough`（`PASSTHROUGH_SHADERS`）で Naga を迂回して流し込んでも、破綻は不変（20/20 shader が passthrough 経由・Android はパイプラインキャッシュ無しで確実に実行、OPPO では回帰なし）。Dawn は同じ Tint SPIR-V を同じドライバに渡して正常なので、**犯人は wgpu-hal の Vulkan 駆動（バリア/同期/メモリ扱い）**——wgpu の Vulkan・GL 両バックエンドが共有し、Dawn とは異なる層——に確定的に絞られた。
4. **upstream に修正はない。** wgpu v30 CHANGELOG に該当修正なし。既知の Adreno issue（[#5318](https://github.com/gfx-rs/wgpu/issues/5318) u32 上位bit破損＝未修正のドライババグ、[#7445](https://github.com/gfx-rs/wgpu/issues/7445) closed/not-planned）も救済にならない。blind な版上げは vello 一式の再 vendoring を伴い高コスト・低期待値。

要するに、**vello の枠内で（切替でも少改修でも版上げでも）直せる構成は存在せず、根治は wgpu-hal 上流（Dawn との差分実装）の不確実な深掘りにしかない**。

## Decision

**Android ネイティブ描画において vello/wgpu 経路を Adreno 向け主力から外し、skia-safe（[ADR-0146](0146-skia-safe-native-scene-renderer.md)）を採用する。** ADR-0146 は「Adreno 破綻の保険」を動機に skia-safe をネイティブ導入する決定だったが、本実機検証を経て skia-safe は**保険ではなく Android の主力レンダラ**へ格上げされる。ADR-0146 の設計（surface 非依存 painter、parley 正本＋SkTextBlob、raster→Ganesh GL、既存シーム流用、ソース非ベンダリング）はそのまま有効で、本 ADR はその**採用理由の確定と優先度の格上げ**を記録する。

具体:

- **skia-safe を Android の standard alternative から主力へ。** vello→skia の一方向 fallback（ADR-0146 §2）を維持しつつ、Adreno での既定順序・raster/GL 既定は実機で確定する（issue #804）。実装ツリーは #798（PRD）配下 #800→#801→#802→#803→#804。
- **vello は web / desktop / iOS では不変。** 破綻はそこに無い。Android ネイティブでのみ skia へ移す。web の 2-WASM 排他（REND-11）も不変。
- **vello の Android 既定は Vulkan + Area のまま据え置く。** GL 化（Adreno 620 は init 不能・810 は同一破綻）・MSAA 化（Adreno を悪化）は実機で棄却されたため、`DEFAULT_WGPU_BACKEND=Vulkan` / `DEFAULT_AA_METHOD=Area` を変更しない。ADR-0145 の切替スイッチ（intent extra）は診断用途で残す。
- **根治（wgpu-hal 修正）は保留。** 唯一未着手の深掘りは「Vulkan synchronization validation layer で wgpu-hal の欠落バリアを特定 → Dawn との差分を実装」だが、費用対効果が読めないため採らない（記録のみ）。将来 wgpu 上流がこれを直せば vello をネイティブに戻す選択肢は残る（`backend-vello` feature が出口、ADR-0146 §5）。

## Considered Options

いずれも**実機で棄却**（証拠は [調査記録](../investigations/2026-07-12-vello-wgpu-adreno-breakage.md)）:

- **GL バックエンドへ切替（Vulkan 凍結）**: Adreno 810 で Vulkan と同一破綻、Adreno 620 で device 生成不能。却下。
- **MSAA へ切替**: NP3a で画面全体が総崩れ。却下。
- **Naga → Tint（SPIR-V passthrough）**: 破綻不変。犯人がシェーダ変換でない証明にはなったが修正にはならない。却下。
- **wgpu の版上げ**: v30 に該当修正なし・既知 Adreno issue は未修正のドライババグ、移行コスト大。却下。
- **wgpu-hal を直接修正（Dawn 相当の Adreno 回避策を実装）**: 根治候補だが上流の深掘りで不確実。保留（記録のみ）。
- **現状維持（vello のまま）**: Adreno 810 実機で常時破綻し、Play 配信端末の品質をドライバ品質に人質に取られる。却下。

## Consequences

- ADR-0146 の skia-safe 実装が「保険」から「Android 主力」へ格上げされ、issue #798/#800–#804 が着手対象になる。
- ネイティブバイナリは vello/wgpu と Skia を併載する（ADR-0146）。`backend-vello` feature（default on）が、将来 vello をネイティブから外す/戻す両方向の出口。
- vello の Android 既定は Vulkan+Area 据え置き。GL/MSAA への変更は本 ADR で明示的に禁止。
- web/desktop/iOS の vello 挙動・構成は不変。
- Adreno 破綻の根本原因は **wgpu-hal の Vulkan 駆動**にあると記録された（Naga・ドライバ単独・シェーダ内容ではない）。将来 wgpu 上流が修正した場合の再評価の起点になる。
- 一次証拠（実機スクリーンショット・ログ・実験マトリクス）は `docs/investigations/2026-07-12-vello-wgpu-adreno-breakage.md` と同 `assets/` に永続化。

## 関係

- **amends / 優先度格上げ** [ADR-0146](0146-skia-safe-native-scene-renderer.md)（skia-safe ネイティブ導入 — 動機を「保険」から「主力」へ確定）。
- **supersedes（仮説部分）** [ADR-0145](0145-android-vello-aa-and-wgpu-backend-runtime-switch.md)（「容疑は Vulkan 経路」を実機で否定。切替スイッチ自体は診断用途で存続、既定 Vulkan+Area も据え置き）。
- **根拠** [docs/investigations/2026-07-12-vello-wgpu-adreno-breakage.md](../investigations/2026-07-12-vello-wgpu-adreno-breakage.md)（実機一次証拠の全記録）。
- **references** ADR-0050（Backend / Scene Renderer 分離）、ADR-0007（ベンダリング方針 — skia-safe 例外）、issue #796（vello 実機実験、本 ADR で結論）、#798（skia PRD）、#800–#804（skia 実装スライス）。
