import { TORIMI_MOUNT_GLOBAL as HOST_WEB_MOUNT_GLOBAL } from '@torimi/host-web';
import { describe, expect, it } from 'vitest';
import { TORIMI_MOUNT_GLOBAL } from './register.js';

/**
 * バンドル側（このパッケージ）とホスト側（`@torimi/host-web`）は互いに import しない
 * （依存方向：ホスト実装が App Bundle に紛れ込まないため）。その代わり、両者が別々に
 * 定義する mount seam の global 名が乖離したら（= handshake 変更が片側だけに入ったら）
 * ここで落ちる。ADR-0008 §4 の動機そのもの — wire 契約のコピペ配布は黙って壊れる。
 */
describe('mount seam wire contract (@torimi/bundle ↔ @torimi/host-web)', () => {
  it('exposes the mount on the exact global the web host reads', () => {
    expect(TORIMI_MOUNT_GLOBAL).toBe(HOST_WEB_MOUNT_GLOBAL);
  });
});
