---
name: inherit-prompt
description: >
  Generate a self-contained handover prompt from the current conversation and
  codebase state so the next session can continue without context loss.
  Use when user invokes /inherit-prompt or asks for a "引き継ぎプロンプト" /
  continuation prompt / handover prompt.
---

会話・コードベース・git 状態をもとに、次のセッションが即座に作業を再開できる
引き継ぎプロンプトを生成する。

## 出力形式（厳守）

- **チャットに直接出力する**（ファイルに書き出さない）
- プロンプト全体を **1 つの ``` ブロック** で囲む
- ブロック内に ``` を入れない（コピーボタンが壊れる）
- コード例は **4 スペースインデント** のみで表現する
- モバイルのコピーボタン 1 タップで全文コピーできることがゴール

## 生成手順

1. `git branch --show-current` と直近 5 コミットをメモする
2. 会話履歴から「何をした・何が残っている」を抽出する
3. 関連ファイルを列挙し、行番号付きで記録する
4. 下記テンプレートに埋めてチャットに出力する

## テンプレート

    ## タスク概要

    <1〜3 行で「なにを・なぜ」を書く>

    ---

    ## ブランチ

    <git branch 名>

    ---

    ## 背景・調査済み事項

    <前セッションで判明した事実。ファイルパス:行番号付きで>

    ---

    ## 残タスク

    1. <具体的なタスク。ファイルパスと変更内容まで書く>
    2. ...

    ---

    ## 関連ファイル

        ファイルパス                        役割・注目点
        ─────────────────────────────────────────────────
        path/to/file.rs:123                 <一言>
        path/to/other.ts:45                 <一言>

    ---

    ## 実装上の注意

    <アーキテクチャ上の制約・落とし穴・やってはいけないこと>

    ---

    ## 確認コマンド

        <ビルド・テスト手順をインデントで>

## 例

**入力**: `/inherit-prompt`（border-radius 修正作業の途中）

**出力**:

```
## タスク概要

Canvas モードで border-radius が効かない問題を修正する。
さらに、未知スタイルプロパティが WASM に渡った際のサイレント失敗に
console.warn を追加する。

---

## ブランチ

claude/css-border-radius-issue-5Lb7o

---

## 背景・調査済み事項

- border_radius は scene_build.rs:146 で背景塗り矩形にのみ使われる
- background_color が None の要素では border_radius が格納されるが描画されない
- ボーダー4辺矩形はすべて corner_radius: 0.0 ハードコード (scene_build.rs:174)
- canvas-renderer.ts:154 の flush() が apply_mutations() の例外を握り潰している

---

## 残タスク

1. canvas-renderer.ts:154 flush() を try-catch で囲み console.warn を追加
2. scene_build.rs:174 のボーダー矩形に el.visual.border_radius を渡す
3. background_color が None かつ border_radius > 0 のとき warn ログを出す

---

## 関連ファイル

    ファイルパス                                                  役割・注目点
    ───────────────────────────────────────────────────────────────────────────
    Tsubame/packages/renderer-canvas/src/canvas-renderer.ts:154  flush() 修正箇所
    Hayate/crates/core/src/element/scene_build.rs:135-181        背景/ボーダー描画
    Hayate/crates/adapters/web/src/backend/vello.rs:207-228      RoundedRect 描画
    Hayate/crates/core/src/element/tree.rs:1231                  border_radius 格納
    Hayate/crates/adapters/web/src/style_packet.rs:338           DOM mode 適用(正常)

---

## 実装上の注意

- DOM Renderer (renderer-dom) は WASM を経由しない。style-mapping.ts が
  el.style.borderRadius に直接書き込む。こちらは現状正常動作。
- Canvas モードと DOM モードで挙動が異なる可能性があるため両方でテストする。
- scene_build.rs の border 4辺矩形を stroke 方式に切り替えると breaking change
  になる可能性あり。まず corner_radius の変更のみで様子を見る。

---

## 確認コマンド

    # Rust ビルド
    cd Hayate && cargo build --target wasm32-unknown-unknown

    # TypeScript ビルド
    cd Tsubame && pnpm build
```
