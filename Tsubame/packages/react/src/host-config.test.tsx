import { describe, it, expect } from 'vitest';
import {
  RecordingRenderer,
  EVENT_PROP as PROTOCOL_EVENT_PROP,
  REJECTED_EVENT_PROPS as PROTOCOL_REJECTED_EVENT_PROPS,
  type RecordedCall,
} from '@tsubame/renderer-protocol';
import type { ReactNode } from 'react';
import { createTsubameRoot, renderTsubame } from './mount.js';
import { applyProp } from './props.js';
import { createInstance } from './instance.js';
import { EVENT_PROP, REJECTED_EVENT_PROPS } from './events.js';

/**
 * React を {@link RecordingRenderer} へ同期 flush する薄い mount ヘルパ。
 * テストは具象 DOM/WASM に踏み込まず、`IRenderer` 境界に記録された呼び出し列
 * （`recorder.calls`）だけを assert する（ADR-0008）。
 */
function mount(element: ReactNode) {
  const recorder = new RecordingRenderer();
  const root = createTsubameRoot(recorder);
  root.render(element);
  return {
    recorder,
    calls: recorder.calls,
    render: (el: ReactNode) => root.render(el),
    unmount: () => root.unmount(),
  };
}

/** 記録列から `method` の呼び出しだけを順序通りに取り出す。 */
function only<M extends RecordedCall['method']>(
  calls: readonly RecordedCall[],
  method: M,
): Array<Extract<RecordedCall, { method: M }>> {
  return calls.filter((c): c is Extract<RecordedCall, { method: M }> => c.method === method);
}

/** id → 生成された element の kind。React は子 fiber を先に完了するため生成順は前後する。 */
function kindById(calls: readonly RecordedCall[]): Map<number, string> {
  const m = new Map<number, string>();
  for (const c of only(calls, 'createElement')) m.set(c.id, c.kind);
  return m;
}

describe('@tsubame/react host config (IRenderer boundary)', () => {
  it('mounts <view/> as root view + child createElement + appendChild', () => {
    const { calls } = mount(<view />);

    // root view (setRoot 対象) と <view/> の 2 要素が view として作られる
    const created = only(calls, 'createElement');
    expect(created.map((c) => c.kind)).toEqual(['view', 'view']);

    const rootId = created[0]!.id;
    const childId = created[1]!.id;

    expect(only(calls, 'setRoot')).toEqual([{ method: 'setRoot', id: rootId }]);
    expect(only(calls, 'appendChild')).toEqual([
      { method: 'appendChild', parent: rootId, child: childId },
    ]);
  });

  it('renders nested text as a child text element with setText (ADR-0058)', () => {
    const { calls } = mount(
      <view>
        <text>hi</text>
      </view>,
    );
    const kinds = kindById(calls);

    // "hi" は独立した text element として setText される（要素内蔵テキストではない）
    const setTexts = only(calls, 'setText');
    expect(setTexts).toHaveLength(1);
    const labelId = setTexts[0]!.id;
    expect(setTexts[0]!.text).toBe('hi');
    expect(kinds.get(labelId)).toBe('text');

    // label は <text> element（kind: text）の子として append される
    const append = only(calls, 'appendChild').find((c) => c.child === labelId);
    expect(append).toBeDefined();
    expect(kinds.get(append!.parent)).toBe('text');
  });

  it('renders a button label as a child text element (ADR-0058)', () => {
    const { calls } = mount(<button>OK</button>);
    const kinds = kindById(calls);

    const setTexts = only(calls, 'setText');
    expect(setTexts).toHaveLength(1);
    const labelId = setTexts[0]!.id;
    expect(setTexts[0]!.text).toBe('OK');
    expect(kinds.get(labelId)).toBe('text');

    // button 直下ラベルも子 text element になる（要素内蔵テキストにしない）
    const append = only(calls, 'appendChild').find((c) => c.child === labelId);
    expect(append).toBeDefined();
    expect(kinds.get(append!.parent)).toBe('button');
  });
});

/** key 付きリスト。各 item は `<text>{value}</text>`（ラベルは子 text element）。 */
function List({ items }: { items: readonly string[] }) {
  return (
    <view>
      {items.map((i) => (
        <text key={i}>{i}</text>
      ))}
    </view>
  );
}

/** value のラベルを内包する `<text>` ラッパー element の id を、記録列全体から引く。 */
function wrapperIdFor(calls: readonly RecordedCall[], value: string): number | undefined {
  const label = only(calls, 'setText').find((c) => c.text === value);
  if (!label) return undefined;
  const link = calls.find(
    (c): c is Extract<RecordedCall, { method: 'appendChild' | 'insertBefore' }> =>
      (c.method === 'appendChild' || c.method === 'insertBefore') && c.child === label.id,
  );
  return link?.parent;
}

