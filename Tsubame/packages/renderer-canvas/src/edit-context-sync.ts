import { MODIFIER } from '@tsubame/protocol-generated/protocol';
import type { RawHayate } from './hayate.js';

/** EditContext の変換フォーマット範囲1件（`textformatupdate.getTextFormats()`）。
 * オフセットは EditContext テキスト上の UTF-16 コードユニット位置。 */
export interface EditTextFormat {
  rangeStart: number;
  rangeEnd: number;
  underlineStyle?: string;
  underlineThickness?: string;
}

const byteEncoder = new TextEncoder();

/** `text` 内の UTF-16 コードユニットオフセットを UTF-8 バイトオフセットへ変換する。
 * EditContext は UTF-16、Hayate core の編集モデルは UTF-8 バイトオフセットを扱うため、
 * 変換節の範囲は wasm 境界を渡す前に変換する。 */
function utf16ToByteOffset(text: string, utf16Offset: number): number {
  const clamped = Math.max(0, Math.min(utf16Offset, text.length));
  return byteEncoder.encode(text.slice(0, clamped)).length;
}

/** EditContext の `textformatupdate` フォーマットを、wasm core が消費する平坦な
 * `[start, end, weight, …]` UTF-8 バイトオフセット三つ組ストリームへ変換する（ADR-0102）。
 * `text` は現在のプリエディット、`base` は EditContext テキスト上の変換中セグメント開始
 * オフセット（UTF-16）。プリエディット外・つぶれた範囲・明示的に下線なしの範囲は除外する。
 * `weight` は太い下線（アクティブ節）で `1`、それ以外は `0`。 */
export function compositionFormatsToWire(
  text: string,
  base: number,
  formats: readonly EditTextFormat[],
): Uint32Array {
  const out: number[] = [];
  for (const f of formats) {
    if (f.underlineStyle === 'None') continue;
    const start = utf16ToByteOffset(text, f.rangeStart - base);
    const end = utf16ToByteOffset(text, f.rangeEnd - base);
    if (start >= end) continue;
    out.push(start, end, f.underlineThickness === 'Thick' ? 1 : 0);
  }
  return Uint32Array.from(out);
}

/** canvas ピクセル座標をスクリーン空間の DOMRect へ変換する。 */
export function canvasPixelRectToDomRect(
  canvas: HTMLCanvasElement,
  x: number,
  y: number,
  width: number,
  height: number,
): DOMRect {
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width === 0 ? 1 : rect.width / canvas.width;
  const scaleY = canvas.height === 0 ? 1 : rect.height / canvas.height;
  return new DOMRect(
    rect.left + x * scaleX,
    rect.top + y * scaleY,
    width * scaleX,
    height * scaleY,
  );
}

// web の ImeBridge ホスト（ADR-0069）。プラットフォームの `EditContext` に触れてよいのは
// このモジュールだけ — 生成・イベント配線・canvas への着脱を一手に担う。ソフトキーボードの
// 表示可否は core が決め（`ElementTree::drive_ime`、`raw.ime_wants_keyboard()` として露出）、
// ホストはそれを反映するのみ。すべての `EditContext` 参照をここに集約すること
// （`ime-bridge-encapsulation.test.ts` で強制）が、プラットフォームごとのゲーティング乖離の
// 再発を防ぐ。

/** canvas ごとの生きた EditContext。canvas ではなくここで保持するため、デタッチ中
 * （`canvas.editContext === null`）もインスタンスが生き残る。 */
const editContexts = new WeakMap<HTMLCanvasElement, EditContext>();

/**
 * canvas の EditContext を生成し、IME・キーボードイベントを配線する（ADR-0069）。
 *
 * EditContext は起動時にはアタッチしない。アタッチがモバイルのソフトキーボードを
 * 立ち上げてしまうため、アタッチは {@link syncEditContext} に委ね、core が `text-input`
 * のフォーカスを報告している間（`raw.ime_wants_keyboard()`）だけアタッチする。
 * 以前は常時アタッチしていたため、canvas をフォーカスする任意のタップで非編集要素でも
 * キーボードが立ち上がっていた。
 */
