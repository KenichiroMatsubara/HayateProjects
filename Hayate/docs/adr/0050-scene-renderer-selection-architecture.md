# Scene Renderer Selection Architecture

Hayate の描画差し替え可能性を保つため、Vello と `tiny-skia` は `SceneGraph` を消費する `Scene Renderer` として扱い、GPU API を指す `Backend` とは分離する。`Render Host` は renderer の実行・資源寿命管理・one-way fallback を担い、どの renderer を許可し、どの順で試すかは `Renderer Selection Policy` が決める。標準契約は当面、汎用アプリ UI を完璧に支えるための共通表現に寄せ、Vello は `preferred default renderer`、`tiny-skia` は同質性を保つ `standard alternative renderer` とする。`recording` / `null` は本番候補ではなく非標準 renderer 群として分離し、高機能描画は必要になった時点で標準契約へ直ちに入れず、まず拡張契約として試す。

**SceneGraph の「消費」**（[ADR-0054](0054-scene-painter-shared-walk.md)）: 各 Scene Renderer は `SceneGraph` を直接 walk しない。`hayate-core` の `render_scene_graph` が一度だけ walk し、実装は `ScenePainter` trait への委譲のみを担う。実装は `crates/scene-renderers/{vello,tiny-skia}` に置き、adapter は `render_scene` を呼ぶ。