describe('@tsubame/react keyed list reconciliation (IRenderer boundary)', () => {
  it('appends/inserts list items via appendChild / insertBefore', () => {
    const m = mount(<List items={['a', 'b']} />);
    const idA = wrapperIdFor(m.calls, 'a')!;
    const idB = wrapperIdFor(m.calls, 'b')!;
    const listView = only(m.calls, 'appendChild').find((c) => c.child === idA)!.parent;

    const mark = m.calls.length;
    m.render(<List items={['a', 'c', 'b']} />);
    const since = m.calls.slice(mark);
    const idC = wrapperIdFor(m.calls, 'c')!;

    // 'c' は a と b の間に insertBefore で挿入される（既存要素は作り直さない）
    expect(only(since, 'createElement').map((c) => c.kind)).toContain('text');
    expect(only(since, 'insertBefore')).toContainEqual({
      method: 'insertBefore',
      parent: listView,
      child: idC,
      before: idB,
    });
    // 既存 a / b の wrapper は再生成されない
    expect(only(since, 'createElement').some((c) => c.id === idA || c.id === idB)).toBe(false);
  });

  it('reorders list items by moving existing elements (no recreate/remove)', () => {
    const m = mount(<List items={['a', 'b', 'c']} />);
    const idA = wrapperIdFor(m.calls, 'a')!;
    const idB = wrapperIdFor(m.calls, 'b')!;
    const idC = wrapperIdFor(m.calls, 'c')!;
    const listView = only(m.calls, 'appendChild').find((c) => c.child === idA)!.parent;

    const mark = m.calls.length;
    m.render(<List items={['c', 'a', 'b']} />);
    const since = m.calls.slice(mark);

    // 並べ替えは既存要素の移動だけで表現される（作り直しも削除もしない）
    expect(only(since, 'createElement')).toHaveLength(0);
    expect(only(since, 'removeChild')).toHaveLength(0);

    // 移動（appendChild / insertBefore）は既存 wrapper id のみを listView 上で対象にする
    const existing = new Set([idA, idB, idC]);
    const moves = [...only(since, 'appendChild'), ...only(since, 'insertBefore')];
    expect(moves.length).toBeGreaterThan(0);
    for (const mv of moves) {
      expect(mv.parent).toBe(listView);
      expect(existing.has(mv.child)).toBe(true);
    }
  });

  it('removes a deleted list item via removeChild', () => {
    const m = mount(<List items={['a', 'b', 'c']} />);
    const idB = wrapperIdFor(m.calls, 'b')!;
    const listView = only(m.calls, 'appendChild').find((c) => c.child === idB)!.parent;

    const mark = m.calls.length;
    m.render(<List items={['a', 'c']} />);
    const since = m.calls.slice(mark);

    // 'b' の wrapper だけが removeChild される
    expect(only(since, 'removeChild')).toEqual([
      { method: 'removeChild', parent: listView, child: idB },
    ]);
  });
});

describe('@tsubame/react conditional rendering (IRenderer boundary)', () => {
  it('removes an element via removeChild when the condition turns false', () => {
    function Conditional({ show }: { show: boolean }) {
      return (
        <view>
          <text key="always">x</text>
          {show && <text key="cond">y</text>}
        </view>
      );
    }

    const m = mount(<Conditional show={true} />);
    const condId = wrapperIdFor(m.calls, 'y')!;
    const listView = only(m.calls, 'appendChild').find((c) => c.child === condId)!.parent;

    const mark = m.calls.length;
    m.render(<Conditional show={false} />);
    const since = m.calls.slice(mark);

    expect(only(since, 'removeChild')).toEqual([
      { method: 'removeChild', parent: listView, child: condId },
    ]);
  });
});

describe('@tsubame/react element vocabulary (TSX typing, react-jsx)', () => {
  it('types and creates all Element-vocabulary intrinsics', () => {
    // 6 つの Element 語彙が標準 react-jsx でコンパイルでき、それぞれの kind で
    // createElement される（型が通ること自体が IntrinsicElements 拡張の検証）。
    const { calls } = mount(
      <view>
        <text>t</text>
        <button>b</button>
        <text-input value="v" placeholder="p" />
        <scroll-view />
        <image src="x.png" />
      </view>,
    );

    const created = only(calls, 'createElement').map((c) => c.kind);
    for (const kind of ['view', 'text', 'button', 'text-input', 'scroll-view', 'image']) {
      expect(created).toContain(kind);
    }
  });

  it('subscribes onClick via IRenderer.addEventListener', () => {
    const { calls } = mount(<view onClick={() => {}} />);

    const created = kindById(calls);
    const subs = only(calls, 'addEventListener');
    expect(subs).toHaveLength(1);
    expect(subs[0]!.event).toBe('click');
    expect(created.get(subs[0]!.id)).toBe('view');
  });
});

