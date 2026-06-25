//! file picker capability 契約（ADR-0119）。モデル: `file_selector`
//! （`openFile -> XFile?` / `getSaveLocation -> path?`）。
//!
//! 返り値の `XFile`（path か stream か）は ADR-0119 が名指しした risk 筆頭。scaffold では
//! opaque な path 文字列だけを持ち、stream 化は実機実装時に決める。

use crate::capability::CapabilityError;

/// open ダイアログの拡張子フィルタ。空 = 任意。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub extensions: Vec<String>,
}

/// ユーザが選んだファイル（scaffold では opaque path のみ・stream 化は実装時）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedFile {
    pub path: String,
}

/// 保存先として選ばれた path。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavePath {
    pub path: String,
}

/// システムのファイル選択/保存ダイアログ。キャンセルは `Ok(None)`。
pub trait FilePicker {
    fn open_file(&mut self, filter: &FileFilter) -> Result<Option<PickedFile>, CapabilityError>;
    fn save_file(&mut self, suggested_name: &str) -> Result<Option<SavePath>, CapabilityError>;
}
