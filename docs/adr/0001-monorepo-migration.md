# HayateProjects をモノレポに統合する

Hayate・Tsubame をそれぞれ独立リポジトリ（ポリレポ）として管理していたが、
リモート AI ツール（Grill 等）はリポジトリ単位でコンテキストを持つため、
Tsubame の作業中に Hayate のアーキテクチャが見えず、ドキュメントが分断されていた。

## 採用した設計

- `HayateProjects/` を単一 git リポジトリのルートとする
- Hayate の git 履歴を主体とし、Tsubame の履歴を DAG に接続（`git merge -s ours`）
- ディレクトリ構造は変更なし（Hayate/ と Tsubame/ が並列）
- git リモート: `origin` は Hayate の旧 GitHub リポジトリを継承
- アーキテクチャ上の分離（Hayate は Tsubame を知らない）は維持

## Considered Options

- ポリレポを維持しドキュメントを同期: 二重管理で即腐る
- git submodule: リモート AI ツールの対応が貧弱
- ローカル起動点の統一: リモートでは解決しない

## 影響

- `Tsubame/docs/adr/0001-independent-repository.md` の「別リポジトリ」の決定はリポジトリ構造として superseded
  （アーキテクチャ上の分離設計自体は有効）
- CONTEXT.md はリポジトリルートに一元化
