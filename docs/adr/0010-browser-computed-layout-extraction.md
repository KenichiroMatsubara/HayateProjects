---
status: superseded by ADR-0011
---

# HTML/CSS レイアウトはブラウザの計算結果抽出に委譲する

`getBoundingClientRect()` + `getComputedStyle()` でブラウザ計算済みのレイアウトを抽出すれば CSS エンジン不要で Extension を軽量化できると考えたが、誤りだった。これらの API はブラウザが reflow を完了した後にしか呼べないため、DOM の描画コストは消えない。ペイントを GPU に置き換えるだけで reflow コストはそのまま残り、抽出+Hayate 描画の分だけ逆に重くなる。
