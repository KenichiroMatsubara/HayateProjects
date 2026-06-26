import { describe, expect, it } from 'vitest';
import {
  checkProtocolVersion,
  MIHARASHI_PROTOCOL_VERSION_GLOBAL,
  ProtocolMismatchError,
  readBundleProtocolVersion,
} from './index.js';

/**
 * protocol version 突き合わせの単体契約テスト（#530）。バンドル（encoder）が埋めた wire 定数
 * バージョンと、ホスト（decoder）に焼き込まれたバージョンを突き合わせ、一致なら mount を許し、
 * 不一致なら明示エラー（両バージョンを含む）にする。FW/プラットフォーム非依存の純関数として、
 * Web/Android のホストが共有する（ADR-0001 / CONTEXT.md「Protocol Version」）。
 */
describe('checkProtocolVersion', () => {
  it('一致したら ok を返す（mount を許す）', () => {
    expect(checkProtocolVersion(1, 1)).toEqual({ ok: true });
  });

  it('不一致なら ok=false と両バージョンを返す', () => {
    const result = checkProtocolVersion(1, 2);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.hostVersion).toBe(1);
    expect(result.bundleVersion).toBe(2);
  });

  it('不一致メッセージにホスト/バンドル両方のバージョンを明示する', () => {
    const result = checkProtocolVersion(3, 7);
    if (result.ok) throw new Error('expected mismatch');
    expect(result.message).toContain('3');
    expect(result.message).toContain('7');
  });

  it('バンドルが version 未埋め込み（undefined）なら明示エラーにする', () => {
    const result = checkProtocolVersion(1, undefined);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.hostVersion).toBe(1);
    expect(result.bundleVersion).toBeUndefined();
    expect(result.message).toContain('1');
  });
});

/**
 * バンドル → ホストの protocol version 受け渡しシーム。バンドル（App Bundle）は eval 時に
 * global へ自身の wire 定数バージョンを立て（`__miharashiMount` と対称）、ホストは eval 後に
 * これを読んで突き合わせる。global 名は wire 契約なので定数で固定する。
 */
describe('readBundleProtocolVersion', () => {
  it('バンドルが立てた global の数値バージョンを読む', () => {
    const scope = { [MIHARASHI_PROTOCOL_VERSION_GLOBAL]: 1 };
    expect(readBundleProtocolVersion(scope)).toBe(1);
  });

  it('バージョン未埋め込みなら undefined を返す（契約違反はホストが明示エラーにする）', () => {
    expect(readBundleProtocolVersion({})).toBeUndefined();
  });

  it('数値でない値は undefined として扱う（壊れた埋め込みを mount に通さない）', () => {
    const scope = { [MIHARASHI_PROTOCOL_VERSION_GLOBAL]: 'v1' };
    expect(readBundleProtocolVersion(scope)).toBeUndefined();
  });
});

/**
 * ホスト（Web/Android 共通）が不一致時に投げる型付きエラー。合成ルートはこれを捕まえて明示
 * エラー UI を出す（mount もクラッシュもさせない）。両バージョンを構造化して持つ。
 */
describe('ProtocolMismatchError', () => {
  it('不一致結果から message と両バージョンを保持する', () => {
    const result = checkProtocolVersion(1, 2);
    if (result.ok) throw new Error('expected mismatch');
    const error = new ProtocolMismatchError(result);
    expect(error).toBeInstanceOf(Error);
    expect(error.name).toBe('ProtocolMismatchError');
    expect(error.message).toBe(result.message);
    expect(error.hostVersion).toBe(1);
    expect(error.bundleVersion).toBe(2);
  });
});
