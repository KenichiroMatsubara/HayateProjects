import type { DemoManifest } from '@miharashi/dev-server-contract';
import demosSource from './demos.json';

/**
 * 配信する Demo Manifest。正本は `demos.json`（wire フィールド＋build metadata）で、
 * ここで wire 型（表示名とバンドル URL のみ）へ射影する — build metadata は
 * `build:demos` の領分で、wire に漏らさない。
 */
export const demoManifest: DemoManifest = {
  demos: demosSource.demos.map(({ name, bundleUrl }) => ({ name, bundleUrl })),
};
