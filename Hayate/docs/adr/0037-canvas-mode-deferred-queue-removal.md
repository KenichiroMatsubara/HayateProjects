# Canvas Mode の deferred queue を廃止する

ADR-0030 の deferred queue は「JS→WASM 境界を N 回から 1 回/frame に削減する」ために導入された。しかし Tsubame（Canvas Mode）は JS 側でフレーム分の mutations をバッチ化して apply_mutations(batch) で一括渡しするため、Hayate 内部でさらにキューイングする意味がない。Hayabusa は Hayate と単一 WASM バイナリにリンクされるため境界コスト自体が存在しない。

Canvas Mode の deferred queue を廃止する。apply_mutations で受け取った batch を即時処理し render する。

HTML Mode の deferred queue は維持する。HTML Mode では DOM mutation が即時反映されるとレイアウトスラッシングが起きるため、バッチ化による DOM flush の一括化は依然として必要である。ADR-0030 の rationale は HTML Mode 限定として読み替える。
