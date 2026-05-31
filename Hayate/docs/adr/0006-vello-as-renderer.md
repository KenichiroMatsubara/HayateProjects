# 2D レンダラーに Vello を直接採用する

Hayate の GPU 描画レイヤーに Vello（Linebender, wgpu ベースの GPU compute 2D renderer）を直接採用する。

Vello-style の GPU compute shader path rendering（flatten → binning → coarse → fine の4ステージ）を自前実装することも検討したが、GPU compute shader のデバッグは AI コーディングアシスタントを用いても人間の目視確認サイクルが必要であり、Phase 0 のリソースをここに費やすべきでない。Hayate の差別化は Scene Graph + Layout + 言語非依存 C ABI にあり、path rendering アルゴリズムの独自実装ではない。

Vello は wgpu の上で動き、Linebender テキストスタック（parley/fontique）と同チーム設計。Hayate のコア技術スタックが Linebender エコシステムで統一される。

Vello の API が将来破壊的変更を行った場合に備え、Hayate の Scene Graph → Vello Scene 変換は薄い独立したレイヤーとして分離する。
