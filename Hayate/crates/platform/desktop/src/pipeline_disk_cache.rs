//! 永続 GPU パイプラインキャッシュの desktop 配線（ADR-0130b・issue #777）。
//!
//! GPU ドライバのパイプラインキャッシュ blob をプラットフォーム標準のキャッシュディレクトリに
//! 永続化し、**二度目以降の起動**の vello シェーダ/パイプラインコンパイルを短縮する。blob の
//! キー付き符号化・破損フォールバックは GPU 非依存の
//! [`hayate_layer_compositor::pipeline_cache`]（ADR-0130b）が担い、本モジュールはその desktop
//! 側の I/O（パス選択・load/save・wgpu との受け渡し）だけを持つ。
//!
//! wgpu のパイプラインキャッシュは現状 Vulkan backend のみ対応
//! （`wgpu::util::pipeline_cache_key` が Vulkan 以外で `None`）。非対応 backend・キャッシュ
//! ディレクトリ不明・破損・キー不一致のいずれでも **起動は壊さず**、キャッシュ無しで従来
//! どおり初期化する。

use std::fs;
use std::path::{Path, PathBuf};

use hayate_layer_compositor::pipeline_cache::{self, fnv1a_hash, PipelineCacheKey};

/// ADR-0130b の符号化フォーマットに載せる、本配線のキャッシュ世代。載せ方（キーの導出規則）を
/// 変えたら上げる。
const FORMAT_VERSION: u32 = 1;

/// プラットフォーム標準のユーザーキャッシュディレクトリ配下の hayate 用サブディレクトリ。
/// 環境変数から純粋に導く（テスト可能にするため実 I/O・実 `std::env` から分離）。
/// 返り値のディレクトリは存在しないことがある（呼び出し側が `create_dir_all` する）。
pub fn cache_dir_from(os: &str, var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let base = match os {
        "linux" => var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".cache")))?,
        "macos" => PathBuf::from(var("HOME")?).join("Library/Caches"),
        "windows" => PathBuf::from(var("LOCALAPPDATA")?),
        _ => return None,
    };
    Some(base.join("hayate"))
}

/// 実行環境のキャッシュディレクトリ。導けなければ `None`（キャッシュ無しで続行）。
fn cache_dir() -> Option<PathBuf> {
    cache_dir_from(std::env::consts::OS, |k| std::env::var(k).ok())
}

/// アダプタ情報から ADR-0130b の [`PipelineCacheKey`] を導く。`driver_version` はドライバ情報
/// 文字列の安定ハッシュ（ドライバ更新で無効化）、`shader_hash` は呼び出し側が渡す vello
/// シェーダ集合の指紋（シェーダ変更で無効化）。
pub fn cache_key(info: &wgpu::AdapterInfo, shader_hash: u64) -> PipelineCacheKey {
    driver_cache_key(&info.name, &info.driver, &info.driver_info, shader_hash)
}

/// [`cache_key`] の純粋 seam。`wgpu::AdapterInfo` から関係する三つ組だけを抜いた形で、
/// wgpu のフィールド増減に依存せずキー導出規則をテストできる。
pub fn driver_cache_key(
    name: &str,
    driver: &str,
    driver_info: &str,
    shader_hash: u64,
) -> PipelineCacheKey {
    PipelineCacheKey {
        driver_version: fnv1a_hash([name.as_bytes(), driver.as_bytes(), driver_info.as_bytes()]),
        shader_hash,
        format_version: FORMAT_VERSION,
    }
}

/// 永続パイプラインキャッシュのファイル所在とキー。backend 非対応（非 Vulkan）や
/// キャッシュディレクトリ不明なら `discover` が `None` を返し、呼び出し側はキャッシュ無しで
/// 続行する。
pub struct DiskPipelineCache {
    path: PathBuf,
    key: PipelineCacheKey,
    /// 起動時に読めた blob。`persist` で「変化があったときだけ書く」判定に使う。
    loaded: Option<Vec<u8>>,
}

impl DiskPipelineCache {
    /// アダプタからキャッシュファイルの所在を解決し、既存 blob があれば読む。
    pub fn discover(info: &wgpu::AdapterInfo, shader_hash: u64) -> Option<Self> {
        // Vulkan 以外は wgpu がパイプラインキャッシュ未対応（filename が None）。
        let file_name = wgpu::util::pipeline_cache_key(info)?;
        let path = cache_dir()?.join(file_name);
        let key = cache_key(info, shader_hash);
        let loaded = pipeline_cache::load(&path, &key);
        Some(Self { path, key, loaded })
    }

    /// 起動時に読めたキャッシュ blob（キー一致・非破損のときだけ `Some`）。
    pub fn loaded_blob(&self) -> Option<&[u8]> {
        self.loaded.as_deref()
    }

    /// renderer 構築後の blob を永続化する。読めた blob と同一なら書かない。失敗は警告のみ
    /// （キャッシュは最適化であり、書けなくても起動は成功のまま）。
    pub fn persist(&self, blob: &[u8]) {
        if self.loaded.as_deref() == Some(blob) {
            return;
        }
        if let Err(e) = self.persist_inner(blob) {
            log::warn!("pipeline cache: save failed ({}): {e}", self.path.display());
        } else {
            log::info!(
                "pipeline cache: saved {} bytes to {}",
                blob.len(),
                self.path.display()
            );
        }
    }

    fn persist_inner(&self, blob: &[u8]) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        // 途中終了で破損ファイルを残さないよう temp へ書いて rename（同一ディレクトリ内なので
        // 主要プラットフォームで原子的に置き換わる）。
        let tmp = self.path.with_extension("tmp");
        pipeline_cache::save(&tmp, &self.key, blob)?;
        fs::rename(&tmp, &self.path)
    }

    /// キャッシュファイルのパス（ログ用）。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(name, _)| *name == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn linux_prefers_xdg_cache_home_then_home_dot_cache() {
        let dir = cache_dir_from("linux", env(&[("XDG_CACHE_HOME", "/xdg"), ("HOME", "/h")]));
        assert_eq!(dir, Some(PathBuf::from("/xdg/hayate")));
        let dir = cache_dir_from("linux", env(&[("HOME", "/h")]));
        assert_eq!(dir, Some(PathBuf::from("/h/.cache/hayate")));
        assert_eq!(cache_dir_from("linux", env(&[])), None);
    }

    #[test]
    fn macos_and_windows_use_platform_cache_roots() {
        let dir = cache_dir_from("macos", env(&[("HOME", "/Users/u")]));
        assert_eq!(dir, Some(PathBuf::from("/Users/u/Library/Caches/hayate")));
        let dir = cache_dir_from("windows", env(&[("LOCALAPPDATA", "C:/u/AppData/Local")]));
        assert_eq!(dir, Some(PathBuf::from("C:/u/AppData/Local/hayate")));
        assert_eq!(cache_dir_from("unknown-os", env(&[("HOME", "/h")])), None);
    }

    #[test]
    fn cache_key_invalidates_on_driver_or_shader_change() {
        let base = driver_cache_key("gpu", "drv", "1.0", 42);
        assert_eq!(
            base,
            driver_cache_key("gpu", "drv", "1.0", 42),
            "同一入力で決定的"
        );
        assert_ne!(
            base,
            driver_cache_key("gpu", "drv", "2.0", 42),
            "ドライバ更新でキーが変わる"
        );
        assert_ne!(
            base,
            driver_cache_key("gpu", "drv", "1.0", 43),
            "シェーダ変更でキーが変わる"
        );
    }
}
