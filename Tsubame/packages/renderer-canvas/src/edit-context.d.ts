/**
 * Experimental EditContext API — not yet in TypeScript's DOM lib.
 * @see https://developer.mozilla.org/en-US/docs/Web/API/EditContext_API
 */

interface EditContextInit {
  text?: string;
  selectionStart?: number;
  selectionEnd?: number;
}

interface TextUpdateEvent extends Event {
  readonly text: string;
  readonly updateRangeStart: number;
  readonly updateRangeEnd: number;
  readonly selectionStart: number;
  readonly selectionEnd: number;
}

interface CompositionEndEvent extends Event {
  readonly data: string;
}

declare class EditContext extends EventTarget {
  constructor(options?: EditContextInit);
  readonly text: string;
  readonly selectionStart: number;
  readonly selectionEnd: number;
  updateText(rangeStart: number, rangeEnd: number, text: string): void;
  updateSelection(start: number, end: number): void;
  updateControlBounds(controlBounds: DOMRect): void;
  updateSelectionBounds(selectionBounds: DOMRect): void;
  addEventListener(
    type: 'textupdate',
    listener: (this: EditContext, ev: TextUpdateEvent) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: 'compositionend',
    listener: (this: EditContext, ev: CompositionEndEvent) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ): void;
}

interface HTMLElement {
  editContext?: EditContext | null;
}
