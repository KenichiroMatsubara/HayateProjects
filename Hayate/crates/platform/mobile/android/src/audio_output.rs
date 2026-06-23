//! Android leaf の audio glue（ADR-0117）。
//!
//! platform-free な音声出力契約 [`AudioOutput`] の正本は `hayate_core` が持つ。本モジュールは
//! その型を re-export し、Android 固有の glue — f32 PCM を `AudioTrack`（`ENCODING_PCM_FLOAT`）
//! へ書く native 呼び出し（`hayate_android_audio_*`、Kotlin ホストが AudioTrack に写す）— だけを
//! `#[cfg(target_os = "android")]` で薄く置く。バッファ長やチャンネルマスクの算術は host-testable
//! な純粋部分に寄せ、汚い FFI glue を薄く保つ（`surface_lifecycle` / `ime_bridge` と同パターン）。
//!
//! 実機（NDK + AudioTrack）での発音検証はサンドボックスに NDK が無いため AFK 範囲外。
//! 純粋部分はホストで単体テストし、FFI glue はソース走査ガードで封じ込める。

#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub use hayate_core::{AudioFormat, AudioOutput};

/// f32 PCM 1 サンプルあたりのバイト数（`AudioFormat.ENCODING_PCM_FLOAT`）。
const BYTES_PER_PCM_FLOAT_SAMPLE: usize = 4;

/// `AudioTrack` のチャンネルマスク定数（`android.media.AudioFormat` より）。
pub const CHANNEL_OUT_MONO: i32 = 0x4;
pub const CHANNEL_OUT_STEREO: i32 = 0xc;

/// `format` の 1 バッファを `AudioTrack.write(float[])` へ渡すのに要するバイト数（純粋計算）。
pub fn audio_track_buffer_bytes(format: AudioFormat) -> usize {
    format.buffer_len() * BYTES_PER_PCM_FLOAT_SAMPLE
}

/// `format` のチャンネル数を `AudioTrack` のチャンネルマスクへ写す（純粋計算）。
pub fn channel_mask(format: AudioFormat) -> i32 {
    match format.channel_count {
        1 => CHANNEL_OUT_MONO,
        _ => CHANNEL_OUT_STEREO,
    }
}

#[cfg(target_os = "android")]
pub use platform::AudioTrackOutput;

#[cfg(target_os = "android")]
mod platform {
    use super::*;

    // Kotlin ホストが `AudioTrack`（`ENCODING_PCM_FLOAT` / `write(float[])`）へ写す native 境界。
    // ソフトキーボード制御 FFI（`ime_bridge`）と同じく、この音声 FFI は本モジュールに封じ込める。
    extern "C" {
        fn hayate_android_audio_open(sample_rate_hz: u32, channel_mask: i32, buffer_bytes: usize);
        fn hayate_android_audio_write(frames: *const f32, len: usize);
        fn hayate_android_audio_close();
    }

    /// Android の [`AudioOutput`] 実装。`AudioTrack` を介して f32 PCM を発音する薄い glue。
    #[derive(Default)]
    pub struct AudioTrackOutput;

    impl AudioOutput for AudioTrackOutput {
        fn open(&mut self, format: AudioFormat) {
            unsafe {
                hayate_android_audio_open(
                    format.sample_rate_hz,
                    channel_mask(format),
                    audio_track_buffer_bytes(format),
                );
            }
        }

        fn submit(&mut self, frames: &[f32]) {
            unsafe { hayate_android_audio_write(frames.as_ptr(), frames.len()) }
        }

        fn close(&mut self) {
            unsafe { hayate_android_audio_close() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_bytes_is_samples_times_pcm_float_width() {
        let format = AudioFormat::DEFAULT;
        assert_eq!(
            audio_track_buffer_bytes(format),
            format.buffer_len() * BYTES_PER_PCM_FLOAT_SAMPLE
        );
    }

    #[test]
    fn channel_mask_maps_mono_and_stereo() {
        let mono = AudioFormat {
            channel_count: 1,
            ..AudioFormat::DEFAULT
        };
        let stereo = AudioFormat {
            channel_count: 2,
            ..AudioFormat::DEFAULT
        };
        assert_eq!(channel_mask(mono), CHANNEL_OUT_MONO);
        assert_eq!(channel_mask(stereo), CHANNEL_OUT_STEREO);
    }
}
