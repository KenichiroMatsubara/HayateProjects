import { describe, expect, it } from 'vitest';
import { HAYATE_CSS_CATALOG } from '@tsubame/hayate-css-catalog';
import { GALLERY_LIVE_PROPERTIES, GALLERY_ROADMAP_PROPERTIES } from './CssGallery';

// The CSS Gallery promises a *live* card for every Hayate CSS property in the
// protocol catalog, so the showcase stays complete as the vocabulary grows.
// These tests guard that promise (issue #246) through the gallery's public
// catalog of what it renders, not its layout details.

const catalogKeys = HAYATE_CSS_CATALOG.map((entry) => entry.patchKey);

describe('CssGallery property coverage', () => {
  it('shows a live card for every property in the Hayate CSS catalog', () => {
    const live = new Set(GALLERY_LIVE_PROPERTIES);
    const missing = catalogKeys.filter((key) => !live.has(key));
    expect(missing).toEqual([]);
  });

  it('lists each catalog property exactly once (header count stays honest)', () => {
    const duplicates = GALLERY_LIVE_PROPERTIES.filter(
      (name, index) => GALLERY_LIVE_PROPERTIES.indexOf(name) !== index,
    );
    expect(duplicates).toEqual([]);
    expect([...GALLERY_LIVE_PROPERTIES].sort()).toEqual([...catalogKeys].sort());
  });

  it('places cursor in the live examples (no longer a roadmap entry)', () => {
    expect(GALLERY_LIVE_PROPERTIES).toContain('cursor');
    expect(GALLERY_ROADMAP_PROPERTIES).not.toContain('cursor');
  });

  it('promotes box-shadow to a live card now that the catalog ships it (#252)', () => {
    expect(catalogKeys).toContain('boxShadow');
    expect(GALLERY_LIVE_PROPERTIES).toContain('boxShadow');
    expect(GALLERY_ROADMAP_PROPERTIES).not.toContain('boxShadow');
  });

  it('only lists genuinely-unimplemented properties on the roadmap', () => {
    const overlap = GALLERY_ROADMAP_PROPERTIES.filter((name) => catalogKeys.includes(name));
    expect(overlap).toEqual([]);
  });

  it('does not list a property as both live and roadmap', () => {
    const live = new Set(GALLERY_LIVE_PROPERTIES);
    const both = GALLERY_ROADMAP_PROPERTIES.filter((name) => live.has(name));
    expect(both).toEqual([]);
  });
});
