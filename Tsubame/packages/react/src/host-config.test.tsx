import { describe, it, expect } from 'vitest';
import { RecordingRenderer, type RecordedCall } from '@tsubame/renderer-protocol';
import type { ReactNode } from 'react';
import { createTsubameRoot, renderTsubame } from './mount.js';

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
