import { Window } from 'happy-dom';

export interface HappyDomFixture {
  window: Window;
  document: Document;
  container: HTMLElement;
}

/** happy-dom DOM types are not structurally compatible with lib.dom; cast at this boundary. */
export function createHappyDomFixture(): HappyDomFixture {
  const window = new Window();
  const container = window.document.createElement('div');
  window.document.body.appendChild(container);
  return {
    window,
    document: window.document as unknown as Document,
    container: container as unknown as HTMLElement,
  };
}
