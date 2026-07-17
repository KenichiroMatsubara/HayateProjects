/// IME 変換中の文節1つ分の下線の太さ（ADR-0102）。Chromium は変換中の文節を太線、
/// 確定済みの周辺文節を細線で描く。EditContext `textformatupdate` の下線太さスタイルに対応する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositionUnderline {
    /// 変換前テキストまたは非アクティブ文節 — 細い下線。
    Thin,
    /// 変換中の文節（IME のアクティブセグメント）— 太い下線。
    Thick,
}

/// 変換文節1つ: preedit テキスト内のバイト範囲と下線の太さ。オフセットは preedit
/// 文字列基準（0 = 先頭バイト）で、アダプタが確定済み接頭辞を差し引いた後の
/// EditContext `textformatupdate` の範囲表現に一致する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionClause {
    pub start: usize,
    pub end: usize,
    pub underline: CompositionUnderline,
}

impl CompositionClause {
    /// EditContext `textformatupdate` 境界を渡るワイヤ形式をデコードする（ADR-0102）。
    /// `[start, end, weight, …]` の3つ組フラットストリームで、`weight == 0` が
    /// [`CompositionUnderline::Thin`]、非0が [`CompositionUnderline::Thick`]。
    /// 末尾の不完全な3つ組は無視する。
    pub fn from_wire(formats: &[u32]) -> Vec<CompositionClause> {
        formats
            .chunks_exact(3)
            .filter_map(|c| {
                let (start, end) = (c[0] as usize, c[1] as usize);
                if start >= end {
                    return None;
                }
                let underline = if c[2] == 0 {
                    CompositionUnderline::Thin
                } else {
                    CompositionUnderline::Thick
                };
                Some(CompositionClause {
                    start,
                    end,
                    underline,
                })
            })
            .collect()
    }
}

/// 変換中の IME 入力（ADR-0102）。preedit テキストと、EditContext `textformatupdate`
/// から渡される文節フォーマット範囲を持つ。文節がない場合、preedit 全体が単一の
/// 細線下線として描画される（IME が読みをセグメント分割する前の変換前の見た目）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    pub clauses: Vec<CompositionClause>,
}

/// 編集モーションの方向。`Backward`/`Forward` はテキスト上を水平に進み、`Up`/`Down`
/// は表示行間を垂直に移動する（ADR-0103）。垂直モーションは Parley の行ジオメトリを
/// 必要とするため複数行フィールドでは `ElementTree` 編集シームが解決する。単一行
/// フィールドには行がないので、純粋な `EditState` シームは `Up`/`Down` をフィールド
/// 先頭/末尾へのジャンプとして扱う（Chromium `<input>`: ↑ = 先頭, ↓ = 末尾）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
    Up,
    Down,
}

impl Direction {
    /// 水平ステップではなく垂直（行間）モーションかどうか。垂直モーションは
    /// sticky な目標カラムを保持し、水平モーションはリセットする。
    fn is_vertical(self) -> bool {
        matches!(self, Direction::Up | Direction::Down)
    }
}

/// 編集モーションの粒度（ADR-0103）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Grapheme,
    Word,
    /// 現在の表示行の境界 — Home/End（macOS では Cmd+←/→）。単一行（`<input>`）
    /// セマンティクスでは行 = フィールド全体なので、フィールド先頭/末尾に解決される。
    LineBoundary,
    /// フィールド全体の境界 — Ctrl+Home/End（macOS では Cmd+↑/↓）。
    DocBoundary,
}

impl Granularity {
    /// この粒度がキャレット相対の1グラフェム/単語ではなく、絶対的なフィールド/行
    /// 境界へステップするかどうか。境界モーション（Home/End）は矢印キーと違い、選択が
    /// あっても境界へジャンプする。
    fn is_boundary(self) -> bool {
        matches!(self, Granularity::LineBoundary | Granularity::DocBoundary)
    }
}

/// 単一の編集シーム [`EditState::apply`] を通して適用される編集コマンドの語彙
/// （ADR-0103, ADR-0071）。`Move` はキャレットを移動。`Extend` はアンカーを固定した
/// まま focus を動かして選択を伸縮。`Delete` は `direction` 方向に `granularity` 1ステップ
/// 分（または選択範囲）を削除。`SelectAll` はフィールド全体を選択。`Copy` / `Cut` /
/// `Paste` はクリップボードメンバーだが、システムクリップボードは Platform Adapter の
/// 責務（ADR-0097）。`EditState` はクリップボードを持たないため、[`EditState::apply`] は
/// 純粋状態メンバー（`Move` / `Extend` / `Delete` / `SelectAll`）のみ消費し、クリップ
/// ボードメンバーは未消費として報告し、その読み書きを `Clipboard` を所有する
/// `ElementTree` 編集シームに委ねる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditIntent {
    Move {
        granularity: Granularity,
        direction: Direction,
    },
    Extend {
        granularity: Granularity,
        direction: Direction,
    },
    Delete {
        granularity: Granularity,
        direction: Direction,
    },
    /// 複数行フィールドの現在選択を改行で置換する。
    InsertLineBreak,
    /// フィールド内容全体を選択（Ctrl/Cmd+A）。
    SelectAll,
    /// 選択をシステムクリップボードへコピー（Ctrl/Cmd+C）。状態変化なし。
    Copy,
    /// 選択を切り取り: クリップボードへコピーしてから削除（Ctrl/Cmd+X）。
    Cut,
    /// 選択をクリップボードのテキストで置換（Ctrl/Cmd+V）。
    Paste,
}

