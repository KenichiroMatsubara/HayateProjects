# Hayate CSS の意味論はレンダラー非依存の契約である（DOM 系レンダラーは2チャネル継承を再現する）

**Status: accepted**

**Date: 2026-06-11**

## Context

同一アプリを DOM で開発して Canvas で動かすと文字色が消える事故が起きた（Badge 問題）。原因は、Tsubame DOM Renderer が全要素に `color: inherit; font: inherit` を付与しブラウザの CSS 継承で通常テキストスタイルが block を貫通する一方、Canvas（Hayate）は ADR-0065 の2チャネル継承（通常スタイルは text-local、`default-*` のみ block 貫通）に従うため。スタイル語彙（Hayate CSS）は共有しているのに、その**意味**がレンダラーごとに違っていた。

「Canvas を CSS 式継承に戻す（ADR-0047 への回帰、0065 の supersede）」も検討したが却下した。アプリ全体の既定値というユースケースは `default-*` チャネルが既にカバーしており、0065 の予測可能性の論拠（layout 用 view の font 指定が深部の text を restyle しない）は依然有効。

## Decision

Renderer Protocol のスタイル語彙は名前だけでなく**意味論ごと**契約である。継承を含む Hayate CSS のセマンティクス（ADR-0065 の2チャネルモデル）は、Canvas / DOM / HTML Mode すべてのレンダラーで同一でなければならない。Canvas が正準であり、DOM 系レンダラーがブラウザ既定の挙動を抑制して合わせる。

DOM 系レンダラーの写像規則:

- **通常テキストスタイル**（color / font-family / font-size / font-weight / font-style / text-decoration）は **text 要素にのみ** CSS として発行する。span の入れ子間のブラウザ継承は IFC 内の text→text 継承（チャネル1）と一致するので、そのまま委ねてよい。
- **block box 上の通常テキストスタイルは no-op**（Canvas と同じ）。子孫に届く形で CSS を発行してはならない。
- **`default-*`（チャネル2）** は対応する継承 CSS プロパティ（color / font-* 等）として要素に発行し、block 貫通はブラウザ継承に委ねる。

## Consequences

- Tsubame DOM Renderer の BASE_STYLE（全要素 `color: inherit; font: inherit`）は上記規則に沿って改修が必要。
- block 要素の `color` 指定でラベル色を出していたデモコードは DOM モードでも色が付かなくなる。`defaultColor` か text 直接指定へ書き換える（Canvas では元々効いていないため、「DOM だけ動いていた偶然」の除去）。
- 将来、block 要素への通常テキストスタイル指定に lint / dev warning を出す余地がある（語彙上は受理されるが常に no-op のため）。
- ADR-0065（Hayate）の HTML Mode 写像方針を、Tsubame DOM Renderer を含む全 DOM 系レンダラーへ一般化したものに当たる。
