/**
 * protocol version 突き合わせ結果。一致なら `{ ok: true }`、不一致なら両バージョンと、
 * 表示用の明示メッセージを含む `{ ok: false, ... }`。
 */
export type ProtocolHandshakeResult =
  | { readonly ok: true }
  | {
      readonly ok: false;
      readonly hostVersion: number;
      /** バンドルが埋めた版数。未埋め込み（契約違反）なら `undefined`。 */
      readonly bundleVersion: number | undefined;
      readonly message: string;
    };

/**
 * バンドル（encoder）が埋めた wire 定数バージョンと、ホスト（decoder）に焼き込まれた
 * バージョンを突き合わせる。一致時のみ mount を許し、不一致は両バージョンを含む明示エラーに
 * する（謎クラッシュにしない）。FW/プラットフォーム非依存の純関数で、Web/Android のホストが
 * 共有する（ADR-0001 / CONTEXT.md「Protocol Version」）。
 */
export function checkProtocolVersion(
  hostVersion: number,
  bundleVersion: number | undefined,
): ProtocolHandshakeResult {
  if (hostVersion === bundleVersion) return { ok: true };
  const bundleLabel = bundleVersion === undefined ? 'version 未埋め込み' : `v${bundleVersion}`;
  return {
    ok: false,
    hostVersion,
    bundleVersion,
    message: `このホストは protocol v${hostVersion}、バンドルは ${bundleLabel}`,
  };
}

/**
 * App Bundle が eval 時に自身の wire 定数バージョンを露出する global プロパティ名。mount を渡す
 * `__torimiMount`（`@torimi/host-web` の TORIMI_MOUNT_GLOBAL）と対称の、バンドル →
 * ホストの受け渡しシーム。global 名は wire 契約なので Web/Android で共有する定数に固定する。
 */
export const TORIMI_PROTOCOL_VERSION_GLOBAL = '__torimiProtocolVersion';

/**
 * eval 済みバンドルが {@link TORIMI_PROTOCOL_VERSION_GLOBAL} に立てた protocol version を読む。
 * 有限数なら返し、未埋め込み・非数値（契約違反 / 壊れた埋め込み）は `undefined` を返す
 * — ホストはそれを明示エラーにして mount もクラッシュもさせない。
 */
export function readBundleProtocolVersion(scope: object): number | undefined {
  const value = (scope as Record<string, unknown>)[TORIMI_PROTOCOL_VERSION_GLOBAL];
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

/** {@link checkProtocolVersion} の不一致結果（`ok: false`）。 */
export type ProtocolMismatch = Extract<ProtocolHandshakeResult, { ok: false }>;

/**
 * protocol version 不一致でホストが投げる型付きエラー。合成ルート（Web/Android 共通）はこれを
 * 捕まえて明示エラー UI を出し、mount もクラッシュもさせない（#530）。両バージョンを構造化して
 * 運ぶので、UI はメッセージだけでなく host/bundle の版数も使える。
 */
export class ProtocolMismatchError extends Error {
  readonly hostVersion: number;
  readonly bundleVersion: number | undefined;

  constructor(mismatch: ProtocolMismatch) {
    super(mismatch.message);
    this.name = 'ProtocolMismatchError';
    this.hostVersion = mismatch.hostVersion;
    this.bundleVersion = mismatch.bundleVersion;
  }
}
