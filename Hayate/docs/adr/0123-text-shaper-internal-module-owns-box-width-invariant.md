# テキスト整形を LayoutPass 内部 module TextShaper に集約し box幅不変条件を所有する

**Status: accepted**

**Date: 2026-06-28**

## Context

retained グリフ層（描画用の glyph runs）が「要素の最終ボックス幅でシェイプされる」という不変条件に、所有者が居なかった。整形は以下に分散していた:

- `layout_pass.rs` の measure クロージャ — Taffy の `compute_layout_with_measure` 中に `inline_text::shape` を呼び、結果を `pending` HashMap に **last-wins** で溜める。
- compute_layout 後の drain — `pending` を各要素へ書き戻すついでに、最終ボックス幅と不一致なら再シェイプする（後付けパッチ）。
- `text.rs` の `width_constraint` フィールド — 「直近どの幅でシェイプしたか」の簿記を retained 値に載せていた。
- text-input の `content_layout` 経路 — drain とは別ループで `content_box_width()` を使い1回シェイプ（こちらは偶然正しい）。`missing_families → FetchFont` 発行ロジックが IFC 側と**重複**。

タイトル折れバグ（react-demo、`align-items: baseline` 行）はこの不在を露呈した: Taffy は確定サイズの後にも intrinsic-size プローブ（min/max-content）を走らせ、baseline 行ではその最後が MinContent（max_advance=0）になり、`pending` が min-content 折返しのグリフを保持する。ボックス幾何は確定サイズからキャッシュされ正しいため、正しい幅の箱に 0 幅折返しのグリフが描かれ縦にはみ出した。commit 7cc0605 で drain に再シェイプを足して塞いだが、不変条件の**持ち主は依然不在**で、1リファクタで再発し得る。

## Decision

整形を **`LayoutPass` の private field となる内部 module `TextShaper`** に集約する。`TaffyProjection`（箱）と対をなし、グリフを所有する deep module。`ElementTree` の public surface は不変（ADR-0075 と同型の「内部 module 抽出・public 不変」方針）。

**所有するもの**: `font_cx`（font collection）／ `layout_cx` ／ settle ごとの**幅キーのシェイプメモ**／ box幅不変条件。

**小さい interface（2点で box レイアウトと接続）**:

- `register_font(family, bytes)` — collection への登録。`ElementTree::register_font` が委譲する。
- `measure(ctx, known, available, &elements, viewport) -> Size` — Taffy への寸法回答。内部のメモを埋める。
- `finalize(&projection, &mut elements, viewport) -> Vec<MissingFamily>` — 全テキストを**確定（unrounded）ボックス幅**で retain し（`text_layout` ＝ IFC と `content_layout` ＝ text-input の**両方**）、欠落 family を**値として返す**。
- `shape_label(text, style) -> TextLayout` — toolbar 等の単発ラベル。
- 内部プリミティブ: メモ化 `shape(eid, width)`（settle ごとにクリア）。

**不変条件の機械的保証**: 正しさは「finalize が box幅でシェイプした結果を retain する」ことだけに依存する。メモは純粋な最適化で、(eid, width) キーが浮動小数差でミスしても finalize はその場で box幅シェイプするだけ — ヒット時に高速、ミス時は無メモ版に優雅に劣化し、**決して間違わない**。`width_constraint` はコードベースから消滅し、内部のメモキーになる。retained `TextLayout` は純粋な描画データ（`layout` / `runs` / `font_size` / `text` / `range_map`）に縮む。

**境界（モジュール外）**:

- フォント取得トランスポート（family → bytes）・再試行・backoff は持たない（port = 別レイヤ）。`FetchFont` の発行は `LayoutPass` が欠落集合から**1箇所**で行う（IFC/content の重複を解消）。
- キャレット/選択クランプ（EditState・ADR-0069）は finalize 後に `LayoutPass` が text-input へ走らせる後処理。`TextShaper` は `content_layout` を生成するのみで編集意味論を持たない。

**`LayoutPass` の縮退**: 「compute を回す → measure を `TextShaper` に配線 → finalize → `layout_cache` 更新」の composer になる。`last_cursor_toggle_ms`（ADR-0032）と `layout_cache` は整形と無関係なので `LayoutPass` に残る。

## Considered Options

