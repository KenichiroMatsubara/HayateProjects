import { afterEach, describe, expect, it } from 'vitest';
import {
  findElementByText,
  mountGoldenFrameParity,
  type GoldenFrameParityHarness,
} from './test-helpers/golden-frame-parity-harness.js';

describe('golden frame semantic parity (ADR-0079, #151)', () => {
  let harness: GoldenFrameParityHarness | null = null;

  afterEach(() => {
    harness?.dispose();
    harness = null;
  });

  it('defaultColor on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultColor: '#ff6600',
      });
      setText(text, 'ambient');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'ambient');
    expect(text?.visual?.textColor?.r).toBe(1);
    expect(text?.visual?.textColor?.g).toBeCloseTo(102 / 255, 5);
    expect(text?.visual?.textColor?.b).toBe(0);
    expect(frame).toMatchSnapshot();
  });

  it('block box color and fontSize do not leak to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        color: '#ff0000',
        fontSize: 24,
      });
      setText(text, 'child');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'child');
    expect(text?.visual?.textColor).toEqual({ r: 0, g: 0, b: 0, a: 1 });
    expect(text?.visual?.fontSize).toBe(16);
    expect(frame).toMatchSnapshot();
  });

  it('text-local color and fontSize apply on the text element', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { color: '#00ff00', fontSize: 20 });
      setText(text, 'styled');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'styled');
    expect(text?.visual?.textColor?.g).toBe(1);
    expect(text?.visual?.fontSize).toBe(20);
    expect(frame).toMatchSnapshot();
  });

  it('nested text elements inherit parent text styles in IFC', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const outer = createElement('text');
      const inner = createElement('text');
      insertNode(view, outer);
      insertNode(outer, inner);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        color: '#ff0000',
        fontSize: 32,
      });
      setProp(outer, 'style', { color: '#336699', fontSize: 18 });
      setText(outer, 'Hi ');
      setText(inner, 'there');
      return view;
    });

    const frame = harness.capture();
    const inner = findElementByText(frame, 'there');
    expect(inner?.visual?.fontSize).toBe(18);
    expect(inner?.visual?.textColor?.b).toBeCloseTo(153 / 255, 5);
    expect(frame).toMatchSnapshot();
  });

  it('fontWeight 600 resolves on text for variable-font synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontWeight: 600, fontSize: 24 });
      setText(text, 'w600');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'w600');
    expect(text?.visual?.fontWeight).toBe(600);
    expect(frame).toMatchSnapshot();
  });

  it('fontStyle italic resolves on text for faux-italic synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontStyle: 'italic', fontSize: 24 });
      setText(text, 'italic');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'italic');
    expect(text?.visual?.fontStyle).toBe('italic');
    expect(frame).toMatchSnapshot();
  });

  it('fontWeight 700 resolves on text for bold synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontWeight: 700, fontSize: 24 });
      setText(text, 'bold');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'bold');
    expect(text?.visual?.fontWeight).toBe(700);
    expect(frame).toMatchSnapshot();
  });

  it('flexWrap wrap places overflow flex children on a second row', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp }) => {
      const row = createElement('view');
      setProp(row, 'style', {
        display: 'flex',
        flexWrap: 'wrap',
        width: '70px',
        gap: 0,
      });
      for (let i = 0; i < 3; i++) {
        const child = createElement('view');
        insertNode(row, child);
        setProp(child, 'style', { width: '40px', height: '15px' });
      }
      return row;
    });

    const frame = harness.capture();
    const children = frame.elements
      .filter((el) => el.bounds[2] === 40 && el.bounds[3] === 15)
      .sort((a, b) => a.bounds[0]! - b.bounds[0]! || a.bounds[1]! - b.bounds[1]!);
    expect(children).toHaveLength(3);
    expect(children[2]!.bounds[1]).toBeGreaterThan(children[0]!.bounds[1]!);
    expect(frame).toMatchSnapshot();
  });

  it('aspect-ratio derives a flex item height from its width (#490)', async () => {
    // align-self: flex-start で交差軸 stretch を切り、高さを auto に保つ。aspect-ratio が
    // width / ratio で高さを解決する。WASM 解決の Canvas 経路で固定し、DOM はネイティブ
    // CSS `aspect-ratio` で同値を得る（hayate-css-parity が入力の単一ソース性を固定）。
    const BOX_WIDTH = 80;
    const ASPECT = 2; // width / height
    const EXPECTED_HEIGHT = BOX_WIDTH / ASPECT; // 40
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp }) => {
      const row = createElement('view');
      setProp(row, 'style', { display: 'flex', width: '200px', height: '100px' });
      const box = createElement('view');
      insertNode(row, box);
      setProp(box, 'style', {
        width: `${BOX_WIDTH}px`,
        alignSelf: 'flex-start',
        aspectRatio: ASPECT,
      });
      return row;
    });

    const frame = harness.capture();
    const box = frame.elements.find((el) => el.bounds[2] === BOX_WIDTH);
    expect(box, 'aspect-ratio box must be present').toBeDefined();
    // 高さは width / ratio = 80 / 2 = 40。スナップショットではなく解決幾何を直接固定する
    // （bounds = [x, y, width, height]）。
    expect(box!.bounds[3]).toBeCloseTo(EXPECTED_HEIGHT, 1);
  });

  it('grid-auto-rows sizes the implicit row beyond the explicit track (#492)', async () => {
    // 明示行を 1 つだけ定義し、2 つ目のアイテムを暗黙行へあふれさせる。暗黙行の高さは
    // grid-auto-rows が決める。WASM 解決の Canvas 経路で固定し、DOM はネイティブ CSS
    // `grid-auto-rows` で同値を得る（hayate-css-parity が入力の単一ソース性を固定）。
    const EXPLICIT_ROW = 50;
    const AUTO_ROW = 30;
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp }) => {
      const grid = createElement('view');
      setProp(grid, 'style', {
        display: 'grid',
        gridTemplateColumns: ['1fr'],
        gridTemplateRows: [`${EXPLICIT_ROW}px`],
        gridAutoRows: [`${AUTO_ROW}px`],
        width: '100px',
        height: '100px',
      });
      const first = createElement('view');
      const second = createElement('view');
      insertNode(grid, first);
      insertNode(grid, second);
      setProp(first, 'style', { backgroundColor: '#ff0000' });
      setProp(second, 'style', { backgroundColor: '#0000ff' });
      return grid;
    });

    const frame = harness.capture();
    // 暗黙行のアイテムは grid-auto-rows = 30 の高さで、明示行 (50) の直下 y=50 に置かれる。
    // 高さ 30 のボックスは他に存在しないので、これで一意に特定できる。
    const implicit = frame.elements.find((el) => Math.abs(el.bounds[3] - AUTO_ROW) < 1);
    expect(implicit, 'implicit-row item must be present').toBeDefined();
    expect(implicit!.bounds[1]).toBeCloseTo(EXPLICIT_ROW, 0);
    expect(implicit!.bounds[3]).toBeCloseTo(AUTO_ROW, 0);
  });

  it('grid-auto-flow column fills columns before rows (#493)', async () => {
    // 2×2 の明示グリッドに 3 アイテムを column フローで自動配置する。column は列を端から
    // 埋めるので 2 番目のアイテムは (col0,row1) = 下段左へ落ちる（row フローなら上段右）。
    // WASM 解決の Canvas 経路で固定し、DOM はネイティブ CSS `grid-auto-flow: column` で
    // 同じ配置を得る（hayate-css-parity が入力の単一ソース性を固定）。2 番目のアイテムは
    // テキストプローブで一意に特定する。
    const TRACK = 40;
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const grid = createElement('view');
      setProp(grid, 'style', {
        display: 'grid',
        gridTemplateColumns: [`${TRACK}px`, `${TRACK}px`],
        gridTemplateRows: [`${TRACK}px`, `${TRACK}px`],
        gridAutoFlow: 'column',
        width: `${TRACK * 2}px`,
        height: `${TRACK * 2}px`,
      });
      const first = createElement('view');
      const second = createElement('view');
      const third = createElement('view');
      insertNode(grid, first);
      insertNode(grid, second);
      insertNode(grid, third);
      setProp(first, 'style', { backgroundColor: '#ff0000' });
      setProp(second, 'style', { backgroundColor: '#0000ff' });
      setProp(third, 'style', { backgroundColor: '#00ff00' });
      const probe = createElement('text');
      insertNode(second, probe);
      setText(probe, 'cflow');
      return grid;
    });

    const frame = harness.capture();
    const probe = findElementByText(frame, 'cflow');
    expect(probe, 'second-item probe must be present').toBeDefined();
    // 下段左セル: x は左列（< 1 トラック分）、y は下段（≈ 1 トラック分）。
    expect(probe!.bounds[0]).toBeLessThan(TRACK);
    expect(probe!.bounds[1]).toBeGreaterThan(TRACK / 2);
  });

  it('justify-items / justify-self align grid items on the inline axis (#494)', async () => {
    // 1 セル（TRACK 角）の grid に、セルより小さい TRACK/3 角のアイテムを 2 つ別々の
    // grid で置く。コンテナ既定 justify-items: end は左端のアイテムをインライン終端へ
    // 寄せ（x ≈ TRACK - ITEM）、アイテムの justify-self: start はそれを上書きして始端へ
    // 戻す（x ≈ 0）。WASM 解決の Canvas 経路で固定し、DOM はネイティブ CSS で同じ配置を
    // 得る（hayate-css-parity が入力の単一ソース性を固定）。各アイテムはテキストプローブで
    // 一意に特定する。
    const TRACK = 60;
    const ITEM = 20;
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      // 2 つの grid を縦積みにして両方の絶対 x 原点を 0 に揃える（インライン軸の差だけを見る）。
      const row = createElement('view');
      setProp(row, 'style', { display: 'flex', flexDirection: 'column' });

      // コンテナ既定（justify-items: end）に従うアイテム。
      const endGrid = createElement('view');
      setProp(endGrid, 'style', {
        display: 'grid',
        gridTemplateColumns: [`${TRACK}px`],
        gridTemplateRows: [`${TRACK}px`],
        justifyItems: 'end',
        width: `${TRACK}px`,
        height: `${TRACK}px`,
      });
      const endItem = createElement('view');
      setProp(endItem, 'style', { width: `${ITEM}px`, height: `${ITEM}px`, backgroundColor: '#ff0000' });
      insertNode(endGrid, endItem);
      const endProbe = createElement('text');
      insertNode(endItem, endProbe);
      setText(endProbe, 'jend');

      // 同じコンテナ既定を justify-self: start で上書きするアイテム。
      const selfGrid = createElement('view');
      setProp(selfGrid, 'style', {
        display: 'grid',
        gridTemplateColumns: [`${TRACK}px`],
        gridTemplateRows: [`${TRACK}px`],
        justifyItems: 'end',
        width: `${TRACK}px`,
        height: `${TRACK}px`,
      });
      const selfItem = createElement('view');
      setProp(selfItem, 'style', {
        width: `${ITEM}px`,
        height: `${ITEM}px`,
        justifySelf: 'start',
        backgroundColor: '#0000ff',
      });
      insertNode(selfGrid, selfItem);
      const selfProbe = createElement('text');
      insertNode(selfItem, selfProbe);
      setText(selfProbe, 'jstart');

      insertNode(row, endGrid);
      insertNode(row, selfGrid);
      return row;
    });

    const frame = harness.capture();
    const endProbe = findElementByText(frame, 'jend');
    const selfProbe = findElementByText(frame, 'jstart');
    expect(endProbe, 'justify-items end probe must be present').toBeDefined();
    expect(selfProbe, 'justify-self start probe must be present').toBeDefined();
    const endX = endProbe!.bounds[0]!;
    const selfX = selfProbe!.bounds[0]!;
    // 縦積みなので両グリッドの絶対 x 原点は 0。justify-items: end はアイテムを終端へ
    // 寄せる（x ≈ TRACK - ITEM）。
    expect(endX).toBeGreaterThan(TRACK - ITEM - 2);
    // justify-self: start はコンテナ既定の end を上書きして始端へ戻す（x ≈ 0）。
    expect(selfX).toBeLessThan(ITEM);
    expect(selfX).toBeLessThan(endX);
  });

  it('grid-column places an item in an explicit column and span widens it (#495)', async () => {
    // 3 列（各 TRACK）の grid に 1 アイテムを gridColumn: 2 / span 2 で明示配置する。
    // 開始は 2 列目（x ≈ TRACK）、span 2 で 2 トラックぶん（幅 ≈ 2*TRACK）占有する。
    // WASM 解決の Canvas 経路で固定し、DOM はネイティブ CSS `grid-column: 2 / span 2`
    // で同じ配置を得る（hayate-css-parity が入力の単一ソース性を固定）。アイテムは
    // テキストプローブで一意に特定する。
    const TRACK = 40;
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const grid = createElement('view');
      setProp(grid, 'style', {
        display: 'grid',
        gridTemplateColumns: [`${TRACK}px`, `${TRACK}px`, `${TRACK}px`],
        gridTemplateRows: [`${TRACK}px`],
        width: `${TRACK * 3}px`,
        height: `${TRACK}px`,
      });
      const item = createElement('view');
      insertNode(grid, item);
      setProp(item, 'style', {
        gridColumn: { start: 2, end: { span: 2 } },
        backgroundColor: '#0000ff',
      });
      const probe = createElement('text');
      insertNode(item, probe);
      setText(probe, 'gcol');
      return grid;
    });

    const frame = harness.capture();
    const probe = findElementByText(frame, 'gcol');
    expect(probe, 'grid-column probe must be present').toBeDefined();
    // 2 列目から始まる: x ≈ 1 トラック。span 2 で内容は 2 トラックぶんの幅に広がる。
    expect(probe!.bounds[0]).toBeGreaterThan(TRACK - 2);
    expect(probe!.bounds[0]).toBeLessThan(TRACK * 2);
  });

  it('box-sizing content-box adds padding outside a flex item width (#491)', async () => {
    // content-box では width は内容箱を指し、padding は外側に足される。WASM 解決の
    // Canvas 経路で外形 = width + 左右 padding を固定し、DOM はネイティブ CSS
    // `box-sizing: content-box` で同じ寸法規約を得る（hayate-css-parity が単一ソース性を固定）。
    // align-self: flex-start で交差軸 stretch を切り、width が支配する状態にする。
    const CONTENT_WIDTH = 80;
    const PADDING = 20;
    const EXPECTED_OUTER_WIDTH = CONTENT_WIDTH + PADDING * 2; // 120
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp }) => {
      const row = createElement('view');
      setProp(row, 'style', { display: 'flex', width: '300px', height: '100px' });
      const box = createElement('view');
      insertNode(row, box);
      setProp(box, 'style', {
        width: `${CONTENT_WIDTH}px`,
        padding: `${PADDING}px`,
        alignSelf: 'flex-start',
        boxSizing: 'content-box',
      });
      return row;
    });

    const frame = harness.capture();
    // 外形 = 80 + 2*20 = 120。スナップショットではなく解決幾何を直接固定する。
    const box = frame.elements.find(
      (el) => Math.round(el.bounds[2]) === EXPECTED_OUTER_WIDTH,
    );
    expect(box, 'content-box box must resolve to padded outer width').toBeDefined();
    expect(box!.bounds[2]).toBeCloseTo(EXPECTED_OUTER_WIDTH, 1);
  });

  it('defaultFontSize on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontSize: 22,
      });
      setText(text, 'sized');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'sized');
    expect(text?.visual?.fontSize).toBe(22);
    expect(frame).toMatchSnapshot();
  });

  it('defaultFontFamily on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontFamily: 'Noto Sans',
      });
      setText(text, 'family');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'family');
    expect(text?.visual?.fontFamily).toBe('Noto Sans');
    expect(frame).toMatchSnapshot();
  });

  it('defaultFontWeight on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontWeight: 600,
      });
      setText(text, 'weighted');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'weighted');
    expect(text?.visual?.fontWeight).toBe(600);
    expect(frame).toMatchSnapshot();
  });
});
