//! native 永続パイプラインキャッシュ（ADR-0130b）。
//!
//! Vulkan/Metal のパイプラインキャッシュ blob をディスクに永続化し、**起動間**の cold start を速くする
//! （Flutter/Impeller 相当）。#610 の init 時 warmup は同一プロセス内の初回スパイクを消すが、本モジュールは
//! **二度目以降の起動**を速くする。キャッシュのパス・バージョニング（ドライバ/シェーダ変更時の無効化）・
//! 破損フォールバックを担う。
//!
//! GPU driver が吐く不透明 blob（`VkPipelineCache` データ / `MTLBinaryArchive`）は `Vec<u8>` として扱い、
//! 永続化・検証・破損処理だけを GPU 非依存に実装するため、ホストの `cargo test` で実ファイル往復まで
//! 固定できる。

use std::fs;
use std::io;
use std::path::Path;

/// キャッシュフォーマットの magic（バージョン番号込み）。フォーマット非互換を最上流で弾く。
const MAGIC: [u8; 4] = *b"HPC1";
/// ヘッダ固定長（magic 4 ＋ format_version 4 ＋ driver 8 ＋ shader 8 ＋ blob_len 8）。
const HEADER_LEN: usize = 32;

/// 永続キャッシュの妥当性を決めるキー（ADR-0130b）。driver / shader / フォーマットのいずれかが
/// 変わると別キーになり、古いキャッシュは無効化して再生成する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCacheKey {
    /// GPU ドライバ版（Vulkan `driverVersion` / Metal の GPU family + OS 版から導出）。
    pub driver_version: u64,
    /// シェーダ集合のハッシュ。シェーダが変わると変化し、古いパイプラインを無効化する。
    pub shader_hash: u64,
    /// 本モジュールのキャッシュフォーマット版（互換性の世代）。
    pub format_version: u32,
}

/// FNV-1a による決定的 64bit ハッシュ。[`PipelineCacheKey`] の `driver_version` / `shader_hash`
/// をドライバ情報文字列やシェーダソースから導くための共有ヘルパー。`std` の `DefaultHasher` は
/// シード・実装が安定保証されず**永続**キーに使えないため、自前の安定ハッシュを持つ。
pub fn fnv1a_hash<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for part in parts {
        for &b in part {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(PRIME);
        }
        // part 境界にも 1 バイト混ぜ、["ab","c"] と ["a","bc"] を区別する。
        hash ^= 0xff;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("4 bytes"))
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("8 bytes"))
}

/// キーと blob を 1 つのバイト列へ符号化する（magic ＋ ヘッダ ＋ blob）。
pub fn encode(key: &PipelineCacheKey, blob: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + blob.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&key.format_version.to_le_bytes());
    out.extend_from_slice(&key.driver_version.to_le_bytes());
    out.extend_from_slice(&key.shader_hash.to_le_bytes());
    out.extend_from_slice(&(blob.len() as u64).to_le_bytes());
    out.extend_from_slice(blob);
    out
}

/// バイト列を復号し、`current_key` と一致するときだけ blob を返す。magic 不一致・キー不一致
/// （ドライバ/シェーダ/フォーマット変更）・切り詰め/破損のいずれでも `None`（再生成へフォールバック）。
/// 例外を投げずに `None` に倒すことで、破損キャッシュで起動が壊れないことを保証する（ADR-0130b）。
pub fn decode(bytes: &[u8], current_key: &PipelineCacheKey) -> Option<Vec<u8>> {
    if bytes.len() < HEADER_LEN || bytes[0..4] != MAGIC {
        return None;
    }
    let stored = PipelineCacheKey {
        format_version: read_u32(&bytes[4..8]),
        driver_version: read_u64(&bytes[8..16]),
        shader_hash: read_u64(&bytes[16..24]),
    };
    if stored != *current_key {
        return None; // ドライバ/シェーダ/フォーマット変更 → 無効化
    }
    let blob_len = read_u64(&bytes[24..32]) as usize;
    // 宣言長と実バイトが食い違えば破損とみなしてフォールバック。
    if bytes.len() != HEADER_LEN + blob_len {
        return None;
    }
    Some(bytes[HEADER_LEN..].to_vec())
}

/// キャッシュ blob をキー付きで `path` に永続化する（初回起動で書く）。
pub fn save(path: &Path, key: &PipelineCacheKey, blob: &[u8]) -> io::Result<()> {
    fs::write(path, encode(key, blob))
}

