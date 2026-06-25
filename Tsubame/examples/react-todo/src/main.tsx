import { renderTsubame } from '@tsubame/react';
import { DomRenderer } from '@tsubame/renderer-dom';
import { App } from './App';

// DOM Renderer 経路：Hayate（WASM/Canvas）を迂回し、React Fiber の更新を
// Tsubame Renderer Protocol 経由で素の DOM へ流す最小デモ（ADR-0010）。
// viewport 追従はブラウザの CSS リフローが担い、Tsubame は resize を配線しない
// （ADR-0080 / tsubame-solid と対称）。
const host = document.getElementById('dom-host') as HTMLDivElement;
const renderer = new DomRenderer({ container: host });

renderTsubame(<App />, renderer);