- **細いプリミティブだけ抽出（`shape(text, width)` 関数のみ）し、measure/pending/drain のオーケストレーションは `LayoutPass` に残す**: 不変条件の持ち主が依然 drain のまま。adapter 1個 = 仮の seam で、deletion test に落ちる（消すと不変条件が四散して再現する）。却下。
- **finalize で毎回フレッシュに reshape（メモを持たない）**: 最も単純で `width_constraint` も消えるが、shape-dirty テキスト1つにつき finalize で1回余分にシェイプする。最終形の凝集は同じだが、幅キーメモ（採用案）が「通常 measure が既に box幅で shape 済み」を突いて余分シェイプを避けつつ同じ不変条件を保証するため、メモ版を採用。
- **`width_constraint` を retained `TextLayout` に残す**: 外部読者ゼロの簿記フィールドが描画データに居座る。簿記はメモキーとして整形器内部に置き、retained は純粋データにする。却下。
- **font_cx を `LayoutPass` 所有のまま `TextShaper` に借用させる**: 「font collection を所有する者」と「テキストをシェイプする者」が別になる。collection は本質的にシェイプの内在状態（共有 collection の実消費者は全てシェイプ系）なので、整形器が所有する。却下。
- **欠落 family を `TextShaper` が直接 `FetchFont` 発行**: `event_queue` と `font_fetches`（候補2＝fetch lifecycle 領分）に結合し、副作用を持つ。欠落は値として返し、発行は呼び出し側に委ねる。却下。
- **IFC（`text_layout`）だけをスコープにする**: `content_layout` が同じ不変条件と（重複した）欠落発行を別経路に残す — 元バグを生んだ「概念の持ち主が分かれている」状態の再生産。両 retained 層を所有する。却下。
- **`ElementTree` を改称し新 public 型にする**: ADR-0075 同様、確立した public surface を壊すリターンが無い。`ElementTree` 不変・内部 module 抽出を踏襲。却下。

## Consequences

- box幅不変条件に唯一の home ができ、retained グリフが箱からはみ出す類のバグが構造的に再発しない。
- `width_constraint` フィールドが消滅。retained `TextLayout` は純粋な描画データになる。
- `missing_families → FetchFont` 発行が `LayoutPass` の1箇所に統合され、IFC/content の重複が解消。ボックス幅ソース（丸め/未丸め）も1箇所で決まる。
- 全シェイプ経路（IFC・text-input content・UA default width・toolbar ラベル）が `TextShaper` を通り、`font_cx` が `LayoutPass` を貫く共有借用ではなくなる。
- `LayoutPass` の実装が `TaffyProjection`（箱）＋ `TextShaper`（グリフ）の composer に縮む。
- テスト面が明確化: 回帰は `ElementTree::render` 統合シーム（Taffy measure 順序の相互作用を再現）に残し、`TextShaper` interface テスト（不変条件・メモ・欠落返却）を主軸に、`build_text_layout` 系プリミティブは整形器の内部テストへ移す。
- 欠落 family を値で返す `finalize` の戻り口が、将来の FontProvider port（fetch トランスポートの seam）が差し込まれる清潔な接続点になる。

## 関係

- ADR-0064：`TaffyProjection` の lazy 投影は不変。`TextShaper` はその measure 葉（IFC ルート）への回答側を所有し、Projection と対をなす。
- ADR-0075：`ElementEngine` と同型の「内部 module 抽出・`ElementTree` public 不変」方針を踏襲。dirty 解決（`shape_dirty`）は従来どおり `commit_frame` 経路で `TextShaper` の整形を駆動する。
- ADR-0063：IFC ルートを measure 葉とする整形範囲モデルは不変。`TextShaper` はその範囲をシェイプする所有者。
- ADR-0069：text-input の EditState/ImeBridge と caret は `TextShaper` の対象外。`content_layout` の生成のみ整形器が担い、キャレット/選択クランプは finalize 後の `LayoutPass` 後処理に残す。
- ADR-0042/0043/0106：欠落 family の検出（codepoint→family・.notdef）は `TextShaper` が担うが、family→source 解決・取得・再試行/backoff（adapter 領分）には踏み込まない。`finalize` が返す欠落集合がその seam の入口。
- CONTEXT.md：語彙「Text Shaper（テキスト整形器）」を追加済み。
- commit 7cc0605：drain への後付け再シェイプは本決定で `TextShaper::finalize` の不変条件保証に吸収される（パッチからモジュール所有へ）。