/// `path` の永続キャッシュを読み、`current_key` に一致すれば blob を返す（二度目以降の起動で読む）。
/// ファイル無し・I/O エラー・キー不一致・破損のいずれも `None`＝呼び元は新規生成して `save` し直す。
/// 起動を壊さないため、ここでは決して `Err` を伝播しない。
pub fn load(path: &Path, current_key: &PipelineCacheKey) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;
    decode(&bytes, current_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn key() -> PipelineCacheKey {
        PipelineCacheKey {
            driver_version: 42,
            shader_hash: 0xDEAD_BEEF,
            format_version: 1,
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("hayate_pipeline_cache_test_{name}"));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn fnv1a_is_deterministic_and_boundary_sensitive() {
        // 永続キーに使うので、プロセス・ビルドをまたいで同値であることが本質。
        assert_eq!(
            fnv1a_hash([b"abc".as_slice()]),
            fnv1a_hash([b"abc".as_slice()])
        );
        assert_ne!(fnv1a_hash([b"a".as_slice()]), fnv1a_hash([b"b".as_slice()]));
        // part 境界が異なれば別ハッシュ（["ab","c"] ≠ ["a","bc"]）。
        assert_ne!(
            fnv1a_hash([b"ab".as_slice(), b"c".as_slice()]),
            fnv1a_hash([b"a".as_slice(), b"bc".as_slice()]),
        );
    }

    #[test]
    fn round_trips_blob_for_matching_key() {
        // 同一キーで符号化→復号すると blob が戻る（二度目以降の起動で読める）。
        let blob = vec![1u8, 2, 3, 4, 5];
        let bytes = encode(&key(), &blob);
        assert_eq!(decode(&bytes, &key()), Some(blob));
    }

    #[test]
    fn driver_change_invalidates_cache() {
        let bytes = encode(&key(), &[9, 9, 9]);
        let mut newer = key();
        newer.driver_version = 43; // ドライバ更新
        assert_eq!(
            decode(&bytes, &newer),
            None,
            "ドライバ変更でキャッシュ無効化"
        );
    }

    #[test]
    fn shader_change_invalidates_cache() {
        let bytes = encode(&key(), &[9, 9, 9]);
        let mut newer = key();
        newer.shader_hash = 0x1234; // シェーダ更新
        assert_eq!(
            decode(&bytes, &newer),
            None,
            "シェーダ変更でキャッシュ無効化"
        );
    }

    #[test]
    fn format_version_change_invalidates_cache() {
        let bytes = encode(&key(), &[9, 9, 9]);
        let mut newer = key();
        newer.format_version = 2;
        assert_eq!(decode(&bytes, &newer), None);
    }

    #[test]
    fn corrupt_or_truncated_bytes_fall_back_to_none() {
        // 破損キャッシュで起動が壊れないこと（Err を投げず None に倒す）。
        assert_eq!(decode(&[], &key()), None, "空");
        assert_eq!(decode(&[0u8; 10], &key()), None, "ヘッダ未満");
        assert_eq!(
            decode(b"XXXX....................", &key()),
            None,
            "magic 不一致"
        );

        // 正しいヘッダだが blob を切り詰めた（宣言 5 バイトに対し実 2 バイト）。
        let mut bytes = encode(&key(), &[1, 2, 3, 4, 5]);
        bytes.truncate(HEADER_LEN + 2);
        assert_eq!(decode(&bytes, &key()), None, "切り詰め破損");
    }

    #[test]
    fn save_then_load_persists_across_launches() {
        // 初回起動で save → 二度目の起動を模して load。永続キャッシュが読める（ADR-0130b）。
        let path = temp_path("persist");
        let blob = vec![7u8; 64];
        save(&path, &key(), &blob).unwrap();
        assert_eq!(load(&path, &key()), Some(blob));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_none_for_first_launch() {
        // 初回起動はキャッシュファイルが無い → None（呼び元が新規生成して save する）。
        let path = temp_path("missing");
        assert_eq!(load(&path, &key()), None);
    }

    #[test]
    fn load_after_driver_change_ignores_stale_file() {
        // ディスク上の古いキャッシュは、ドライバが変わると load で無効化される（再生成へ）。
        let path = temp_path("stale");
        save(&path, &key(), &[1, 2, 3]).unwrap();
        let mut newer = key();
        newer.driver_version = 99;
        assert_eq!(load(&path, &newer), None);
        let _ = fs::remove_file(&path);
    }
}