describe('@tsubame/react style channels (IRenderer boundary, ADR-0008)', () => {
  it('records the base style as setStyle', () => {
    // backgroundColor / width はどちらも text-local ではないので view にそのまま適用される。
    const { calls } = mount(<view style={{ width: 120, backgroundColor: '#222' }} />);
    const kinds = kindById(calls);

    // root view は style を持たない。style を運ぶ <view/> の setStyle だけが記録される。
    const styled = only(calls, 'setStyle');
    expect(styled).toHaveLength(1);
    expect(kinds.get(styled[0]!.id)).toBe('view');
    expect(styled[0]!.style).toEqual({ width: 120, backgroundColor: '#222' });
  });

  it('splits :hover / :active / :focus blocks into setPseudoStyle', () => {
    // base は setStyle、擬似クラスブロックは setPseudoStyle に分解される（splitHayateStyle）。
    const { calls } = mount(
      <view
        style={{
          width: 100,
          ':hover': { backgroundColor: '#333' },
          ':active': { backgroundColor: '#444' },
          ':focus': { backgroundColor: '#555' },
        }}
      />,
    );
    const kinds = kindById(calls);

    const base = only(calls, 'setStyle');
    expect(base).toHaveLength(1);
    expect(base[0]!.style).toEqual({ width: 100 });
    const styledId = base[0]!.id;
    expect(kinds.get(styledId)).toBe('view');

    const pseudo = only(calls, 'setPseudoStyle').filter((c) => c.id === styledId);
    expect(pseudo).toEqual([
      { method: 'setPseudoStyle', id: styledId, pseudo: ':hover', style: { backgroundColor: '#333' } },
      { method: 'setPseudoStyle', id: styledId, pseudo: ':active', style: { backgroundColor: '#444' } },
      { method: 'setPseudoStyle', id: styledId, pseudo: ':focus', style: { backgroundColor: '#555' } },
    ]);
  });

  it('forwards each styleVariants entry to setStyleVariant (ADR-0081)', () => {
    const { calls } = mount(
      <view
        styleVariants={[
          { condition: { maxWidth: 720 }, style: { flexDirection: 'column' } },
          { condition: { minWidth: 1100 }, style: { gap: 24 } },
        ]}
      />,
    );
    const kinds = kindById(calls);

    const variants = only(calls, 'setStyleVariant');
    expect(variants).toHaveLength(2);
    const styledId = variants[0]!.id;
    expect(kinds.get(styledId)).toBe('view');
    expect(variants).toEqual([
      {
        method: 'setStyleVariant',
        id: styledId,
        condition: { maxWidth: 720 },
        style: { flexDirection: 'column' },
      },
      {
        method: 'setStyleVariant',
        id: styledId,
        condition: { minWidth: 1100 },
        style: { gap: 24 },
      },
    ]);
  });

  it('applies style to text elements (ADR-0058: text も Hayate element)', () => {
    // color / fontSize は text-local だが text は carrier なのでそのまま適用される。
    const { calls } = mount(<text style={{ color: '#4fd1c5', fontSize: 22 }}>hi</text>);
    const kinds = kindById(calls);

    // ラベル "hi" を内包する <text> ラッパー element に style が乗る。
    const wrapperId = wrapperIdFor(calls, 'hi')!;
    expect(kinds.get(wrapperId)).toBe('text');

    const styled = only(calls, 'setStyle').filter((c) => c.id === wrapperId);
    expect(styled).toHaveLength(1);
    expect(styled[0]!.style).toEqual({ color: '#4fd1c5', fontSize: 22 });
  });
});

/** `kind` の element として createElement された唯一の id を引く。 */
function soleIdOfKind(calls: readonly RecordedCall[], kind: string): number {
  const created = only(calls, 'createElement').filter((c) => c.kind === kind);
  expect(created).toHaveLength(1);
  return created[0]!.id;
}

