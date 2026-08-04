import { validateDemoManifest, type DemoManifest } from '@torimi/wire-contract';
import demosSource from './demos.json';

/**
 * 配信する Demo Manifest。正本は `demos.json`（wire フィールド＋build metadata）で、
 * ここで wire 型（表示名とバンドル URL のみ）へ射影する — build metadata は
 * `build:demos` の領分で、wire に漏らさない。
 */
const projectedManifest = {
  demos: demosSource.demos.map(({ name, bundleUrl }) => ({ name, bundleUrl })),
};

const validatedManifest = validateDemoManifest(projectedManifest);
if (!validatedManifest.ok) {
  const summary = validatedManifest.issues
    .map(({ path, expected, actualType }) => `${path}: expected ${expected}, got ${actualType}`)
    .join('; ');
  throw new Error(`Demo Manifest does not conform to the generated wire contract: ${summary}`);
}

export const demoManifest: DemoManifest = validatedManifest.value;