/// テキスト入力の編集モデル（ADR-0069）。TextInput 要素のみが所有する。
///
/// キャレットは統一 Selection モデル（ADR-0097）の縮退形。`selection_anchor` と
/// `cursor_byte_index` は `text_content` 内範囲のアンカー/focus バイトオフセットで、
/// 両者が一致すると選択は単一キャレットに縮退する（編集の一般的なケース）。
#[derive(Clone, Debug, Default)]
pub struct EditState {
    pub text_content: String,
    pub preedit: Option<Preedit>,
    /// 選択の focus（動く端 / キャレット位置）。
    pub cursor_byte_index: usize,
    /// 選択のアンカー（固定端）。縮退キャレットでは `cursor_byte_index` と等しい。
    pub selection_anchor: usize,
    /// 垂直（↑/↓）モーション用の sticky な目標カラム。表示行を跨ぐ際にキャレットが
    /// 狙うコンテンツローカル x で、短い行を通っても元のカラムを失わない（ADR-0103）。
    /// 垂直モーションが確立するまで `None`、水平モーションがクリアする。Parley
    /// ジオメトリを所有する `ElementTree` シームがコンテンツローカルピクセルで設定する。
    pub desired_x: Option<f32>,
}

impl EditState {
    pub fn display_text(&self) -> String {
        match &self.preedit {
            Some(p) => {
                // preedit はキャレット位置に挿入される（末尾固定ではない）。
                // text_content[..at] + preedit + text_content[at..]。
                let at = self.cursor_byte_index.min(self.text_content.len());
                let mut s = String::with_capacity(self.text_content.len() + p.text.len());
                s.push_str(&self.text_content[..at]);
                s.push_str(&p.text);
                s.push_str(&self.text_content[at..]);
                s
            }
            None => self.text_content.clone(),
        }
    }

    /// 表示テキスト（preedit 挿入済み）におけるキャレットのバイト位置。変換中は
    /// preedit の末尾にキャレットを置く（Chromium と同じ振る舞い）。変換が無ければ
    /// `cursor_byte_index` と一致する。
    pub fn display_cursor_byte_index(&self) -> usize {
        let at = self.cursor_byte_index.min(self.text_content.len());
        match &self.preedit {
            Some(p) => at + p.text.len(),
            None => at,
        }
    }

    /// アクティブな変換の下線範囲を**表示テキストのバイトオフセット**（キャレット位置の
    /// preedit に合わせ、キャレットまでの接頭辞分シフト済み）で、それぞれの太さ付きで
    /// 返す（ADR-0102）。
    /// 変換が非アクティブなら空。文節フォーマットがない場合は preedit 全体が単一の
    /// 細線範囲（IME が読みをセグメント分割する前の見た目）。
    pub fn composition_underlines(&self) -> Vec<(usize, usize, CompositionUnderline)> {
        let Some(preedit) = &self.preedit else {
            return Vec::new();
        };
        // preedit はキャレット位置に挿入されるので、下線も同じ位置を基準にする。
        let base = self.cursor_byte_index.min(self.text_content.len());
        if preedit.clauses.is_empty() {
            if preedit.text.is_empty() {
                return Vec::new();
            }
            return vec![(base, base + preedit.text.len(), CompositionUnderline::Thin)];
        }
        preedit
            .clauses
            .iter()
            .map(|c| (base + c.start, base + c.end, c.underline))
            .collect()
    }

    /// 選択がキャレットに縮退している（anchor == focus）とき true。
    pub fn is_caret(&self) -> bool {
        self.selection_anchor == self.cursor_byte_index
    }

