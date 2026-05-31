# Hayabusa Single-File Component の拡張子は .hybs とする

Hayabusa の Single-File Component ファイルの拡張子を `.hybs` とする。
**HaYaBuSa** の頭字語に由来する。

## Considered Options

- **`.hbs`**: 短く入力しやすいが、Handlebars テンプレートエンジンが同拡張子を使用しており、
  エディタ・CI ツール・GitHub の言語検出が Handlebars として誤認識する。
  `hayabusa-lsp` の VS Code 拡張で上書きは可能だが、初期セットアップ前の体験が壊れる。
- **`.hybs`（採用）**: Handlebars との衝突がない。HaYaBuSa の頭字語として意味が通り、
  既存ツールが未知の拡張子として素通りするため初期体験が壊れない。
- **`.hayabusa`**: 衝突リスクはないが長すぎる。日常的なファイル操作での入力コストが高い。

## Consequences

- エディタの `files.associations` や言語検出の設定は `*.hybs` を対象とする
- `hayabusa-lsp` の VS Code 拡張は `*.hybs` をデフォルトで Hayabusa 言語に関連付ける
- ドキュメント・サンプルコードはすべて `.hybs` 拡張子を使用する
