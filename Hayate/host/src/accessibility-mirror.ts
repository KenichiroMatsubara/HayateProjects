import type { RawHayate } from './raw-hayate.js';

export type { RawHayate } from './raw-hayate.js';

/**
 * {@link attachAccessibilityMirror} の後始末関数。ミラー root を DOM から除去する。host の
 * ライフサイクル teardown（full reload）から呼ぶ（ADR-0124）。
 */
export type DetachAccessibilityMirror = () => void;

/**
 * attach 済みミラーのハンドル（#645）。ミラーは**独立 rAF ループを持たない** — `poll()` が外から
 * （レンダラのフレーム末尾で）駆動され、レンダラが idle に落ちれば相乗りする tick も完全に止まる
 * （frame-clock がアプリ全体で 1 本になる。診断 要因 1 / ADR-0126）。`detach()` は full reload で呼ぶ。
 */
export interface AccessibilityMirror {
  /**
   * レンダラフレーム 1 回分のミラー同期。`raw.poll_accessibility()` を 1 度引き、非 null かつ前回
   * 適用値と異なるときだけ DOM を投影する（#642 の dirty ゲートで null / 同一なら DOM を触らない）。
   * `createHayateWebHost` がレンダラの各フレーム末尾で 1 回呼ぶ（相乗り）。
   */
  readonly poll: () => void;
  /** ミラー root を DOM から除去する。以後の `poll()` は安全な no-op（#645）。 */
  readonly detach: DetachAccessibilityMirror;
}

/** ミラー root コンテナを識別する属性名。Playwright / テストはこの配下を観測する（ADR-0124）。 */
export const A11Y_ROOT_ATTR = 'data-hayate-a11y';

/**
 * 不可視化の style 値（ADR-0124）。`opacity:0` ＋ `pointer-events:none` で、矩形配置のまま
 * a11y ツリーには残しつつ描画と入力からは外す（`display:none`/`visibility:hidden` は a11y
 * ツリーから消えるため不可）。座標クリックはミラーを素通りして下の `<canvas>` に届く。
 */
export const MIRROR_OPACITY = '0';
export const MIRROR_POINTER_EVENTS = 'none';

/**
 * 投影ノードの DOM `id` の接頭辞。`<接頭辞><NodeId>` で一意 id を振り、root の
 * `aria-activedescendant` が focus ノードを指せるようにする（ADR-0124 / #594）。
 */
export const A11Y_NODE_ID_PREFIX = 'hayate-a11y-node-';

/**
 * AccessKit `Role`（accesskit 0.24 serde camelCase 文字列）→ ARIA role 文字列の写像表。
 * Core の `poll_accessibility()` が出す role を Playwright の `getByRole` が引ける ARIA role に
 * 写す。表に無い role は generic（role 属性なし）として投影し、name/value/構造は保つ。
 */
export const ACCESSKIT_ROLE_TO_ARIA: Record<string, string> = {
  button: 'button',
  textInput: 'textbox',
  label: '',
  list: 'list',
  listItem: 'listitem',
  image: 'img',
  link: 'link',
  heading: 'heading',
  navigation: 'navigation',
  main: 'main',
  dialog: 'dialog',
  alertDialog: 'alertdialog',
  scrollView: '',
  genericContainer: '',
  window: '',
};

/** AccessKit `Node`（serde）の、ミラーが消費する部分形。 */
interface AccessKitNode {
  readonly role: string;
  readonly properties?: {
    readonly children?: readonly number[];
    readonly label?: string | null;
    readonly value?: string | null;
    readonly bounds?: AccessKitRect | null;
  };
}

/** AccessKit `Rect`（serde）: 角の絶対座標。width/height は `x1-x0` / `y1-y0`。 */
interface AccessKitRect {
  readonly x0: number;
  readonly y0: number;
  readonly x1: number;
  readonly y1: number;
}

/** AccessKit `TreeUpdate`（serde）の、ミラーが消費する部分形。 */
interface AccessKitTreeUpdate {
  readonly nodes: ReadonlyArray<readonly [number, AccessKitNode]>;
  readonly tree?: { readonly root: number } | null;
  readonly focus?: number;
}

/**
 * Web Canvas Accessibility Mirror（ADR-0124）を canvas 兄弟に attach する。`<canvas>` の兄弟に
 * `data-hayate-a11y` の不可視 root を建て、返り値の `poll()` が呼ばれるたびに `raw.poll_accessibility()`
 * （AccessKit `TreeUpdate` の JSON）を 1 度取得し、**返り JSON が null／前回適用値と同一なら DOM を一切
 * 触らない**（#642 dirty ゲート＋安価な文字列比較スキップ）。変化時は `TreeUpdate` を root 配下の不可視
 * ARIA DOM に 1:1 投影する（各ノード → `<div role=…>`、accessible name = `aria_label`、value/text =
 * textContent、対応は NodeId キーで差分適用）。
 *
 * **独立 rAF ループは持たない（#645）**。ミラーは自前でフレームを掴まず、`createHayateWebHost` が
 * レンダラの各フレーム末尾で `poll()` を 1 回呼ぶ（相乗り）。レンダラが on-demand で idle に落ちれば
 * （ADR-0126）ミラーの tick もそのまま止まり、wake 経路（入力・mutation・継続 pending）はレンダラと
 * 共有される。frame-clock がアプリ全体で 1 本になる（診断 `docs/perf-android-chrome-vello-jank-*` 要因 1）。
 *
 * このシームは `createHayateWebHost` が canvas boot のたびに 1 箇所で呼ぶ（#591）。標準アプリの
 * 直 boot（`main.tsx`）も Miharashi dev ホストも `createHayateWebHost` を通るため、全 Canvas アプリ
 * がここを 1 回通り、host-boot 毎の配線なしにミラーを得る（ADR-0124）。
 */
