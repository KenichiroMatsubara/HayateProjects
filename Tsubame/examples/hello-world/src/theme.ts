import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import type { StorageLike } from './todo-model.js';

/**
 * Hayate CSS にはセレクタ・カスケード・custom properties が無いため、テーマは
 * 「ライト/ダーク × アクセント色」を受け取って全色を解決する純関数で表現する。
 * 解決済みパレットを Signal に載せ、全コンポーネントがそれを参照して
 * インラインスタイルを再適用する（意味論パリティ・`CONTEXT.md`）。
 */

export type Theme = 'light' | 'dark';
export type AccentKey = 'teal' | 'pink' | 'orange' | 'lime' | 'violet';

/** スウォッチに並べるアクセント色の順序（UI と検証で共有する正本）。 */
export const ACCENT_KEYS: readonly AccentKey[] = ['teal', 'pink', 'orange', 'lime', 'violet'];

/** 既定はライト（gomi 準拠）。 */
export const DEFAULT_THEME: Theme = 'light';
/** 既定アクセントは teal（従来デモの基調色）。 */
export const DEFAULT_ACCENT: AccentKey = 'teal';

/** 解決済みパレット。キーは旧 `COLORS` 定数と同形で、全コンポーネントが参照する。 */
export interface Palette {
  bg: string;
  rail: string;
  panel: string;
  panel2: string;
  panel3: string;
  ink: string;
  text: string;
  muted: string;
  quiet: string;
  line: string;
  accent: string;
  accent2: string;
  danger: string;
  dangerBg: string;
  success: string;
  successBg: string;
  blue: string;
  violet: string;
  /** 明るいアクセント/塗りの上に載るテキスト色（両テーマとも near-black の ink）。 */
  black: string;
  /** POP な浮きを表す影色（`box-shadow`・ADR-0095）。テーマで濃さを変える。 */
  shadow: string;
}

/** テーマ非依存の基調色（アクセントを除く）。 */
type BasePalette = Omit<Palette, 'accent'>;

const LIGHT_BASE: BasePalette = {
  bg: '#f1ede3',
  rail: '#fbf8f1',
  panel: '#fdfdfb',
  panel2: '#ece6d8',
  panel3: '#e0d8c7',
  ink: '#262130',
  text: '#322c3f',
  muted: '#6f6878',
  quiet: '#9a93a3',
  line: '#d9d3c6',
  accent2: '#ef9d2e',
  danger: '#e5484d',
  dangerBg: '#fbe4e4',
  success: '#2fa86a',
  successBg: '#d8f0e2',
  blue: '#4b8ef0',
  violet: '#8b5cf6',
  black: '#14101c',
  shadow: '#2621301f',
};

const DARK_BASE: BasePalette = {
  bg: '#0b1020',
  rail: '#111827',
  panel: '#162033',
  panel2: '#1b2a3f',
  panel3: '#21344e',
  ink: '#eef4ff',
  text: '#d8e2f2',
  muted: '#8ea1bb',
  quiet: '#5f728d',
  line: '#31425b',
  accent2: '#f59e0b',
  danger: '#fb7185',
  dangerBg: '#3d1722',
  success: '#65d38c',
  successBg: '#163526',
  blue: '#60a5fa',
  violet: '#a78bfa',
  black: '#070b14',
  shadow: '#00000066',
};

/** 各アクセントのテーマ別 hex。明色は dark、彩度を上げた版は light で読みやすいよう分ける。 */
const ACCENT_SWATCHES: Record<AccentKey, { light: string; dark: string }> = {
  teal: { light: '#14b8a6', dark: '#4fd1c5' },
  pink: { light: '#e84d8a', dark: '#f472b6' },
  orange: { light: '#ef8f3c', dark: '#fb923c' },
  lime: { light: '#5ca80f', dark: '#a3e635' },
  violet: { light: '#7c5cf0', dark: '#a78bfa' },
};

/** ライト/ダーク × アクセント色から全色を解決する。 */
export function palette(theme: Theme, accent: AccentKey): Palette {
  const base = theme === 'dark' ? DARK_BASE : LIGHT_BASE;
  return { ...base, accent: ACCENT_SWATCHES[accent][theme] };
}

/** スウォッチ表示用に、現在テーマでのアクセント色を返す。 */
export function accentColor(theme: Theme, accent: AccentKey): string {
  return ACCENT_SWATCHES[accent][theme];
}

/** text-input の基本スタイル。パレットから色を解決する。 */
export function inputStyle(p: Palette): HayateCssStyle {
  return {
    height: 38,
    paddingLeft: 12,
    paddingRight: 12,
    backgroundColor: p.panel2,
    color: p.text,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: p.line,
    fontSize: 13,
    ':focus': {
      borderColor: p.accent,
      backgroundColor: p.panel3,
    },
  };
}

/** テーマ設定の永続化単位。 */
export interface ThemePrefs {
  theme: Theme;
  accent: AccentKey;
}

/** localStorage に書き込む既定のキー（#247 の永続化方針に合わせる）。 */
export const THEME_STORAGE_KEY = 'pop-theme-v1';

const DEFAULT_PREFS: ThemePrefs = { theme: DEFAULT_THEME, accent: DEFAULT_ACCENT };

function isTheme(value: unknown): value is Theme {
  return value === 'light' || value === 'dark';
}

function isAccent(value: unknown): value is AccentKey {
  return typeof value === 'string' && (ACCENT_KEYS as readonly string[]).includes(value);
}

/** テーマ設定を保存用文字列へ変換する。 */
export function serializeTheme(prefs: ThemePrefs): string {
  return JSON.stringify(prefs);
}

/**
 * 保存文字列をテーマ設定へ復元する。
 * null・不正 JSON・形が壊れている・未知のテーマ/アクセントは既定（ライト/teal）へフォールバック。
 */
export function deserializeTheme(raw: string | null): ThemePrefs {
  if (raw === null) return { ...DEFAULT_PREFS };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === 'object' && parsed !== null) {
      const value = parsed as Record<string, unknown>;
      if (isTheme(value.theme) && isAccent(value.accent)) {
        return { theme: value.theme, accent: value.accent };
      }
    }
  } catch {
    // 壊れた JSON は既定へ落とす
  }
  return { ...DEFAULT_PREFS };
}

/** ストレージからテーマ設定を読み込む（無い/壊れていれば既定）。 */
export function loadTheme(storage: StorageLike, key: string = THEME_STORAGE_KEY): ThemePrefs {
  return deserializeTheme(storage.getItem(key));
}

/** テーマ設定をストレージへ保存する。 */
export function saveTheme(storage: StorageLike, prefs: ThemePrefs, key: string = THEME_STORAGE_KEY): void {
  storage.setItem(key, serializeTheme(prefs));
}
