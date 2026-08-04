import { createBrowserHost } from '@torimi/tsubame-browser-host';
import { renderTsubame } from '@torimi/tsubame-solid';
import { runTsubameApp } from '@torimi/tsubame-app';
import { DrawGalleryApp } from './App';

const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は
// IRenderer しか知らない（ADR-0012）。draw ギャラリーは todo example と同じ web
// Host adapter の縮小版で、layer-present / tuning などのチューニング口は持たない。
const host = createBrowserHost({ dom, canvas });

runTsubameApp(host, (renderer) => renderTsubame(() => <DrawGalleryApp />, renderer));