export function attachAccessibilityMirror(
  raw: RawHayate,
  canvas: HTMLCanvasElement,
): AccessibilityMirror {
  // 非ブラウザ環境（テストの fake canvas 等）や DOM 未接続の canvas では建てられない。
  // host 構築を落とさないため no-op のミラーを返す（clock lookup を遅延するのと同じ思想）。
  const doc = canvas?.ownerDocument;
  const parent = canvas?.parentNode;
  if (!doc || !parent) return { poll: () => {}, detach: () => {} };

  const root = doc.createElement('div');
  root.setAttribute(A11Y_ROOT_ATTR, '');
  // 矩形配置のまま不可視・非干渉にする。精密な per-node bounds は #593。
  root.style.position = 'absolute';
  root.style.top = '0';
  root.style.left = '0';
  root.style.opacity = MIRROR_OPACITY;
  root.style.pointerEvents = MIRROR_POINTER_EVENTS;
  parent.insertBefore(root, canvas.nextSibling);

  // NodeId → 投影要素。差分適用で要素の identity を保つ（不変フレームは触らない）。
  const nodeEls = new Map<number, HTMLElement>();
  let lastApplied: string | null = null;
  let detached = false;

  const project = (json: string): void => {
    let update: AccessKitTreeUpdate;
    try {
      update = JSON.parse(json) as AccessKitTreeUpdate;
    } catch {
      return; // 壊れた JSON は前フレームの DOM を保つ。
    }

    const present = new Set<number>();

    // 1) 各ノードの要素を get-or-create し、role / accessible name を反映する。
    for (const [id, node] of update.nodes) {
      present.add(id);
      let el = nodeEls.get(id);
      if (!el) {
        el = doc.createElement('div');
        el.id = `${A11Y_NODE_ID_PREFIX}${id}`;
        nodeEls.set(id, el);
      }
      const aria = ACCESSKIT_ROLE_TO_ARIA[node.role] ?? '';
      if (aria) el.setAttribute('role', aria);
      else el.removeAttribute('role');

      const label = node.properties?.label;
      if (label != null) el.setAttribute('aria-label', label);
      else el.removeAttribute('aria-label');

      // bounds を on-canvas 矩形へ絶対配置する。これでミラーノードが当たり位置に重なり、
      // Playwright が `boundingBox()` から駆動座標を得られる（pointer-events:none なので
      // 座標クリックは下の <canvas> に届き、ミラーは横取りしない・ADR-0124）。
      const bounds = node.properties?.bounds;
      if (bounds) {
        el.style.position = 'absolute';
        el.style.left = `${bounds.x0}px`;
        el.style.top = `${bounds.y0}px`;
        el.style.width = `${bounds.x1 - bounds.x0}px`;
        el.style.height = `${bounds.y1 - bounds.y0}px`;
      }
    }

    // 2) 構造を結線する。子があれば子要素を順に再 parent、無ければ value を textContent に。
    for (const [id, node] of update.nodes) {
      const el = nodeEls.get(id)!;
      const childIds = node.properties?.children ?? [];
      if (childIds.length > 0) {
        // 残存テキストノードを除去してから子要素を順に append（appendChild が再 parent する）。
        for (const child of [...el.childNodes]) {
          if (child.nodeType === 3 /* TEXT_NODE */) el.removeChild(child);
        }
        for (const cid of childIds) {
          const cel = nodeEls.get(cid);
          if (cel) el.appendChild(cel);
        }
      } else {
        const value = node.properties?.value;
        el.textContent = value != null ? value : '';
      }
    }

    // 3) 消えた NodeId の要素を除去する。
    for (const [id, el] of nodeEls) {
      if (!present.has(id)) {
        el.remove();
        nodeEls.delete(id);
      }
    }

    // 4) root ノードをミラーコンテナ直下に取り付ける。
    const rootId = update.tree?.root ?? update.focus;
    if (rootId != null) {
      const rootEl = nodeEls.get(rootId);
      if (rootEl) root.appendChild(rootEl);
    }

    // 5) focus を反映する。root の aria-activedescendant を focus ノードの id に向け、
    // どの要素が focus されているかをテストから一意に判別できるようにする（#594）。
    const focusId = update.focus;
    if (focusId != null && nodeEls.has(focusId)) {
      root.setAttribute('aria-activedescendant', `${A11Y_NODE_ID_PREFIX}${focusId}`);
    } else {
      root.removeAttribute('aria-activedescendant');
    }
  };

  const poll = (): void => {
    // detach 後にレンダラのフレーム相乗り poll が 1 回遅れて来ても、DOM を再生させず安全に抜ける。
    if (detached) return;
    const json = raw.poll_accessibility();
    // #642: core の dirty ゲートが「変更なし」フレームを `null` で返す（全ツリー walk も JSON 生成も
    // しない）。`null` は文字列比較なしにスキップし、直近適用 JSON も温存する（次の実変更まで DOM 不変）。
    // 非 null（＝a11y 変更あり）でも、稀な over-bump（視覚のみ変更で a11y JSON は同一）に備え文字列
    // 比較を二段目のガードとして残し、同一なら再投影しない。
    if (json == null) return;
    if (json !== lastApplied) {
      lastApplied = json;
      project(json);
    }
  };

  const detach = (): void => {
    detached = true;
    root.remove();
    nodeEls.clear();
  };

  return { poll, detach };
}