describe('@tsubame/react element properties (IRenderer boundary, ADR-0071)', () => {
  it('records text-input semantic props (value/placeholder/disabled/multiline) as setProperty', () => {
    const { calls } = mount(<text-input value="hi" placeholder="名前" disabled multiline />);
    const inputId = soleIdOfKind(calls, 'text-input');

    // 閉じたセマンティック prop は IRenderer.setProperty として流れる（再実装しない）。
    const props = only(calls, 'setProperty').filter((c) => c.id === inputId);
    expect(props).toEqual([
      { method: 'setProperty', id: inputId, name: 'value', value: 'hi' },
      { method: 'setProperty', id: inputId, name: 'placeholder', value: '名前' },
      { method: 'setProperty', id: inputId, name: 'disabled', value: true },
      { method: 'setProperty', id: inputId, name: 'multiline', value: true },
    ]);
  });

  it('records image src as setProperty', () => {
    const { calls } = mount(<image src="logo.png" />);
    const imageId = soleIdOfKind(calls, 'image');

    expect(only(calls, 'setProperty').filter((c) => c.id === imageId)).toEqual([
      { method: 'setProperty', id: imageId, name: 'src', value: 'logo.png' },
    ]);
  });

  it('records user-select as setProperty', () => {
    const { calls } = mount(<view user-select="none" />);
    // root view 自体は prop を持たない。user-select を運ぶ子 <view/> の id を引く。
    const rootId = only(calls, 'setRoot')[0]!.id;
    const viewId = only(calls, 'createElement').find(
      (c) => c.kind === 'view' && c.id !== rootId,
    )!.id;

    expect(only(calls, 'setProperty').filter((c) => c.id === viewId)).toEqual([
      { method: 'setProperty', id: viewId, name: 'user-select', value: 'none' },
    ]);
  });

  it('renders children as elements, not a "children" property', () => {
    const { calls } = mount(
      <view>
        <text>child</text>
      </view>,
    );

    // children は独立した text element になり、'children' setProperty にはならない。
    expect(only(calls, 'setProperty').map((c) => c.name)).not.toContain('children');
    expect(only(calls, 'setText').map((c) => c.text)).toContain('child');
  });

  it('ignores structural props (children/ref/key) — no setProperty', () => {
    // children / ref / key はホストへ流さない。ref は React が host instance を渡して
    // 呼ぶため通常 props には現れないが、流れても renderer には漏れないことを継ぎ目で
    // 直接確認する（ADR-0008）。
    const recorder = new RecordingRenderer();
    const id = recorder.createElement('view');
    const instance = createInstance(id, 'view');

    applyProp(recorder, instance, 'children', [{}]);
    applyProp(recorder, instance, 'ref', () => {});
    applyProp(recorder, instance, 'key', 'k');

    expect(only(recorder.calls, 'setProperty')).toHaveLength(0);
  });

  it('throws on an unknown element property (ADR-0071)', () => {
    // 未知 prop の判定は renderer-protocol の assertKnownElementProperty に委ねており
    // （react 側で再実装しない）、それが throw する。React の render は host エラーを
    // error logger に流して握りつぶすため、prop 適用の継ぎ目を IRenderer 越しに直接
    // 突いて throw を検証する（tsubame-solid の setProp テストと対称、ADR-0008）。
    const recorder = new RecordingRenderer();
    const id = recorder.createElement('view');
    const instance = createInstance(id, 'view');

    expect(() => applyProp(recorder, instance, 'id', 'x')).toThrow(
      /Unknown element property "id".*ADR-0071/,
    );
    // 拒否された prop は setProperty として記録されない。
    expect(only(recorder.calls, 'setProperty')).toHaveLength(0);
  });
});

