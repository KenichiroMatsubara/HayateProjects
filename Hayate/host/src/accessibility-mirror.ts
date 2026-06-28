import type { RawHayate } from './raw-hayate.js';

export type { RawHayate } from './raw-hayate.js';

/**
 * {@link attachAccessibilityMirror} の後始末関数。ミラー root を DOM から除去し rAF ループを
 * 止める。host のライフサイクル teardown（full reload）から呼ぶ（ADR-0124）。
 */
export type DetachAccessibilityMirror = () => void;

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
  };
}

/** AccessKit `TreeUpdate`（serde）の、ミラーが消費する部分形。 */
interface AccessKitTreeUpdate {
  readonly nodes: ReadonlyArray<readonly [number, AccessKitNode]>;
  readonly tree?: { readonly root: number } | null;
  readonly focus?: number;
}

/** rAF の注入 seam（テストが 1 フレームずつ駆動できるようにする）。既定はブラウザの rAF。 */
export interface AccessibilityMirrorOptions {
  readonly requestFrame?: (cb: FrameRequestCallback) => number;
  readonly cancelFrame?: (handle: number) => void;
}

/**
 * Web Canvas Accessibility Mirror（ADR-0124）を canvas 兄弟に attach する。`<canvas>` の兄弟に
 * `data-hayate-a11y` の不可視 root を建て、自前 rAF ループで `raw.poll_accessibility()`（AccessKit
 * `TreeUpdate` の JSON）を毎フレーム取得し、**返り JSON が前回適用値と同一なら DOM を一切触らない**
 * （安価な文字列比較スキップ）。変化時は `TreeUpdate` を root 配下の不可視 ARIA DOM に 1:1 投影する
 * （各ノード → `<div role=…>`、accessible name = `aria_label`、value/text = textContent、対応は
 * NodeId キーで差分適用）。返り値は detach（root 除去・rAF 停止）で full reload で呼ばれる。
 *
 * このシームは `createHayateWebHost` が canvas boot のたびに 1 箇所で呼ぶ（#591）。標準アプリの
 * 直 boot（`main.tsx`）も Miharashi dev ホストも `createHayateWebHost` を通るため、全 Canvas アプリ
 * がここを 1 回通り、host-boot 毎の配線なしにミラーを得る（ADR-0124）。
 *
 * bounds による精密配置・focus 反映は後続スライス（#593/#594）。ここでは role/name/value/構造と
 * loop/配線/detach に集中する。
 */
export function attachAccessibilityMirror(
  raw: RawHayate,
  canvas: HTMLCanvasElement,
  options?: AccessibilityMirrorOptions,
): DetachAccessibilityMirror {
  // 非ブラウザ環境（テストの fake canvas 等）や DOM 未接続の canvas では建てられない。
  // host 構築を落とさないため no-op の detach を返す（clock lookup を遅延するのと同じ思想）。
  const doc = canvas?.ownerDocument;
  const parent = canvas?.parentNode;
  if (!doc || !parent) return () => {};

  const requestFrame =
    options?.requestFrame ?? ((cb: FrameRequestCallback) => globalThis.requestAnimationFrame(cb));
  const cancelFrame =
    options?.cancelFrame ?? ((handle: number) => globalThis.cancelAnimationFrame(handle));

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
  let handle = 0;

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
        nodeEls.set(id, el);
      }
      const aria = ACCESSKIT_ROLE_TO_ARIA[node.role] ?? '';
      if (aria) el.setAttribute('role', aria);
      else el.removeAttribute('role');

      const label = node.properties?.label;
      if (label != null) el.setAttribute('aria-label', label);
      else el.removeAttribute('aria-label');
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
  };

  const tick: FrameRequestCallback = () => {
    const json = raw.poll_accessibility();
    if (json !== lastApplied) {
      lastApplied = json;
      if (json != null) project(json);
    }
    handle = requestFrame(tick);
  };
  handle = requestFrame(tick);

  return () => {
    cancelFrame(handle);
    root.remove();
    nodeEls.clear();
  };
}
