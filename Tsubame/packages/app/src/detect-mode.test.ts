import { describe, expect, it } from 'vitest';
import { detectMode, detectModeFromSearch, parseRendererQuery } from './detect-mode.js';

describe('parseRendererQuery', () => {
  it('parses supported renderer query values', () => {
    expect(parseRendererQuery('?renderer=dom')).toBe('dom');
    expect(parseRendererQuery('?renderer=vello')).toBe('vello');
    expect(parseRendererQuery('?renderer=tiny-skia')).toBe('tiny-skia');
    expect(parseRendererQuery('?renderer=vello-cpu')).toBe('vello-cpu');
    expect(parseRendererQuery('?renderer=auto')).toBe('auto');
  });

  it('returns null for missing or unknown values', () => {
    expect(parseRendererQuery('')).toBeNull();
    expect(parseRendererQuery('?renderer=canvas')).toBeNull();
  });
});

describe('detectMode', () => {
  it('selects DOM when renderer=dom', () => {
    expect(detectMode({
      rendererQuery: 'dom',
      hasEditContext: true,
      hasWebGPU: true,
    })).toEqual({ mode: 'DOM', source: 'query', renderer: 'dom' });
  });

  it('forces vello canvas when renderer=vello', () => {
    expect(detectMode({
      rendererQuery: 'vello',
      hasEditContext: false,
      hasWebGPU: false,
    })).toEqual({
      mode: 'Canvas',
      backend: 'vello',
      source: 'query',
      renderer: 'vello',
    });
  });

  it('forces tiny-skia canvas when renderer=tiny-skia', () => {
    expect(detectMode({
      rendererQuery: 'tiny-skia',
      hasEditContext: true,
      hasWebGPU: true,
    })).toEqual({
      mode: 'Canvas',
      backend: 'tiny-skia',
      source: 'query',
      renderer: 'tiny-skia',
    });
  });

  it('forces vello-cpu canvas when renderer=vello-cpu', () => {
    expect(detectMode({
      rendererQuery: 'vello-cpu',
      hasEditContext: true,
      hasWebGPU: false,
    })).toEqual({
      mode: 'Canvas',
      backend: 'vello-cpu',
      source: 'query',
      renderer: 'vello-cpu',
    });
  });

  it('auto-selects DOM when EditContext is unavailable', () => {
    expect(detectMode({
      rendererQuery: null,
      hasEditContext: false,
      hasWebGPU: true,
    })).toEqual({ mode: 'DOM', source: 'auto', renderer: 'auto' });

    expect(detectMode({
      rendererQuery: 'auto',
      hasEditContext: false,
      hasWebGPU: false,
    })).toEqual({ mode: 'DOM', source: 'auto', renderer: 'auto' });
  });

  it('auto-selects vello canvas when EditContext and WebGPU are available', () => {
    expect(detectMode({
      rendererQuery: null,
      hasEditContext: true,
      hasWebGPU: true,
    })).toEqual({
      mode: 'Canvas',
      backend: 'vello',
      source: 'auto',
      renderer: 'auto',
    });
  });

  it('auto-selects tiny-skia canvas when EditContext exists but WebGPU does not', () => {
    expect(detectMode({
      rendererQuery: null,
      hasEditContext: true,
      hasWebGPU: false,
    })).toEqual({
      mode: 'Canvas',
      backend: 'tiny-skia',
      source: 'auto',
      renderer: 'auto',
    });
  });
});

describe('detectModeFromSearch', () => {
  it('reads renderer query from search string', () => {
    expect(detectModeFromSearch('?renderer=dom', {
      hasEditContext: true,
      hasWebGPU: true,
    }).mode).toBe('DOM');
  });
});
