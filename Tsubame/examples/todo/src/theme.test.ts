import { describe, expect, it } from 'vitest';
import type { StorageLike } from './todo-model.js';
import {
  ACCENT_KEYS,
  accentColor,
  DEFAULT_ACCENT,
  DEFAULT_THEME,
  deserializeTheme,
  loadTheme,
  palette,
  saveTheme,
  serializeTheme,
  type ThemePrefs,
} from './theme.js';

function fakeStorage(initial: Record<string, string> = {}): StorageLike {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => {
      map.set(key, value);
    },
  };
}

const DEFAULTS: ThemePrefs = { theme: DEFAULT_THEME, accent: DEFAULT_ACCENT };

describe('palette defaults', () => {
  it('defaults to the light theme (gomi-compliant)', () => {
    expect(DEFAULT_THEME).toBe('light');
  });

  it('resolves a brighter background for light than for dark', () => {
    const light = palette('light', DEFAULT_ACCENT);
    const dark = palette('dark', DEFAULT_ACCENT);
    expect(light.bg).not.toBe(dark.bg);
    // every palette key must be a non-empty colour so components never read undefined
    for (const value of Object.values(light)) {
      expect(typeof value).toBe('string');
      expect(value.length).toBeGreaterThan(0);
    }
    expect(ACCENT_KEYS).toContain(DEFAULT_ACCENT);
  });
});

describe('accent selection', () => {
  it('changes only the accent, leaving every base colour driven by the theme', () => {
    const teal = palette('light', 'teal');
    const pink = palette('light', 'pink');
    // the accent itself differs...
    expect(teal.accent).not.toBe(pink.accent);
    expect(teal.accent).toBe(accentColor('light', 'teal'));
    // ...but switching accent leaves the base palette untouched
    const { accent: _t, ...tealBase } = teal;
    const { accent: _p, ...pinkBase } = pink;
    expect(tealBase).toEqual(pinkBase);
  });

  it('resolves a theme-specific shade for the same accent key', () => {
    expect(palette('light', 'teal').accent).not.toBe(palette('dark', 'teal').accent);
    for (const key of ACCENT_KEYS) {
      expect(palette('light', key).accent).toBe(accentColor('light', key));
      expect(palette('dark', key).accent).toBe(accentColor('dark', key));
    }
  });
});

describe('serializeTheme / deserializeTheme', () => {
  it('round-trips a preference back to an equal preference', () => {
    const prefs: ThemePrefs = { theme: 'dark', accent: 'violet' };
    expect(deserializeTheme(serializeTheme(prefs))).toEqual(prefs);
  });

  it('falls back to the light/teal default when storage is empty (null)', () => {
    expect(deserializeTheme(null)).toEqual(DEFAULTS);
  });

  it('falls back to defaults on malformed JSON', () => {
    expect(deserializeTheme('{not json')).toEqual(DEFAULTS);
  });

  it('falls back to defaults when the payload is not an object', () => {
    expect(deserializeTheme('"light"')).toEqual(DEFAULTS);
  });

  it('falls back to defaults for an unknown theme or accent', () => {
    expect(deserializeTheme('{"theme":"sepia","accent":"teal"}')).toEqual(DEFAULTS);
    expect(deserializeTheme('{"theme":"dark","accent":"chartreuse"}')).toEqual(DEFAULTS);
  });
});

describe('loadTheme / saveTheme', () => {
  it('round-trips preferences through a storage backend', () => {
    const storage = fakeStorage();
    const prefs: ThemePrefs = { theme: 'dark', accent: 'pink' };
    saveTheme(storage, prefs);
    expect(loadTheme(storage)).toEqual(prefs);
  });

  it('returns the defaults when nothing has been saved', () => {
    expect(loadTheme(fakeStorage())).toEqual(DEFAULTS);
  });

  it('returns the defaults when the stored value is corrupt', () => {
    expect(loadTheme(fakeStorage({ 'pop-theme-v1': 'oops' }))).toEqual(DEFAULTS);
  });
});
