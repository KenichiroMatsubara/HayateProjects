---
name: inherit-prompt
description: >
  Generate a self-contained prompt to paste into a new Claude Code session so
  it can continue the current work without any context loss.
  Use when user invokes /inherit-prompt or asks for a "引き継ぎプロンプト" /
  session handover / continuation prompt.
---

現在の会話・コードベース・git 状態をもとに、**新しい Claude Code セッションに
そのまま貼り付けて作業を再開できるプロンプト**を生成する。

出力は次のセッションの Claude への「最初のメッセージ」として機能する。
読み手は今のセッションを一切知らない前提で書く。

## 出力ルール（絶対厳守）

- チャットに直接出力する（ファイルに書き出さない）
- バッククォート3つ（```）を出力に一切含めない
- コード・コマンドは 4 スペースインデントのみで表現する
- これによりテキスト全体をそのまま選択してコピーできる

## 生成手順

1. git branch --show-current と git log --oneline -5 を実行してブランチ・コミット履歴を確認する
2. 会話履歴から「判明した事実」「完了済み」「残タスク」を抽出する
3. 関連ファイルをファイルパス:行番号付きで列挙する
4. 次のテンプレートに埋めて出力する

## テンプレート

次のセッションの Claude への語りかけとして書く。
「あなたは〜を続けるClaude Codeです」から始め、
作業再開に必要な全情報を1メッセージに収める。

    あなたは [プロジェクト名] の開発を引き継ぐ Claude Code です。
    以下のコンテキストを読んで、「残タスク」を上から順に実装してください。

    ## プロジェクト概要

    <プロジェクトの目的・技術スタックを2〜3行で>

    ## 作業ブランチ

    <branch名>

    ## 前セッションで判明した事実

    - <ファイルパス:行番号> <何がわかったか>
    - ...

    ## 完了済み

    - <すでに実装・コミット済みの内容>

    ## 残タスク（上から順に実装）

    1. <ファイルパス:行番号> を <具体的にどう変える>
    2. ...

    ## 関連ファイル早見表

        パス                                        役割
        ──────────────────────────────────────────────────────────
        path/to/file.rs:123                         <一言>

    ## 実装上の注意

    - <やってはいけないこと・破壊的変更になる可能性があること>

    ## 作業開始の手順

        git status
        git log --oneline -5
        <ビルド確認コマンド>

    まず上記コマンドで現在の状態を確認してから実装を開始してください。

---

## 例

入力: /inherit-prompt（border-radius 修正作業の途中）

出力:

    あなたは HayateProjects の開発を引き継ぐ Claude Code です。
    以下のコンテキストを読んで、「残タスク」を上から順に実装してください。

    ## プロジェクト概要

    Hayate は GPU ネイティブ UI 基盤（Rust + WASM）、Tsubame はその TypeScript
    バインディング。Canvas モード（Vello/WebGPU）と DOM モードの2系統がある。
    スタイルは f32 バイナリパケットで WASM に渡し、Rust 側でデコード・適用する。

    ## 作業ブランチ

    claude/css-border-radius-issue-5Lb7o

    ## 前セッションで判明した事実

    - Hayate/crates/core/src/element/scene_build.rs:135-150
      border_radius は背景塗り矩形にのみ使われ、background_color が None なら完全無視
    - Hayate/crates/core/src/element/scene_build.rs:174
      ボーダー4辺矩形の corner_radius が 0.0 ハードコードのため角が丸くならない
    - Tsubame/packages/renderer-canvas/src/canvas-renderer.ts:154
      flush() が apply_mutations() を try-catch せず WASM エラーをサイレント破棄
    - Hayate/crates/core/src/element/tree.rs:1231
      apply_visual() は border_radius を格納するだけで描画への影響は検証しない
    - Tsubame/packages/renderer-dom/src/style-mapping.ts:74-91
      DOM Renderer は WASM を経由せず el.style.borderRadius に直接書き込む（正常動作）

    ## 完了済み

    - /inherit-prompt スキルの作成（.claude/skills/inherit-prompt/SKILL.md）
    - CONTEXT.md から引き継ぎプロンプトルールをスキルに移管

    ## 残タスク（上から順に実装）

    1. Tsubame/packages/renderer-canvas/src/canvas-renderer.ts:154
       flush() の apply_mutations() 呼び出しを try-catch で囲み
       catch 内で console.warn('[Hayate] apply_mutations failed:', err) を出す
       finally ブロックで ops と styles を必ずクリアする

    2. Hayate/crates/core/src/element/scene_build.rs:174
       ボーダー4辺矩形の corner_radius: 0.0 を el.visual.border_radius に変更する

    3. Hayate/crates/core/src/element/scene_build.rs:135 付近
       border_radius > 0.0 かつ background_color.is_none() のとき
       web_sys::console::warn_1(&"[Hayate] border-radius has no effect without background-color".into())
       を追加する

    ## 関連ファイル早見表

        パス                                                          役割
        ─────────────────────────────────────────────────────────────────────────
        Tsubame/packages/renderer-canvas/src/canvas-renderer.ts:154  flush() 修正箇所
        Hayate/crates/core/src/element/scene_build.rs:135-181        背景/ボーダー描画
        Hayate/crates/adapters/web/src/backend/vello.rs:207-228      RoundedRect GPU描画
        Hayate/crates/core/src/element/tree.rs:1231                  border_radius 格納
        Hayate/crates/adapters/web/src/style_packet.rs:338           DOM mode 適用

    ## 実装上の注意

    - DOM Renderer (renderer-dom) は WASM を経由しないため今回の修正対象外
    - Canvas モードと DOM モードで挙動差が出やすいので両方で動作確認すること
    - ボーダー矩形を stroke 方式に切り替えると breaking change になる可能性あり
      今回は corner_radius の値変更のみに留め、stroke 切り替えは別 issue にする

    ## 作業開始の手順

        git status
        git log --oneline -5
        cd Tsubame && pnpm build
        cd Hayate && cargo check --target wasm32-unknown-unknown

    まず上記コマンドで現在の状態を確認してから実装を開始してください。
