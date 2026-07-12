import { describe, it, expect } from 'vitest';
import type { ElementKind } from '@torimi/tsubame-renderer-protocol';
import { createDomElement } from './dom-elements.js';

const ALL_KINDS: ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

// DOM Renderer の scroll-view はオーバーレイ式スクロールバーを使い、ガターを予約
// しない。Canvas にはスクロールバーガターの概念がなく（overflow は visible | hidden
// のみ）、scroll-view のコンテンツボックスはパディングボックス全体。オーバーレイに
// 固定することで Canvas/DOM のコンテンツボックス幅が一致する（意味論パリティ）。
// ビジュアル正典は DOM（ADR-0102）だが、ガターは予約しない。
describe('scroll-view overlay scrollbars (issue #408)', () => {
  it('reserves no scrollbar gutter — overlay, not classic UA chrome', () => {
    const el = createDomElement(document, 'scroll-view');
    // scrollbar-width: none で従来のガターを除き、スクロール可能でもコンテンツボックス幅を維持する。
    expect(el.style.scrollbarWidth).toBe('none');
  });

  it('stays scrollable as overlay — overflow still scrolls, gutter not reserved', () => {
    const el = createDomElement(document, 'scroll-view');
    // オーバーレイ化のためにスクロール可能性を犠牲にしてはならない（overflow:hidden 等）。
    // overflow はスクロール値のまま。
    expect(el.style.overflow).toBe('auto');
    // ガターは予約しない。scrollbar-gutter: stable はスペースを確保してスクロール時に
    // コンテンツボックスを縮める（解消したい乖離そのもの）ので設定しない。
    expect(el.style.getPropertyValue('scrollbar-gutter')).not.toBe('stable');
    expect(el.style.scrollbarWidth).toBe('none');
  });

  it('matches Canvas content box width — scroll-view is the only scrollable kind, and it scrolls as overlay', () => {
    // overflow の語彙は visible | hidden のみ。Canvas は従来型スクロールバーを持たず
    // ガターを予約しないので、scroll-view のコンテンツボックスはパディングボックス全体。
    // DOM はオーバーレイでスクロールすることで同じコンテンツボックスに到達する。他の種別は
    // スクロールしないため、オーバーレイ扱いを受けるのは scroll-view だけ。
    for (const kind of ALL_KINDS) {
      const el = createDomElement(document, kind);
      if (kind === 'scroll-view') {
        expect(el.style.overflow).toBe('auto');
        expect(el.style.scrollbarWidth).toBe('none');
      } else {
        expect(el.style.overflow).toBe('');
      }
    }
  });
});