export function attachTextInput(
  canvas: HTMLCanvasElement,
  raw: RawHayate,
  // テスト用に注入可能。本番はプラットフォームの `EditContext` を使う。プラットフォームに
  // EditContext がなく（HTML モード、ADR-0016）ファクトリも与えられない場合、IME 配線は
  // 完全にスキップする。
  createEditContext?: () => EditContext,
): void {
  const make =
    createEditContext ??
    (typeof EditContext === 'undefined' ? null : () => new EditContext());
  if (make === null) return;

  canvas.tabIndex = 0;
  const editContext = make();
  editContexts.set(canvas, editContext);
  let composing = false;
  // 変換中セグメントの開始オフセット（UTF-16）と現在のプリエディットテキスト。
  // `textformatupdate` の節範囲をプリエディット相対にし、wire を渡す前に UTF-8 バイト
  // オフセットへ変換するために追跡する（ADR-0102）。
  let composeBase = 0;
  let composeText = '';

  editContext.addEventListener('compositionstart', () => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = true;
    composeBase = editContext.selectionStart;
    composeText = '';
    raw.on_composition_start(id, '');
  });

  editContext.addEventListener('textupdate', (e: TextUpdateEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    const text = e.text ?? '';
    if (composing) {
      composeBase = e.updateRangeStart;
      composeText = text;
      // まずフォーマットなしの更新を送る。変換下線は後続の `textformatupdate` で届き、
      // 節範囲付きで再送される。
      raw.on_composition_update(id, text);
    } else {
      raw.on_text_input(id, text);
    }
  });

  editContext.addEventListener('textformatupdate', (e: TextFormatUpdateEvent) => {
    if (!composing) return;
    const id = raw.focused_element_id();
    if (id === 0) return;
    const formats = e.getTextFormats() as unknown as EditTextFormat[];
    const wire = compositionFormatsToWire(composeText, composeBase, formats);
    raw.on_composition_update_formatted(id, composeText, wire);
  });

  editContext.addEventListener('compositionend', (e: CompositionEndEvent) => {
    const id = raw.focused_element_id();
    if (id === 0) return;
    composing = false;
    composeText = '';
    raw.on_composition_end(id, e.data ?? '');
  });

  canvas.addEventListener('keydown', (e) => {
    const id = raw.focused_element_id();
    // 選択系のキー操作（Ctrl/Cmd+A、Shift+Arrow）はドキュメント全体の選択に作用するため、
    // 何もフォーカスされていなくても（読み取り専用の選択領域）配送する。選択キーは core が
    // 内部で消費する。
    if (id === 0 && !raw.has_selection()) return;
    if (composing && e.key !== 'Escape') {
      e.preventDefault();
      return;
    }

    let mods = 0;
    if (e.shiftKey) mods |= MODIFIER.SHIFT;
    if (e.ctrlKey) mods |= MODIFIER.CTRL;
    if (e.altKey) mods |= MODIFIER.ALT;
    if (e.metaKey) mods |= MODIFIER.META;
    raw.on_key_down(e.key, mods);

    const isPrintable = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;
    if (!isPrintable) {
      e.preventDefault();
    }
  });
}

/**
 * 毎フレーム、core の IME 表現を canvas の EditContext へ反映する（ADR-0069）。
 *
 * - `raw.ime_wants_keyboard()` が false → EditContext をデタッチし、ソフトキーボードを
 *   閉じる（タップだけでは立ち上がらない）。
 * - true → アタッチし（キーボードを立ち上げ）、IME 候補ウィンドウをキャレットの文字境界へ
 *   合わせる。
 */
export function syncEditContext(canvas: HTMLCanvasElement, raw: RawHayate): void {
  const wants = raw.ime_wants_keyboard();
  const owned = editContexts.get(canvas);

  // 自前で所有する EditContext（`attachTextInput` で生成）は、アタッチがモバイルの
  // ソフトキーボードを立ち上げる。よって `text-input` がフォーカスされている間だけアタッチし、
  // それ以外はデタッチする。ホスト管理の EditContext（埋め込みレンダラやテスト）は所有者に任せ、
  // 候補ウィンドウの配置のみ行う。
  if (owned !== undefined) {
    if (wants) {
      if (canvas.editContext !== owned) canvas.editContext = owned;
    } else if (canvas.editContext === owned) {
      canvas.editContext = null;
    }
  }

  if (!wants) return;
  const editContext = canvas.editContext;
  if (editContext === undefined || editContext === null) return;

  const bounds = raw.ime_character_bounds();
  if (bounds[2] === 0 && bounds[3] === 0) return;

  const dom = canvasPixelRectToDomRect(
    canvas,
    bounds[0]!,
    bounds[1]!,
    bounds[2]!,
    bounds[3]!,
  );
  editContext.updateControlBounds(dom);
  editContext.updateSelectionBounds(dom);
}
