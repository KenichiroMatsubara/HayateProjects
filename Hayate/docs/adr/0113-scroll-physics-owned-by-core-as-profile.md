# スクロール物理は Hayate Core が Scroll Physics Profile として所有し、Platform Adapter はフレーム駆動と platform 識別供給に徹する

status: accepted
supersedes: ADR-0046

## Context

ADR-0046 は「スクロール物理演算（イナーシャ・rubber-band・スナップ）を Platform Adapter が担い、各プラットフォームの慣習（Web は wheel delta 即時、iOS は UIScrollView 相当、**Android は OverScroller 相当**）を Adapter ごとに実装する」と決めた。

Web/Android アダプタ抽象化を進める中で、この境界には以下の問題が見えた。

1. **ジェスチャ認識の重複**: raw ポインタ列を「タップか scroll か」「どの `scroll-view` を掴んだか」「いま適用すべき 1:1 follow デルタ」へ分類する処理（slop 判定・状態機械）は、プラットフォーム非依存の純粋な意図分類である。これを Adapter ごとに再実装するのは無駄であり、挙動のドリフトを生む。
2. **物理の数学は汎用、感触だけが別**: 指数減衰（fling）・damped spring（spring back）・sigmoid rubber-band は汎用的な数式で、Adapter 間で複製する価値はない。一方で iOS 風（指数減衰＋sigmoid rubber-band の content 変位）と Android 風（OverScroller の spline 減衰＋Material stretch）は、**定数差ではなくアルゴリズム自体が別**であり、単一の parameterized アルゴリズム＋定数差では両 OS のネイティブ感を再現できない。
3. **感触をユーザーが選べる完成形**: スクロールの感触は最終的にアプリ作者が `auto` / `ios` / `android` から選べるべきで、`auto` が各 OS 相当へ解決するのが完成形である。物理を Adapter が個別所有する設計では、プラットフォームを跨いで選択可能な単一プロファイルを提供できない。

## Decision

**スクロール物理は Hayate Core が所有する。** Core は二つの軸を持つ。

- **Scroll Gesture（意図分類）**: raw ポインタ列を「タップ / scroll」「掴んだ `scroll-view`」「適用すべき follow デルタ」へ分類する純粋な状態機械。slop 閾値などの tunable 値は Platform Adapter が引数で供給する。物理に先行し、scroll を始めるか否かまでを決める。
- **Scroll Physics Profile（感触）**: `auto` / `ios` / `android` の閉じた三値。iOS 風（指数減衰＋sigmoid rubber-band）と Android 風（OverScroller の spline＋Material stretch）の**別アルゴリズムをいずれも Core が実装する**。`auto` は Platform Adapter が渡す platform 識別から各 OS 相当の感触へ解決する（完成形）。Core はプラットフォームを自前で検出せず、Adapter から受け取った enum で解決するだけなので platform-free を保つ。

**Platform Adapter は以下に徹する。**

- フレーム駆動（毎フレーム Core の step を進める）
- `Scroll Offset` の適用（Element Document Runtime 経由）
- ポインタ位置のサンプリング
- tunable 値（slop 等）と **platform 識別**の供給

**現状は `auto` のみを公開し、明示 `ios` / `android` 上書きの公開 API は将来とする。** 今日の `auto` は web で iOS profile に解決し、現行の `scroll_drag.rs` の挙動と一致する。Android profile（spline/stretch）は Android タッチスクロール実装時に Core へ追加する。

`Scroll Offset` の基本 offset を Element Document Runtime が単独所有する点、`scroll` イベントがアプリ通知専用である点は ADR-0046 から不変。

## Considered Options

- **ADR-0046 継続（Adapter が物理所有）**: ジェスチャ認識と汎用物理数学を Adapter ごとに複製し、プラットフォーム横断で選択可能な単一プロファイルを提供できない。却下。
- **Core は gesture のみ所有、物理は Adapter**: 汎用の減衰・spring・rubber-band 数式が Adapter 間で複製されたまま残り、プロファイル選択も不可能。却下。
- **Core が単一 parameterized アルゴリズム＋OS 差は定数のみ**: OverScroller の spline＋Material stretch は iOS の指数減衰＋rubber-band と別アルゴリズムであり、定数だけでは Android のネイティブ感を再現できない（Android が iOS 風になる）。却下。
- **Core が gesture ＋ 両物理 profile を所有、Adapter はフレーム駆動（採用）**: 数学・状態機械を一度だけ実装・テストし、感触はプロファイルで選べ、`auto` で各 OS に解決できる。

## Consequences

- ADR-0046 を破棄。スクロール物理の所有が Platform Adapter → Hayate Core へ移る。
- web `scroll_drag.rs` の物理（`momentum_step` / `spring_step` / `rubber_band_offset` / `estimate_release_velocity` / `scroll_motion_step`）と Scroll Gesture 認識（`exceeds_slop` / `ScrollGesture` 等）は Core へ移設。web の iOS profile の正本となる。
- Android タッチスクロール実装時に Android profile（OverScroller spline＋Material stretch）を Core へ追加する責務が生じる。
- `Scroll Physics Profile`・`Scroll Gesture` を CONTEXT.md 用語集に追加済み。`Scroll Offset` の語釈も物理所有を Core 側へ更新済み。
- 明示 `ios` / `android` 上書きの公開 API は将来。現状は `auto` のみ。
- Element Document Runtime による base offset 単独所有、`scroll` イベントのアプリ通知専用、`element_set_scroll_offset` のプログラマティック専用は ADR-0046 から維持。
