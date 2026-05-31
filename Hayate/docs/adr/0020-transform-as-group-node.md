# transform は SceneGraph の Group ノードとして実装する

## Context

`transform`（translate / rotate / scale / matrix）はアニメーションの基盤となる機能であり、Tier 1 必須として位置付けた。実装方法として二つの候補があった。

1. **StyleProp 焼き込み方式**: `StyleProp::Transform(Affine)` を追加し、`scene_build::walk()` が affine を子要素の座標に逐一加算して SceneGraph の各 Node に焼き込む
2. **Group ノード方式**: `NodeKind::Group { transform: Affine, children: Vec<NodeId> }` を SceneGraph に追加し、Vello の `push_transform()` / `pop()` に affine を渡す。GPU 側で matrix を適用する

## Decision

**Group ノード方式（`NodeKind::Group`）を採用する。**

Hayate の速さの根拠のひとつは「変更分だけ mutation する Retained モード」である。アニメーション時に 60fps でサブツリー全体の座標を書き直す焼き込み方式は、ノード数に比例して更新コストが増大しこの原則を損なう。Group ノード方式では `transform` フィールドを 1 回書き換えるだけで GPU が全子孫に matrix を適用する。

`scene_build::walk()` は `StyleProp::Transform` を持つ要素の subtree を `NodeKind::Group` でラップして SceneGraph に挿入する。Vello bridge は Group ノードを `Scene::push_transform()` / `pop()` に変換する。

## Considered Options

- **StyleProp 焼き込み方式（却下）**: 実装が単純だが、「layout 再計算なし」という要件を満たせない。アニメーション時に毎フレーム全子ノードの座標を再計算・更新するため、サブツリーが大きい場合に CPU コストが線形増大する。Hayabusa のトランジション・キーフレームアニメーションを 60fps で動かす基盤として不適
- **Group ノード方式（採用）**: SceneGraph に新 NodeKind が増えるが、Vello は `push_transform()` / `pop()` を nativeにサポートしており bridge 実装は直線的。アニメーション時の更新は Group の `Affine` 1 フィールドの書き換えのみ。layout 再計算ゼロ

## Consequences

- `NodeKind::Group { transform: Affine, children: Vec<NodeId> }` を `node.rs` に追加する
- `scene_build::walk()` が `StyleProp::Transform` を持つ要素を Group でラップする
- `vello_bridge::build_scene()` が Group ノードを `push_transform()` / `pop()` に変換する
- HTML Mode の `HayateElementHtmlRenderer` は要素に `transform` CSS プロパティを直接設定する（Vello を経由しないため Group ノードは不要）
- Raw Layer を直接使うユーザーも `NodeKind::Group` を利用できる
