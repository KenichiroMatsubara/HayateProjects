//! iOS leaf の audio glue（ADR-0117）。
//!
//! platform-free な音声出力契約 [`AudioOutput`] の正本は `hayate_core` が持つ。本モジュールは
//! その型を re-export し、iOS 固有の glue — f32 PCM を `AVAudioEngine`（`AVAudioPCMBuffer` /
//! float フォーマット）へ送る native 呼び出し（`hayate_ios_audio_*`、Swift ホストが
//! AVAudioEngine に写す）— だけを `#[cfg(target_os = "ios")]` で薄く置く。バッファ容量の算術は
//! host-testable な純粋部分に寄せ、汚い FFI glue を薄く保つ（`surface_lifecycle` / `ime_bridge`
//! と同パターン）。Android leaf（`AudioTrack`）の de-risk 構造を鏡写しにする。
//!
//! 実機（Mac/AVAudioEngine）での発音検証はサンドボックスに Mac SDK が無いため AFK 範囲外。
//! 純粋部分はホストで単体テストし、FFI glue はソース走査ガードで封じ込める。

#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::{AudioFormat, AudioOutput};

/// f32 PCM 1 サンプルあたりのバイト数（`kAudioFormatFlagIsFloat`、32-bit）。
const BYTES_PER_PCM_FLOAT_SAMPLE: usize = 4;

/// `format` の 1 バッファの `AVAudioPCMBuffer` フレーム容量（純粋計算）。AVAudioEngine は
/// フレーム単位で容量を取るため、インターリーブ済みサンプル数ではなくフレーム数を返す。
pub fn frame_capacity(format: AudioFormat) -> u32 {
    format.buffer_frames
}

/// `format` の 1 バッファを `AVAudioPCMBuffer` へ詰めるのに要するバイト数（純粋計算）。
pub fn avaudio_buffer_bytes(format: AudioFormat) -> usize {
    format.buffer_len() * BYTES_PER_PCM_FLOAT_SAMPLE
}

#[cfg(target_os = "ios")]
pub use platform::AvAudioEngineOutput;

#[cfg(target_os = "ios")]
mod platform {
    use super::*;

    // Swift ホストが `AVAudioEngine` / `AVAudioPlayerNode`（float フォーマット）へ写す native 境界。
    // ソフトキーボード制御 FFI（`ime_bridge`）と同じく、この音声 FFI は本モジュールに封じ込める。
    extern "C" {
        fn hayate_ios_audio_open(sample_rate_hz: u32, channel_count: u16, frame_capacity: u32);
        fn hayate_ios_audio_write(frames: *const f32, len: usize);
        fn hayate_ios_audio_close();
    }

    /// iOS の [`AudioOutput`] 実装。`AVAudioEngine` を介して f32 PCM を発音する薄い glue。
    #[derive(Default)]
    pub struct AvAudioEngineOutput;

    impl AudioOutput for AvAudioEngineOutput {
        fn open(&mut self, format: AudioFormat) {
            unsafe {
                hayate_ios_audio_open(
                    format.sample_rate_hz,
                    format.channel_count,
                    frame_capacity(format),
                );
            }
        }

        fn submit(&mut self, frames: &[f32]) {
            unsafe { hayate_ios_audio_write(frames.as_ptr(), frames.len()) }
        }

        fn close(&mut self) {
            unsafe { hayate_ios_audio_close() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_capacity_is_the_format_buffer_frames() {
        assert_eq!(
            frame_capacity(AudioFormat::DEFAULT),
            AudioFormat::DEFAULT.buffer_frames
        );
    }

    #[test]
    fn buffer_bytes_is_samples_times_pcm_float_width() {
        let format = AudioFormat::DEFAULT;
        assert_eq!(
            avaudio_buffer_bytes(format),
            format.buffer_len() * BYTES_PER_PCM_FLOAT_SAMPLE
        );
    }
}
