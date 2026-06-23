//! プラットフォーム非依存の音声出力 capability 契約（ADR-0117）。
//!
//! 音声出力は family（mobile / desktop）内で統一できる platform-bound capability で、
//! Family Adapter が `cfg(target_os)` でビルド時に片方の leaf（Android = `AudioTrack`
//! 相当 / iOS = `AVAudioEngine` 相当）をリンクして単一 facade を上位へ供給する。
//! その契約の正本は常に Core が持つ（`ImeBridge` / `Surface` / `FontFetcher` と同型・
//! ADR-0068/0069）。leaf は本契約を満たす薄い platform glue だけを置く。
//!
//! バッファ長・サンプルレート・チャンネル数といった可変値はすべて本モジュールの
//! named constant に集約する（inline マジックナンバー無し）。確定値は実機検証を伴う
//! 完全人力 follow-up に委ねる placeholder であり、ここを一点で直せば全 leaf に効く。
//!
//! 実機 SDK を要さず全ターゲットでコンパイル/テストできる（純粋な値型と trait のみ）。

/// 既定のサンプルレート（Hz）。48 kHz は Android `AudioTrack` / iOS の出力で共通に
/// 扱える placeholder（確定は完全人力 follow-up）。
pub const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;

/// 既定のチャンネル数（ステレオ）。
pub const DEFAULT_CHANNEL_COUNT: u16 = 2;

/// 既定の 1 バッファあたりフレーム数（チャンネルをまたぐ 1 サンプル時刻の束）。
/// レイテンシとアンダーラン耐性のトレードオフの placeholder。
pub const DEFAULT_BUFFER_FRAMES: u32 = 1_024;

/// Core が platform に開けさせたい音声ストリームの形式。leaf はこの形式を
/// native の出力設定（`AudioTrack` のチャンネルマスク / `AVAudioFormat`）へ写すだけで、
/// 形式の決定ロジックを platform 個別に再導出しない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate_hz: u32,
    pub channel_count: u16,
    pub buffer_frames: u32,
}

impl AudioFormat {
    /// named constant だけで構成した既定形式。
    pub const DEFAULT: AudioFormat = AudioFormat {
        sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
        channel_count: DEFAULT_CHANNEL_COUNT,
        buffer_frames: DEFAULT_BUFFER_FRAMES,
    };

    /// 1 バッファ分のインターリーブされた f32 PCM サンプル数（フレーム数 × チャンネル数）。
    /// leaf の platform バッファ確保が共通に使う純粋計算。
    pub fn buffer_len(&self) -> usize {
        self.buffer_frames as usize * self.channel_count as usize
    }
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// platform 音声出力のシーム（ADR-0117）。Family Adapter が `cfg(target_os)` で選んだ
/// leaf（Android `AudioTrack` / iOS `AVAudioEngine`）がこれを実装し、facade として
/// 上位へ露出する。Core は契約だけを所有し、ストリームの開閉と PCM 供給という最小の
/// ライフサイクルに閉じる。発音判定や形式決定を leaf に再導出させない。
pub trait AudioOutput {
    /// `format` で platform 出力ストリームを開く。
    fn open(&mut self, format: AudioFormat);
    /// インターリーブされた f32 PCM フレームを 1 バッファ分、再生キューへ供給する。
    fn submit(&mut self, frames: &[f32]);
    /// platform 出力ストリームを閉じる。
    fn close(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_built_from_named_constants() {
        let format = AudioFormat::DEFAULT;
        assert_eq!(format.sample_rate_hz, DEFAULT_SAMPLE_RATE_HZ);
        assert_eq!(format.channel_count, DEFAULT_CHANNEL_COUNT);
        assert_eq!(format.buffer_frames, DEFAULT_BUFFER_FRAMES);
    }

    /// 契約を満たす test double。leaf を持たないホストでも `AudioOutput` のライフ
    /// サイクルを公開インターフェース越しに検証できる。
    #[derive(Default)]
    struct RecordingOutput {
        opened: Option<AudioFormat>,
        submitted: Vec<usize>,
        closed: bool,
    }

    impl AudioOutput for RecordingOutput {
        fn open(&mut self, format: AudioFormat) {
            self.opened = Some(format);
        }
        fn submit(&mut self, frames: &[f32]) {
            self.submitted.push(frames.len());
        }
        fn close(&mut self) {
            self.closed = true;
        }
    }

    #[test]
    fn audio_output_drives_open_submit_close_lifecycle() {
        let mut out = RecordingOutput::default();
        out.open(AudioFormat::DEFAULT);
        out.submit(&vec![0.0_f32; AudioFormat::DEFAULT.buffer_len()]);
        out.close();

        assert_eq!(out.opened, Some(AudioFormat::DEFAULT));
        assert_eq!(out.submitted, vec![AudioFormat::DEFAULT.buffer_len()]);
        assert!(out.closed);
    }

    #[test]
    fn buffer_len_is_frames_times_channels() {
        let stereo = AudioFormat {
            sample_rate_hz: 48_000,
            channel_count: 2,
            buffer_frames: 1_024,
        };
        assert_eq!(stereo.buffer_len(), 2_048);

        let mono = AudioFormat {
            sample_rate_hz: 44_100,
            channel_count: 1,
            buffer_frames: 512,
        };
        assert_eq!(mono.buffer_len(), 512);
    }
}