describe('@tsubame/react event handling (IRenderer boundary, ADR-0008)', () => {
  it('maps every EVENT_PROP entry to addEventListener with its EventKind', () => {
    // onClick/onInput/onKeyDown/onFocus/onBlur は renderer-protocol の EVENT_PROP で
    // EventKind に対応し、IRenderer.addEventListener として記録される（react 側で語彙を
    // 再定義しない、#483）。
    const { calls } = mount(
      <view
        onClick={() => {}}
        onInput={() => {}}
        onKeyDown={() => {}}
        onFocus={() => {}}
        onBlur={() => {}}
      />,
    );
    const kinds = kindById(calls);

    const subs = only(calls, 'addEventListener');
    // 全リスナは style を運ぶ単一の <view/>（root view ではない）に乗る。
    const targetId = subs[0]!.id;
    expect(kinds.get(targetId)).toBe('view');
    for (const s of subs) expect(s.id).toBe(targetId);

    expect(subs.map((s) => s.event).sort()).toEqual(
      ['blur', 'click', 'focus', 'input', 'keydown'].sort(),
    );
  });

  it('re-registers on handler change: old listener unsubscribed, no double registration', () => {
    const m = mount(<view onClick={() => {}} />);
    const targetId = only(m.calls, 'addEventListener')[0]!.id;

    const mark = m.calls.length;
    m.render(<view onClick={() => {}} />);
    const since = m.calls.slice(mark);

    // ハンドラが変わると commitUpdate が旧購読を解除してから 1 度だけ再登録する。
    expect(only(since, 'removeEventListener')).toEqual([
      { method: 'removeEventListener', id: targetId, event: 'click' },
    ]);
    expect(only(since, 'addEventListener')).toEqual([
      { method: 'addEventListener', id: targetId, event: 'click' },
    ]);

    // 最終的に click のアクティブリスナは 1 つ（二重登録していない）。
    const adds = only(m.calls, 'addEventListener').filter(
      (c) => c.id === targetId && c.event === 'click',
    );
    const removes = only(m.calls, 'removeEventListener').filter(
      (c) => c.id === targetId && c.event === 'click',
    );
    expect(adds.length - removes.length).toBe(1);
  });

  it('unsubscribes the listener when the event prop is removed', () => {
    const m = mount(<view onClick={() => {}} />);
    const targetId = only(m.calls, 'addEventListener')[0]!.id;

    const mark = m.calls.length;
    m.render(<view />);
    const since = m.calls.slice(mark);

    // prop から外れたら解除のみ。再登録はしない。
    expect(only(since, 'removeEventListener')).toEqual([
      { method: 'removeEventListener', id: targetId, event: 'click' },
    ]);
    expect(only(since, 'addEventListener')).toHaveLength(0);
  });

  it('unsubscribes an element listener when the element itself is removed', () => {
    function Conditional({ show }: { show: boolean }) {
      return <view>{show && <view onClick={() => {}} />}</view>;
    }

    const m = mount(<Conditional show={true} />);
    const targetId = only(m.calls, 'addEventListener')[0]!.id;

    const mark = m.calls.length;
    m.render(<Conditional show={false} />);
    const since = m.calls.slice(mark);

    // 要素ごと消えると detachDeletedInstance が自要素のリスナを解除する
    // （構造は backend の removeChild に委ねる）。
    expect(only(since, 'removeEventListener')).toEqual([
      { method: 'removeEventListener', id: targetId, event: 'click' },
    ]);
  });

  it.each(['onHoverEnter', 'onHoverLeave'])(
    'throws on %s and never subscribes it (ADR-0059)',
    (rejected) => {
      // ホバーは REJECTED_EVENT_PROPS（renderer-protocol 由来、#483）。視覚ホバーは
      // style の :hover で表現するため、購読要求は明確なエラーで弾く。React の render は
      // host エラーを握りつぶすので、prop 適用の継ぎ目を IRenderer 越しに直接突く
      // （unknown-prop テストと対称、ADR-0008）。
      const recorder = new RecordingRenderer();
      const id = recorder.createElement('view');
      const instance = createInstance(id, 'view');

      expect(() => applyProp(recorder, instance, rejected, () => {})).toThrow(
        new RegExp(`${rejected} is not supported`),
      );
      // 拒否された prop は addEventListener として記録されない。
      expect(only(recorder.calls, 'addEventListener')).toHaveLength(0);
    },
  );

  it('reuses the protocol event vocabulary (no react-side redefinition, #483)', () => {
    // react の events.ts は renderer-protocol の語彙を再 export するだけ。同一参照で
    // あることを確認し、solid とドリフトする独自定義を持ち込んでいないことを固定する。
    expect(EVENT_PROP).toBe(PROTOCOL_EVENT_PROP);
    expect(REJECTED_EVENT_PROPS).toBe(PROTOCOL_REJECTED_EVENT_PROPS);
  });
});

describe('@tsubame/react renderTsubame lifecycle', () => {
  it('mounts the root view and returns a dispose that tears down the tree', () => {
    const recorder = new RecordingRenderer();
    const dispose = renderTsubame(<view />, recorder);

    const created = only(recorder.calls, 'createElement');
    const rootId = created[0]!.id;
    const childId = created[1]!.id;
    expect(only(recorder.calls, 'setRoot')).toEqual([{ method: 'setRoot', id: rootId }]);

    const mark = recorder.calls.length;
    dispose();
    const since = recorder.calls.slice(mark);

    // dispose は描画済みツリーを container（root view）から取り除く
    expect(only(since, 'removeChild')).toEqual([
      { method: 'removeChild', parent: rootId, child: childId },
    ]);
  });
});
