# Skia free-draw UI prototype

Question: SkiaSafe の自由描画を、装飾ではなく Todo の情報構造そのものへどう組み込むか。

- A — Orbit timeline: 実用性を残しながら、進捗を曲線上の通過点として描く。
- B — Focus orb: 一覧より「次の一件」を優先し、達成率をセグメント状の軌道で描く。
- C — Constellation: タスクを優先度・完了状態を持つ星として空間配置する。

Current recommendation: C を主画面、B のオーブを選択中タスクのフォーカス表示として組み合わせる。A は既存 Todo から安全に移行する案として残す。

Verdict: pending user review. 採用案が決まったら、勝者を本実装として書き直し、この prototype ディレクトリと切替UIを削除する。
