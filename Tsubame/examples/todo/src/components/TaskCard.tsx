import { type Palette } from '../theme';
import { EASE } from '../ui/styles';

/**
 * タスクカードを構成する小さなプレゼンテーション部品群。いずれもタスク画面専用で
 * `Palette` のみに依存し、他画面では再利用しないため、1ファイルにまとめる。
 */

export function Header(props: { colors: Palette; remaining: number; total: number; percent: number }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <view style={{
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
      }}>
        <text style={{ color: props.colors.ink, fontSize: 24 }}>きょうのタスク</text>
        <text style={{ color: props.colors.muted, fontSize: 13 }}>
          {`残り ${props.remaining} 件 / 全 ${props.total} 件`}
        </text>
      </view>
      <ProgressBar colors={props.colors} percent={props.percent} />
    </view>
  );
}

function ProgressBar(props: { colors: Palette; percent: number }) {
  return (
    <view style={{
      width: '100%',
      height: 12,
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      backgroundColor: props.colors.black,
      borderRadius: 8,
      borderWidth: 1,
      borderStyle: 'solid',
      borderColor: props.colors.line,
    }}>
      <view style={{
        width: `${props.percent}%`,
        height: 8,
        marginLeft: 2,
        backgroundColor: props.colors.success,
        borderRadius: 6,
      }} />
    </view>
  );
}

/**
 * 読み取り専用テキストの選択ジェスチャデモ（ADR-0108、ADR-0097 を supersede /
 * issue #266・#267・#268・#269）。
 *
 * CSS `user-select` と同型で、view / text は**宣言なしで既定選択可**（opt-out）。
 * 明示 `user-select: none` を置いた subtree だけが選択から除外される。DOM Mode
 * ではブラウザのネイティブ選択に委ね、ドラッグに加えダブルクリックで単語・
 * トリプルクリックで段落、Shift+クリック / Shift+矢印で範囲拡張、Cmd/Ctrl+A で
 * 全選択ができる。Cmd/Ctrl+C で選択テキストが Platform Adapter 経由でクリップ
 * ボードへコピーされる。
 *
 * 末尾のキャプションは `user-select: none` を持つ view に包まれており、本文を
 * 全選択しても選択対象に入らない（opt-out の確認）。
 */
export function SelectableNote(props: { colors: Palette }) {
  const para = { color: props.colors.muted, fontSize: 13 } as const;
  return (
    <view
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 12,
        backgroundColor: props.colors.panel2,
        borderRadius: 12,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: props.colors.line,
      }}
    >
      <text style={para}>
        この段落は宣言なしで選択できます。ダブルクリックで単語、トリプルクリックで段落を選び、Shift+クリックや Shift+矢印で範囲を伸縮、Cmd/Ctrl+A で全選択できます。選択して Cmd/Ctrl+C を押すとクリップボードへコピーされ、別アプリへ貼り付けられます。
      </text>
      <text style={para}>
        これは二つ目の段落です。view / text は CSS `user-select` と同型で既定選択可なので、`selectable` を宣言しなくても選択できます。
      </text>
      <view user-select="none">
        <text style={{ color: props.colors.muted, fontSize: 11 }}>
          このキャプションは user-select: none の view に包まれているので、本文を全選択しても選択対象に入りません。
        </text>
      </view>
    </view>
  );
}

export function EmptyState(props: { colors: Palette }) {
  return (
    <view style={{
      height: 96,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: props.colors.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderStyle: 'solid',
      borderColor: props.colors.line,
    }}>
      <text style={{ color: props.colors.muted, fontSize: 14 }}>表示するタスクがありません</text>
    </view>
  );
}

export function Footer(props: { colors: Palette; percent: number; onClearDone: () => void }) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
    }}>
      <text style={{ color: props.colors.muted, fontSize: 13 }}>{`${props.percent}% 完了`}</text>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <text style={{ color: props.colors.quiet, fontSize: 11 }}>クリックで完了 / × で削除</text>
        <button
          style={{
            height: 30,
            paddingLeft: 12,
            paddingRight: 12,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: props.colors.panel2,
            defaultColor: props.colors.text,
            borderRadius: 8,
            borderWidth: 1,
            borderStyle: 'solid',
            borderColor: props.colors.line,
            defaultFontSize: 12,
            ...EASE,
            ':hover': { backgroundColor: props.colors.panel3, borderColor: props.colors.danger, defaultColor: props.colors.danger },
          }}
          onClick={props.onClearDone}
        >
          完了を消す
        </button>
      </view>
    </view>
  );
}