    /// 選択バイト範囲 `(start, end)` をテキスト順に正規化して返す。選択が縮退
    /// （何も選択されていない）なら `None`。
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        if self.is_caret() {
            None
        } else {
            let a = self.selection_anchor;
            let f = self.cursor_byte_index;
            Some((a.min(f), a.max(f)))
        }
    }

    /// `anchor`/`focus` バイトオフセット（各々現在のテキストにクランプ）で
    /// （空でもよい）選択を設定する。
    pub fn set_selection(&mut self, anchor: usize, focus: usize) {
        let len = self.text_content.len();
        self.selection_anchor = anchor.min(len);
        self.cursor_byte_index = focus.min(len);
    }

    /// アンカーを固定したまま focus（キャレット）を `offset` へ移動する
    /// — Shift+矢印 / ドラッグ拡張のプリミティブ。
    pub fn move_focus(&mut self, offset: usize) {
        self.cursor_byte_index = offset.min(self.text_content.len());
    }

    /// 縦移動（↑/↓）を注入した [`CaretGeometry`] に対して純粋計算する
    /// （ADR-0122 決定 5）。sticky な goal column（[`desired_x`](Self::desired_x)）で
    /// 隣接表示行の最近 column に着地し、短い行を跨いでも元の列を保つ。`extend` は
    /// anchor を保つ（Shift+矢印）。先頭行より上はフィールド先頭、最終行より下は
    /// フィールド末尾へ（Chromium `<textarea>`）。整形行が無い（`line_count == 0`）と
    /// `false` を返し、呼び出し側は単一行意味論へフォールバックする。
    pub fn vertical_motion(
        &mut self,
        geometry: &dyn crate::element::caret_geometry::CaretGeometry,
        direction: Direction,
        extend: bool,
    ) -> bool {
        let delta: isize = match direction {
            Direction::Up => -1,
            Direction::Down => 1,
            // 縦移動は上下のみ。
            _ => return false,
        };
        let line_count = geometry.line_count();
        if line_count == 0 {
            return false;
        }
        let caret = self.cursor_byte_index;
        let current_line = geometry.line_of(caret);
        // 保存したゴール列を狙う。初回移動ではキャレットの現在 x。
        let goal_x = self.desired_x.unwrap_or_else(|| geometry.x_of(caret));
        let target_line = current_line as isize + delta;
        let offset = if target_line < 0 {
            // 先頭行より上 → フィールド先頭。
            0
        } else if target_line as usize >= line_count {
            // 最終行より下 → フィールド末尾。
            self.text_content.len()
        } else {
            geometry.byte_at_x_on_line(target_line as usize, goal_x)
        };
        if extend {
            self.move_focus(offset);
        } else {
            self.set_selection(offset, offset);
        }
        // ゴール列は移動後も残るので、短い行を抜ける ↑/↓ の連続は元の列へ戻る。
        self.desired_x = Some(goal_x);
        true
    }

    /// 表示行 Home/End を注入した [`CaretGeometry`] に対して計算する（ADR-0122 決定 5）。
    /// 現在の*表示*行（ソフトラップ後）の先頭（`Backward`）／末尾（`Forward`）へ
    /// キャレットを移す。`extend` は anchor を保つ（Shift+Home/End）。End はソフト
    /// ラップ境界の末尾改行を除外し最後の可視グリフへ着地する。整形行が無い・方向が
    /// 上下のときは `false`。
    pub fn display_line_boundary(
        &mut self,
        geometry: &dyn crate::element::caret_geometry::CaretGeometry,
        direction: Direction,
        extend: bool,
    ) -> bool {
        if geometry.line_count() == 0 {
            return false;
        }
        let line = geometry.line_of(self.cursor_byte_index);
        let (start, end) = geometry.line_bounds(line);
        let offset = match direction {
            Direction::Backward => start,
            Direction::Forward => {
                // ソフトラップ境界の空白／改行を除外し、End が行の最後の可視グリフに
                // 着地するようにする（Parley の `line_end` に一致）。
                let mut end = end;
                while end > start {
                    match self.text_content[..end].chars().next_back() {
                        Some(c) if c == '\n' || c == '\r' => end -= c.len_utf8(),
                        _ => break,
                    }
                }
                end
            }
            // Home/End は水平方向しか持たない。
            Direction::Up | Direction::Down => return false,
        };
        // Home/End は水平移動なので、粘着するゴール列を捨てる。
        self.desired_x = None;
        if extend {
            self.move_focus(offset);
        } else {
            self.set_selection(offset, offset);
        }
        true
    }

    /// 選択範囲を破棄し、現在の focus でキャレットに縮退する
    /// （single-active ルールの強制に使う、ADR-0097）。
    pub fn collapse(&mut self) {
        self.selection_anchor = self.cursor_byte_index;
    }

    /// 選択を `offset` のキャレットに縮退する。編集（insert / delete / set / commit）と
    /// 水平移動でのキャレット再配置の要所なので、sticky な垂直目標カラムも破棄する
    /// — `move_focus` / `set_selection` で再配置する垂直モーションのみが保持する。
    fn collapse_to(&mut self, offset: usize) {
        let o = offset.min(self.text_content.len());
        self.cursor_byte_index = o;
        self.selection_anchor = o;
        self.desired_x = None;
    }

    /// 選択が非空なら削除し、キャレットをその先頭に縮退する（replace-on-type
    /// プリミティブ）。何か削除されたかを返す。
    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            self.text_content.replace_range(start..end, "");
            self.collapse_to(start);
            true
        } else {
            false
        }
    }

    pub fn set(&mut self, text: &str) {
        self.text_content = text.to_string();
        self.preedit = None;
        self.collapse_to(self.text_content.len());
    }

    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text_content.push_str(text);
        self.collapse_to(self.text_content.len());
    }

    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // 範囲上での入力は置換になる（replace-on-type, ADR-0097）。
        self.delete_selection();
        let byte = self.cursor_byte_index.min(self.text_content.len());
        self.text_content.insert_str(byte, text);
        self.collapse_to(byte + text.len());
    }

    pub fn backspace(&mut self) -> bool {
        // 非空の選択は末尾1文字ではなく範囲ごと削除する。
        if self.delete_selection() {
            return true;
        }
        if self.text_content.is_empty() {
            return false;
        }
        let last_start = self
            .text_content
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text_content.truncate(last_start);
        self.collapse_to(self.text_content.len());
        true
    }

    pub fn set_preedit(&mut self, preedit: &str) {
        self.set_preedit_with_clauses(preedit, Vec::new());
    }

    /// preedit テキストを変換文節のフォーマット範囲と共に設定する（ADR-0102）。
    /// テキストを空にすると変換と文節ごと破棄される。
    pub fn set_preedit_with_clauses(&mut self, preedit: &str, clauses: Vec<CompositionClause>) {
        self.preedit = if preedit.is_empty() {
            None
        } else {
            Some(Preedit {
                text: preedit.to_string(),
                clauses,
            })
        };
    }

    pub fn commit_preedit(&mut self) {
        if let Some(preedit) = self.preedit.take() {
            // 末尾追加ではなくキャレット位置へ確定する（preedit はそこに表示されていた）。
            let at = self.cursor_byte_index.min(self.text_content.len());
            self.text_content.insert_str(at, &preedit.text);
            self.collapse_to(at + preedit.text.len());
        }
    }

    /// IME 変換確定: 単一の preedit→content 経路でコミットする。
    pub fn finish_composition(&mut self, committed: &str) {
        self.set_preedit(committed);
        self.commit_preedit();
    }

    /// 切り取り: 選択テキストを返して削除し（削除範囲の先頭でキャレットに縮退）、
    /// 選択が縮退しているなら `None` を返す（ADR-0097）。
    pub fn cut(&mut self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let removed = self.text_content[start..end].to_string();
        self.delete_selection();
        Some(removed)
    }

    /// 確定済み内容全体を `value` で置換する。先にアクティブな preedit を確定し、
    /// 変換中の IME 入力が置換を跨いで残らないようにする（`paste` と同じ
    /// preedit 確定の整合性）。表示テキストが実際に変化したかを返す。
    pub fn set_value(&mut self, value: &str) -> bool {
        let changed = self.display_text() != value;
        self.commit_preedit();
        self.set(value);
        changed
    }

    pub fn paste(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.commit_preedit();
        // 範囲上への貼り付けは置換になる（replace-on-type, ADR-0097）。
        self.insert(text);
        true
    }

    /// 共有のグラフェム/単語ステッパ（`selection.rs`）を再利用し、`offset` から
    /// `direction` 方向へ `granularity` 1ステップ分のバイトオフセットを返す。
    fn step(&self, granularity: Granularity, direction: Direction, offset: usize) -> usize {
        use crate::element::selection::{next_grapheme, next_word, prev_grapheme, prev_word};
        // 単一行の垂直セマンティクス: 行のないフィールドは ↑ をフィールド先頭、
        // ↓ を末尾へのジャンプとして扱う（Chromium `<input>`）。複数行の垂直モーション
        // は Parley ジオメトリを必要とし、`ElementTree` 編集シームで解決される。
        match direction {
            Direction::Up => return 0,
            Direction::Down => return self.text_content.len(),
            Direction::Backward | Direction::Forward => {}
        }
        match (granularity, direction) {
            (Granularity::Grapheme, Direction::Backward) => {
                prev_grapheme(&self.text_content, offset)
            }
            (Granularity::Grapheme, Direction::Forward) => {
                next_grapheme(&self.text_content, offset)
            }
            (Granularity::Word, Direction::Backward) => prev_word(&self.text_content, offset),
            (Granularity::Word, Direction::Forward) => next_word(&self.text_content, offset),
            // 単一行セマンティクス: 行も文書もフィールド全体を覆うので、どちらの
            // 境界もフィールド端に縮退する。複数行の表示行境界はツリーシームで解決する。
            (Granularity::LineBoundary | Granularity::DocBoundary, Direction::Backward) => 0,
            (Granularity::LineBoundary | Granularity::DocBoundary, Direction::Forward) => {
                self.text_content.len()
            }
            // 垂直方向は上で返済み。
            (_, Direction::Up | Direction::Down) => unreachable!("vertical handled above"),
        }
    }

    /// 単一の編集シーム（ADR-0103）。1つの [`EditIntent`] を適用し、消費されたかを返す。
    pub fn apply(&mut self, intent: EditIntent) -> bool {
        match intent {
            EditIntent::Move {
                granularity,
                direction,
            } => {
                // どちらの分岐も `collapse_to` で縮退し、sticky な目標カラムを破棄する
                // （単一行 ↑/↓ には保持すべき行がない）。Chromium: 選択上の素の矢印は
                // ステップせず方向側の端へ縮退、キャレット上では1単位ステップして縮退を
                // 保つ。境界モーション（Home/End）や垂直ジャンプは選択を無視してターゲットへ
                // 直行する（単一行 ↑ = フィールド先頭、↓ = 末尾）。
                match self.selection_range() {
                    Some((start, end))
                        if !granularity.is_boundary() && !direction.is_vertical() =>
                    {
                        let edge = match direction {
                            Direction::Backward => start,
                            Direction::Forward => end,
                            Direction::Up | Direction::Down => unreachable!("vertical excluded"),
                        };
                        self.collapse_to(edge);
                    }
                    _ => {
                        let next = self.step(granularity, direction, self.cursor_byte_index);
                        self.collapse_to(next);
                    }
                }
                true
            }
            EditIntent::Extend {
                granularity,
                direction,
            } => {
                if !direction.is_vertical() {
                    self.desired_x = None;
                }
                let next = self.step(granularity, direction, self.cursor_byte_index);
                self.move_focus(next);
                true
            }
            EditIntent::Delete {
                granularity,
                direction,
            } => {
                // 非空の選択は範囲ごと削除し（replace-on-type の一貫性）先頭に縮退する。
                // それ以外はキャレットから `direction` 方向へ granularity 1ステップ分削除する。
                if self.delete_selection() {
                    return true;
                }
                let from = self.cursor_byte_index;
                let to = self.step(granularity, direction, from);
                if from == to {
                    return false; // テキスト境界 — 削除対象なし
                }
                let (start, end) = (from.min(to), from.max(to));
                self.text_content.replace_range(start..end, "");
                self.collapse_to(start);
                true
            }
            EditIntent::InsertLineBreak => {
                self.insert("\n");
                true
            }
            EditIntent::SelectAll => {
                // アンカーを先頭、focus を末尾に — フィールド全体が選択範囲になる
                // （空フィールドでは 0 で縮退）。
                self.set_selection(0, self.text_content.len());
                true
            }
            // クリップボードメンバーは Platform Adapter 境界を跨ぐ（ADR-0097）。
            // EditState はクリップボードを持たないので、選択の読み出し（Copy）、
            // 取得後削除（Cut）、テキストの取り込み（Paste）はできない。`Clipboard` を
            // 持つ `ElementTree` シームが解決する。ここではこの層が中途半端に適用しない
            // よう未消費として報告する。
            EditIntent::Copy | EditIntent::Cut | EditIntent::Paste => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backspace_removes_last_scalar() {
        let mut edit = EditState::default();
        edit.append("hello");
        assert!(edit.backspace());
        assert_eq!(edit.text_content, "hell");
        assert_eq!(edit.cursor_byte_index, 4);
    }

    use crate::element::caret_geometry::TableCaretGeometry;

    /// 縦移動テスト用の 2 表示行テーブル（ハード改行ありの "abcdef\nabcdef"）。
    /// - 行0: `[0, 7)`（末尾 `\n` を含む）列 0→0px, 6→60px
    /// - 行1: `[7, 13)`            列 7→0px, 13→60px
    fn two_line_table() -> TableCaretGeometry {
        TableCaretGeometry::new(
            vec![
                (0, 7, vec![(0, 0.0), (6, 60.0)]),
                (7, 13, vec![(7, 0.0), (13, 60.0)]),
            ],
            10.0,
        )
    }

    #[test]
    fn vertical_motion_lands_on_nearest_column_of_adjacent_line() {
        let geo = two_line_table();
        let mut edit = EditState::default();
        edit.set("abcdef\nabcdef");
        edit.set_selection(13, 13); // 末尾、行1 の列 60px

        assert!(edit.vertical_motion(&geo, Direction::Up, false));
        assert_eq!(
            edit.cursor_byte_index, 6,
            "↑ は上の行の同じ列（byte 6, 60px）へ"
        );
        assert!(edit.is_caret());

        assert!(edit.vertical_motion(&geo, Direction::Down, false));
        assert_eq!(edit.cursor_byte_index, 13, "↓ は元の行・列へ戻る");
    }

    #[test]
    fn vertical_motion_keeps_goal_column_across_a_short_line() {
        // 中央が短い 3 行：長い行末から ↑↑ で短い行を跨ぎ、元の列へ戻る。
        let geo = TableCaretGeometry::new(
            vec![
                (0, 6, vec![(0, 0.0), (5, 50.0)]),
                (6, 9, vec![(6, 0.0), (8, 20.0)]), // 短い行（最大 20px）
                (9, 14, vec![(9, 0.0), (14, 50.0)]),
            ],
            10.0,
        );
        let mut edit = EditState::default();
        edit.set("world\nhi\nworld");
        edit.set_selection(14, 14); // 行2 の 50px

        assert!(edit.vertical_motion(&geo, Direction::Up, false));
        assert_eq!(edit.cursor_byte_index, 8, "短い行ではその末尾(8)にクランプ");
        assert_eq!(edit.desired_x, Some(50.0), "goal column は保持される");

        assert!(edit.vertical_motion(&geo, Direction::Up, false));
        assert_eq!(
            edit.cursor_byte_index, 5,
            "再度 ↑ で元の列(50px→byte 5)へ戻る"
        );
    }

    #[test]
    fn vertical_motion_past_top_and_bottom_jumps_to_field_ends() {
        let geo = two_line_table();
        let mut edit = EditState::default();
        edit.set("abcdef\nabcdef");

        edit.set_selection(2, 2); // 行0
        assert!(edit.vertical_motion(&geo, Direction::Up, false));
        assert_eq!(edit.cursor_byte_index, 0, "先頭行より上 → フィールド先頭");

        edit.set_selection(9, 9); // 行1
        assert!(edit.vertical_motion(&geo, Direction::Down, false));
        assert_eq!(edit.cursor_byte_index, 13, "最終行より下 → フィールド末尾");
    }

    #[test]
    fn vertical_motion_extend_keeps_anchor() {
        let geo = two_line_table();
        let mut edit = EditState::default();
        edit.set("abcdef\nabcdef");
        edit.set_selection(6, 6); // anchor=focus=6（行0 末尾）

        assert!(edit.vertical_motion(&geo, Direction::Down, true));
        assert_eq!(edit.selection_anchor, 6, "extend は anchor を保つ");
        assert_eq!(edit.cursor_byte_index, 13, "focus は下の行の同じ列へ");
    }

    #[test]
    fn vertical_motion_without_lines_is_noop_and_returns_false() {
        let geo = TableCaretGeometry::new(Vec::new(), 10.0);
        let mut edit = EditState::default();
        edit.set("abc");
        edit.set_selection(1, 1);
        assert!(
            !edit.vertical_motion(&geo, Direction::Up, false),
            "行が無ければ false"
        );
        assert_eq!(edit.cursor_byte_index, 1, "状態は変わらない");
    }

    #[test]
    fn display_line_boundary_moves_to_current_line_ends() {
        let geo = two_line_table();
        let mut edit = EditState::default();
        edit.set("abcdef\nabcdef");
        edit.set_selection(3, 3); // 行0 の途中
        edit.desired_x = Some(99.0); // Home/End は捨てるはず

        assert!(edit.display_line_boundary(&geo, Direction::Backward, false));
        assert_eq!(edit.cursor_byte_index, 0, "Home → 表示行の先頭");
        assert_eq!(edit.desired_x, None, "水平移動は goal column を捨てる");

        edit.set_selection(3, 3);
        assert!(edit.display_line_boundary(&geo, Direction::Forward, false));
        assert_eq!(
            edit.cursor_byte_index, 6,
            "End → 表示行の末尾（末尾の改行は除外）",
        );
    }

    fn move_grapheme(d: Direction) -> EditIntent {
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    #[test]
    fn select_all_spans_the_whole_content() {
        // SelectAll（Ctrl/Cmd+A）は編集シームの純粋状態メンバー。フィールド先頭に
        // アンカーし focus を末尾へ動かすので、直前のキャレットに関係なく内容全体が
        // 選択範囲になる。
        let mut edit = EditState::default();
        edit.set("héllo"); // キャレットは末尾で縮退
        edit.set_selection(2, 2);

        assert!(edit.apply(EditIntent::SelectAll));

        assert_eq!(
            edit.selection_range(),
            Some((0, "héllo".len())),
            "the entire content is selected",
        );
    }

    #[test]
    fn select_all_on_empty_content_stays_collapsed() {
        // 選択対象なし: 範囲は 0 で縮退する（誤った選択を作らない）。
        let mut edit = EditState::default();
        assert!(edit.apply(EditIntent::SelectAll));
        assert!(edit.is_caret());
        assert_eq!(edit.cursor_byte_index, 0);
    }

    #[test]
    fn move_to_line_boundary_jumps_to_field_start_and_end() {
        // 単一行セマンティクス: 行末 = フィールド末尾。Home（Backward）はキャレットを
        // 0 へ、End（Forward）は内容長へ縮退する。
        let mut edit = EditState::default();
        edit.set("hello"); // キャレットは末尾(5)
        edit.set_selection(2, 2); // キャレットは中央

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "Home lands at the field start");
        assert!(edit.is_caret());

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "End lands at the field end");
        assert!(edit.is_caret());
    }

    fn extend_grapheme(d: Direction) -> EditIntent {
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    fn delete_grapheme(d: Direction) -> EditIntent {
        EditIntent::Delete {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    #[test]
    fn delete_over_a_selection_removes_the_range_and_collapses_to_its_start() {
        // Backspace（Backward）も Delete（Forward）も、隣接1文字ではなく選択範囲全体を
        // 削除し、範囲先頭に縮退する。
        for direction in [Direction::Backward, Direction::Forward] {
            let mut edit = EditState::default();
            edit.set("hello");
            edit.set_selection(1, 4); // "ell" を選択
            assert!(edit.apply(delete_grapheme(direction)));
            assert_eq!(edit.text_content, "ho", "{direction:?}: the range is gone");
            assert_eq!(
                edit.cursor_byte_index, 1,
                "{direction:?}: collapses to range start"
            );
            assert!(edit.is_caret(), "{direction:?}: collapsed");
        }
    }

    #[test]
    fn delete_forward_grapheme_removes_the_char_after_the_caret() {
        let mut edit = EditState::default();
        edit.set("aあb"); // キャレットは末尾(5)
        edit.set_selection(0, 0); // キャレットは先頭
        assert!(edit.apply(delete_grapheme(Direction::Forward)));
        assert_eq!(edit.text_content, "あb", "removes the leading 'a'");
        assert_eq!(
            edit.cursor_byte_index, 0,
            "caret stays at the deletion point"
        );
        assert!(edit.apply(delete_grapheme(Direction::Forward)));
        assert_eq!(edit.text_content, "b", "removes the 3-byte 'あ' whole");
        assert_eq!(edit.cursor_byte_index, 0);
        assert!(edit.is_caret());
    }

    #[test]
    fn delete_at_the_text_boundary_is_a_no_op() {
        let mut edit = EditState::default();
        edit.set("hi"); // キャレットは末尾(2)
        assert!(
            !edit.apply(delete_grapheme(Direction::Forward)),
            "nothing past the end"
        );
        assert_eq!(edit.text_content, "hi");
        edit.set_selection(0, 0); // キャレットは先頭
        assert!(
            !edit.apply(delete_grapheme(Direction::Backward)),
            "nothing before the start"
        );
        assert_eq!(edit.text_content, "hi");
    }

    #[test]
    fn delete_backward_grapheme_removes_the_char_before_the_caret() {
        let mut edit = EditState::default();
        edit.set("aあb"); // キャレットは末尾(5)
        assert!(edit.apply(delete_grapheme(Direction::Backward)));
        assert_eq!(edit.text_content, "aあ", "removes the trailing 'b'");
        assert_eq!(edit.cursor_byte_index, 4, "caret lands where 'b' began");
        assert!(edit.apply(delete_grapheme(Direction::Backward)));
        assert_eq!(edit.text_content, "a", "removes the 3-byte 'あ' whole");
        assert_eq!(edit.cursor_byte_index, 1);
        assert!(edit.is_caret());
    }

    #[test]
    fn move_forward_grapheme_steps_the_caret_one_char() {
        let mut edit = EditState::default();
        edit.set("aあb"); // キャレットは末尾(5)
        edit.set_selection(0, 0); // キャレットは先頭
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(edit.cursor_byte_index, 1, "advances past 'a'");
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(edit.cursor_byte_index, 4, "advances past the 3-byte 'あ'");
        assert!(edit.is_caret(), "a Move stays collapsed");
    }

    #[test]
    fn single_line_vertical_moves_to_field_start_and_end() {
        // Chromium `<input>`: 行がないと ↑ はフィールド先頭、↓ は末尾へジャンプする。
        // EditState がこの純粋な単一行セマンティクスを持ち、ジオメトリ駆動の複数行
        // ケースは ElementTree シームで解決する。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2); // キャレットは中央

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Up,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "↑ → field start");
        assert!(edit.is_caret());

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "↓ → field end");
        assert!(edit.is_caret());
    }

    #[test]
    fn single_line_vertical_jumps_over_a_selection_to_the_field_end() {
        // 水平矢印（選択端へ縮退する）と違い、↑/↓ は選択を無視してフィールド境界へ
        // 直行する。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" を選択、focus は 4

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        }));
        assert_eq!(
            edit.cursor_byte_index, 5,
            "↓ jumps past the selection to the end"
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn shift_vertical_extends_to_the_field_ends_in_a_single_line() {
        // 単一行フィールドでの Shift+↑/↓ は、アンカーを固定したまま選択をフィールド
        // 先頭/末尾へ拡張する（Move ジャンプの Extend 版）。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2);

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Up,
        }));
        assert_eq!(edit.selection_anchor, 2, "anchor stays put");
        assert_eq!(
            edit.cursor_byte_index, 0,
            "Shift+↑ extends to the field start"
        );
        assert_eq!(edit.selection_range(), Some((0, 2)));
    }

    #[test]
    fn a_horizontal_move_clears_the_sticky_goal_column() {
        // 目標カラムは垂直モーション間は保持されるが、キャレットが水平移動した瞬間に
        // リセットされる（ADR-0103）。さもないと後の ↑/↓ が古いカラムに戻ってしまう。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.desired_x = Some(42.0);
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(
            edit.desired_x, None,
            "a horizontal step resets the goal column"
        );
    }

    #[test]
    fn editing_clears_the_sticky_goal_column() {
        // 入力はキャレットを再配置するので、次の ↑/↓ は古いカラムではなく新しい
        // カラムから狙う必要がある。挿入は目標カラムをクリアする。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2);
        edit.desired_x = Some(42.0);
        edit.insert("X");
        assert_eq!(edit.desired_x, None, "an edit resets the goal column");
    }

    #[test]
    fn move_to_doc_boundary_jumps_to_field_start_and_end() {
        // Ctrl+Home/End。単一行セマンティクスでは文書境界 = 行境界なので、どちらも
        // フィールド端に縮退する。
        let mut edit = EditState::default();
        edit.set("hello world");
        edit.set_selection(4, 4);

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "Ctrl+Home lands at the start");

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 11, "Ctrl+End lands at the end");
        assert!(edit.is_caret());
    }

    #[test]
    fn move_to_boundary_over_a_selection_jumps_to_the_boundary_not_the_edge() {
        // 素の矢印（選択端へ縮退する）と違い、選択上の Home/End はフィールド境界へ
        // ジャンプしてそこで縮退する。
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" を選択、focus は 4

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(
            edit.cursor_byte_index, 5,
            "End jumps past the selection's right edge (4) to the field end (5)",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn move_backward_grapheme_steps_the_caret_left() {
        let mut edit = EditState::default();
        edit.set("aあb"); // キャレットは末尾(5)
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(edit.cursor_byte_index, 4, "retreats past 'b'");
        assert!(edit.is_caret());
    }

    #[test]
    fn move_forward_over_a_selection_collapses_to_its_right_edge() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" を選択、focus は 4
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(
            edit.cursor_byte_index, 4,
            "collapses to the right edge, does not step to 5",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn move_backward_over_a_selection_collapses_to_its_left_edge() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(4, 1); // "ell" を選択、focus は 1（左方向ドラッグ）
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(
            edit.cursor_byte_index, 1,
            "collapses to the left edge regardless of which end the focus was",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn extend_grapheme_moves_the_focus_keeping_the_anchor() {
        let mut edit = EditState::default();
        edit.set("hello"); // キャレットは末尾(5)
        assert!(edit.apply(extend_grapheme(Direction::Backward)));
        assert!(edit.apply(extend_grapheme(Direction::Backward)));
        assert_eq!(
            edit.selection_anchor, 5,
            "anchor stays fixed at the start point"
        );
        assert_eq!(edit.cursor_byte_index, 3, "focus retreats two chars");
        assert_eq!(edit.selection_range(), Some((3, 5)), "selects 'lo'");
        // 逆向きに前方へ拡張すると、範囲はアンカーへ向かって縮む。
        assert!(edit.apply(extend_grapheme(Direction::Forward)));
        assert_eq!(
            edit.cursor_byte_index, 4,
            "focus advances, shrinking the range"
        );
    }

    #[test]
    fn move_and_extend_by_word_use_the_shared_word_steppers() {
        let mut edit = EditState::default();
        edit.set("hello world"); // キャレットは末尾(11)
        edit.set_selection(0, 0); // キャレットは先頭
        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(
            edit.cursor_byte_index, 5,
            "word move lands at end of 'hello'"
        );
        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(
            edit.cursor_byte_index, 11,
            "word extend reaches end of 'world'"
        );
        assert_eq!(edit.selection_range(), Some((5, 11)));
    }

    #[test]
    fn delete_by_word_removes_a_whole_word_in_each_direction() {
        // Word 粒度の Delete はキャレットから単語境界（`prev_word` / `next_word`）まで
        // 削除する。Ctrl/Alt+Backspace/Delete の背後のモデル。
        let mut edit = EditState::default();
        edit.set("hello world"); // キャレットは末尾(11)
        assert!(edit.apply(EditIntent::Delete {
            granularity: Granularity::Word,
            direction: Direction::Backward,
        }));
        assert_eq!(
            edit.text_content, "hello ",
            "the word before the caret goes"
        );
        assert_eq!(
            edit.cursor_byte_index, 6,
            "caret collapses to the word start"
        );

        edit.set_selection(0, 0); // キャレットをフィールド先頭へ
        assert!(edit.apply(EditIntent::Delete {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.text_content, " ", "the word after the caret goes");
        assert_eq!(
            edit.cursor_byte_index, 0,
            "caret stays at the deletion point"
        );
    }

    #[test]
    fn extend_to_boundary_selects_to_the_field_end_keeping_the_anchor() {
        // フィールド途中のキャレットからの Shift+End は、アンカーを固定したまま
        // キャレットからフィールド末尾まで選択する。続く Shift+Home は先頭まで選択し直す。
        let mut edit = EditState::default();
        edit.set("hello world");
        edit.set_selection(6, 6); // "world" の手前にキャレット

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.selection_anchor, 6, "anchor stays put");
        assert_eq!(edit.cursor_byte_index, 11, "focus reaches the field end");
        assert_eq!(edit.selection_range(), Some((6, 11)), "selects 'world'");

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.selection_anchor, 6, "anchor still fixed");
        assert_eq!(
            edit.cursor_byte_index, 0,
            "focus crosses to the field start"
        );
        assert_eq!(edit.selection_range(), Some((0, 6)), "now selects 'hello '");
    }

    #[test]
    fn paste_commits_preedit_first() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit("CD");
        assert!(edit.paste("xy"));
        assert_eq!(edit.text_content, "abCDxy");
        assert!(edit.preedit.is_none());
    }

    #[test]
    fn typing_replaces_the_selected_range() {
        let mut edit = EditState::default();
        edit.set("hello"); // キャレットは末尾で縮退
        edit.set_selection(1, 4); // "ell" を選択
        assert!(!edit.is_caret());
        edit.insert("X");
        assert_eq!(edit.text_content, "hXo");
        assert_eq!(
            edit.cursor_byte_index, 2,
            "caret sits after the inserted text"
        );
        assert!(edit.is_caret(), "the range collapses once it is replaced");
    }

    #[test]
    fn backspace_deletes_the_selected_range_not_one_char() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" を選択
        assert!(edit.backspace());
        assert_eq!(edit.text_content, "ho");
        assert_eq!(
            edit.cursor_byte_index, 1,
            "caret collapses to the range start"
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn paste_replaces_the_selected_range() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(0, 5); // 単語全体
        assert!(edit.paste("bye"));
        assert_eq!(edit.text_content, "bye");
        assert_eq!(edit.cursor_byte_index, 3);
        assert!(edit.is_caret());
    }

    #[test]
    fn finish_composition_uses_commit_preedit() {
        let mut edit = EditState::default();
        edit.append("abc");
        edit.set_preedit("DEF");
        edit.finish_composition("愛");
        assert_eq!(edit.text_content, "abc愛");
        assert!(edit.preedit.is_none());
    }

    #[test]
    fn set_value_replaces_content_and_finalizes_active_preedit() {
        let mut edit = EditState::default();
        edit.append("abc");
        edit.set_preedit("DEF"); // 変換中の IME 入力
        assert!(edit.set_value("xyz"), "replacing the value is a change");
        assert_eq!(edit.text_content, "xyz", "value is fully replaced");
        assert!(edit.preedit.is_none(), "composition must not linger");
        assert_eq!(edit.display_text(), "xyz");
        assert_eq!(
            edit.cursor_byte_index, 3,
            "caret sits at the end of the value"
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn preedit_retains_clause_format_ranges() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅう",
            vec![CompositionClause {
                start: 0,
                end: 9,
                underline: CompositionUnderline::Thick,
            }],
        );
        // 文節の太さが保持され、表示テキストは依然として連結される。
        let preedit = edit.preedit.as_ref().expect("composition active");
        assert_eq!(preedit.text, "ぎゅう");
        assert_eq!(preedit.clauses[0].underline, CompositionUnderline::Thick);
        assert_eq!(edit.display_text(), "abぎゅう");
    }

    #[test]
    fn unformatted_preedit_underlines_the_whole_run_thin() {
        // 変換前: まだ文節分割なし ⇒ preedit 全体に細線下線1本。確定済み接頭辞
        // （"ab" = 2バイト）分シフト済み。
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit("xyz");
        assert_eq!(
            edit.composition_underlines(),
            vec![(2, 5, CompositionUnderline::Thin)],
        );
    }

    #[test]
    fn clause_split_underlines_each_segment_in_display_offsets() {
        // 変換中、IME は読みを文節に分割し、アクティブな文節が太線になる。返される
        // オフセットは表示テキスト基準（"ab" の後ろ）。
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅうにゅう",
            vec![
                CompositionClause {
                    start: 0,
                    end: 9,
                    underline: CompositionUnderline::Thick,
                },
                CompositionClause {
                    start: 9,
                    end: 18,
                    underline: CompositionUnderline::Thin,
                },
            ],
        );
        assert_eq!(
            edit.composition_underlines(),
            vec![
                (2, 11, CompositionUnderline::Thick),
                (11, 20, CompositionUnderline::Thin),
            ],
        );
    }

    #[test]
    fn no_composition_has_no_underlines() {
        let mut edit = EditState::default();
        edit.set("hello");
        assert!(edit.composition_underlines().is_empty());
    }

    #[test]
    fn from_wire_decodes_format_triples() {
        // [start, end, weight] の3つ組。weight 0 = 細線、非0 = 太線。
        let clauses = CompositionClause::from_wire(&[0, 9, 1, 9, 18, 0]);
        assert_eq!(
            clauses,
            vec![
                CompositionClause {
                    start: 0,
                    end: 9,
                    underline: CompositionUnderline::Thick
                },
                CompositionClause {
                    start: 9,
                    end: 18,
                    underline: CompositionUnderline::Thin
                },
            ],
        );
        // 退化した（空/反転）範囲と末尾の不完全な3つ組は破棄される。
        assert!(CompositionClause::from_wire(&[5, 5, 0, 7]).is_empty());
    }

    #[test]
    fn commit_clears_composition_decoration() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅう",
            vec![CompositionClause {
                start: 0,
                end: 9,
                underline: CompositionUnderline::Thick,
            }],
        );
        edit.commit_preedit();
        assert_eq!(edit.text_content, "abぎゅう");
        assert!(edit.preedit.is_none());
        assert!(
            edit.composition_underlines().is_empty(),
            "committing the composition clears its underlines",
        );
    }

    #[test]
    fn preedit_displays_and_commits_at_the_caret_not_the_tail() {
        // ユーザー報告バグの回帰: キャレットが中央でも、preedit と確定がキャレット
        // 位置に入る（末尾に飛ばない）。
        let mut edit = EditState::default();
        edit.set("helloworld"); // キャレットは末尾(10)で縮退
        edit.set_selection(5, 5); // hello|world
        edit.set_preedit("X");
        assert_eq!(
            edit.display_text(),
            "helloXworld",
            "preedit shows at the caret"
        );
        assert_eq!(
            edit.display_cursor_byte_index(),
            6,
            "display caret sits at the end of the preedit",
        );
        edit.commit_preedit();
        assert_eq!(
            edit.text_content, "helloXworld",
            "commit lands at the caret"
        );
        assert_eq!(
            edit.cursor_byte_index, 6,
            "caret advances past the inserted text"
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn composition_underlines_track_the_caret() {
        // 下線も preedit 位置（キャレット基準）に揃う。
        let mut edit = EditState::default();
        edit.set("abXY"); // キャレット末尾(4)
        edit.set_selection(2, 2); // ab|XY
        edit.set_preedit_with_clauses(
            "き",
            vec![CompositionClause {
                start: 0,
                end: 3,
                underline: CompositionUnderline::Thick,
            }],
        );
        assert_eq!(edit.display_text(), "abきXY");
        assert_eq!(
            edit.composition_underlines(),
            vec![(2, 5, CompositionUnderline::Thick)],
            "underline spans the preedit at the caret (bytes 2..5), not at the tail",
        );
    }

    #[test]
    fn set_value_to_identical_committed_content_is_not_a_change() {
        let mut edit = EditState::default();
        edit.set("abc");
        assert!(
            !edit.set_value("abc"),
            "no-op replacement reports no change"
        );
    }
}
