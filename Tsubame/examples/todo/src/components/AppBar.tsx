import type { HayateCssStyle } from '@torimi/tsubame-renderer-protocol';
import type { Page } from '../App';
import { ACCENT_KEYS, accentColor, type AccentKey, type Palette, type Theme } from '../theme';
import { EASE, glow } from '../ui/styles';

/** 水平スペーサ（幅 w の不可視 view）。AppBar の左右インセット調整に使う。 */
const SpX = (w: number) => <view style={{ width: w, height: 1 }} />;

export function AppBar(props: {
  page: Page;
  setPage: (page: Page) => void;
  colors: Palette;
  theme: Theme;
  accent: AccentKey;
  onToggleTheme: () => void;
  onAccent: (accent: AccentKey) => void;
}) {
  const tab = (active: boolean): HayateCssStyle => ({
    height: 34,
    paddingLeft: 16,
    paddingRight: 16,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? props.colors.accent : props.colors.panel,
    defaultColor: active ? props.colors.black : props.colors.text,
    borderRadius: 10,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: active ? props.colors.accent : props.colors.line,
    defaultFontSize: 13,
    boxShadow: active ? glow(`${props.colors.accent}44`) : [],
    ...EASE,
    ':hover': {
      backgroundColor: active ? props.colors.accent : props.colors.panel3,
      borderColor: active ? props.colors.accent : props.colors.line,
    },
  });

  const swatch = (key: AccentKey): HayateCssStyle => {
    const selected = props.accent === key;
    return {
      width: 22,
      height: 22,
      backgroundColor: accentColor(props.theme, key),
      borderRadius: 999,
      borderWidth: selected ? 3 : 1,
      borderStyle: 'solid',
      borderColor: selected ? props.colors.ink : props.colors.line,
      boxShadow: selected ? glow(`${accentColor(props.theme, key)}66`) : [],
      ...EASE,
      ':hover': { borderColor: props.colors.ink },
    };
  };

  return (
    <view
      style={{
        minHeight: 64,
        display: 'flex',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        flexWrap: 'wrap',
        gap: 12,
        paddingTop: 8,
        paddingBottom: 8,
        backgroundColor: props.colors.rail,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: props.colors.line,
      }}
      // 狭幅では左右のクラスタを縦積みにして溢れを防ぐ（本物の @media・ADR-0081）。
      // flexWrap は nowrap に戻す。column のまま wrap だと低い minHeight に対して
      // 列方向へ折り返し、コントロール群がロゴの下でなく右へ回り込んでしまう。
      styleVariants={[
        {
          condition: { maxWidth: 719 },
          style: { flexDirection: 'column', flexWrap: 'nowrap', alignItems: 'flex-start' },
        },
      ]}
    >
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        {SpX(24)}
        <view style={{
          width: 38,
          height: 38,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: props.colors.accent,
          borderRadius: 12,
        }}>
          <text style={{ fontSize: 18, color: props.colors.black }}>TS</text>
        </view>
        <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <text
            style={{ fontSize: 20, color: props.colors.ink }}
            // 狭幅ではタイトルを一段縮める。
            styleVariants={[{ condition: { maxWidth: 719 }, style: { fontSize: 17 } }]}
          >
            Tsubame Task Studio
          </text>
          {/* サブタイトルは狭幅で隠してヘッダーの段数を減らす。 */}
          <text
            style={{ fontSize: 12, color: props.colors.muted }}
            styleVariants={[{ condition: { maxWidth: 719 }, style: { display: 'none' } }]}
          >
            POP TODO + Hayate CSS gallery
          </text>
        </view>
      </view>

      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', flexWrap: 'wrap', gap: 10 }}>
        <button style={tab(props.page === 'tasks')} onClick={() => props.setPage('tasks')}>Tasks</button>
        <button style={tab(props.page === 'gallery')} onClick={() => props.setPage('gallery')}>CSS Gallery</button>

        <view style={{ width: 1, height: 22, backgroundColor: props.colors.line }} />
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          {ACCENT_KEYS.map((key) => (
            <button style={swatch(key)} onClick={() => props.onAccent(key)}>{' '}</button>
          ))}
        </view>
        <button
          style={{
            width: 34,
            height: 34,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: props.colors.panel,
            defaultColor: props.colors.text,
            borderRadius: 10,
            borderWidth: 1,
            borderStyle: 'solid',
            borderColor: props.colors.line,
            defaultFontSize: 15,
            ...EASE,
            ':hover': { backgroundColor: props.colors.panel3, borderColor: props.colors.line },
          }}
          onClick={props.onToggleTheme}
        >
          {props.theme === 'dark' ? '☀' : '🌙'}
        </button>

        {/*
          右上の固定切替UI（index.html）はチップ幅ぶん右端を占有する。右寄せの
          コントロールが被らないよう、末尾にチップ幅相当のクリアランスを確保する。
        */}
        {SpX(100)}
      </view>
    </view>
  );
}
