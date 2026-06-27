//! 実機発音検証用のテストトーン生成器（ADR-0117 / #562）。
//!
//! `AudioOutput::{open,submit,close}` のライフサイクルが実機で本当に音を出すかを
//! 確かめるための最小の検証信号。`AudioFormat` に合わせたインターリーブ f32 PCM の
//! 正弦波を 1 バッファずつ生成し、位相をバッファ越しに連結して数百 ms 鳴らしても
//! クリックノイズが出ないようにする。NDK 非依存の純粋計算なのでホストで単体テスト
//! でき、AAudio glue（`audio_output.rs`）はこのバッファを書くだけの薄いグルーに保てる。

use std::f64::consts::TAU;

use hayate_core::{AudioFormat, AudioOutput};

/// 実機発音検証で鳴らす長さ（ms）。耳・録音・logcat で確認できる程度の数百 ms。
pub const TEST_TONE_DURATION_MS: u32 = 300;

/// 既定のテストトーン周波数（Hz）。440Hz = 標準ピッチ A4。
pub const DEFAULT_FREQUENCY_HZ: f32 = 440.0;

/// 既定の振幅（フルスケール比）。耳に痛くない安全な音量に絞る（≒ -14 dBFS）。
pub const DEFAULT_AMPLITUDE: f32 = 0.2;

/// テストトーンを `out` で `duration_ms` 鳴らす検証導線。`AudioOutput::{open,submit,close}`
/// のライフサイクルをそのまま駆動する（位相連続な 440Hz サインを 1 バッファずつ submit）。
/// `AudioOutput` にジェネリックなので、実機 leaf でも host の double でも同じ経路を走る。
pub fn play_test_tone<O: AudioOutput>(out: &mut O, format: AudioFormat, duration_ms: u32) {
    out.open(format);
    let mut tone = TestTone::default_tone();
    for _ in 0..buffers_for_duration(format, duration_ms) {
        out.submit(&tone.fill_buffer(format));
    }
    out.close();
}

/// `format` で `duration_ms` ミリ秒を鳴らすのに要するバッファ本数（端数切り上げ）。
/// 数百 ms のテストトーンを「何バッファ submit するか」へ写す純粋計算。
pub fn buffers_for_duration(format: AudioFormat, duration_ms: u32) -> usize {
    let frames_per_buffer = format.buffer_frames as u64;
    if frames_per_buffer == 0 {
        return 0;
    }
    let total_frames = format.sample_rate_hz as u64 * duration_ms as u64 / 1000;
    total_frames.div_ceil(frames_per_buffer) as usize
}

/// 位相を保持しながら 1 バッファずつ正弦波 PCM を吐く検証用トーン。
pub struct TestTone {
    frequency_hz: f32,
    amplitude: f32,
    /// 直近サンプルまでに積算した位相（ラジアン）。バッファ境界をまたいで
    /// 連続させ、数百 ms の連結再生でクリックが出ないようにする。
    phase: f64,
}

impl TestTone {
    /// 既定（440Hz / 安全振幅）のトーン。
    pub fn default_tone() -> Self {
        Self::new(DEFAULT_FREQUENCY_HZ, DEFAULT_AMPLITUDE)
    }

    pub fn new(frequency_hz: f32, amplitude: f32) -> Self {
        Self {
            frequency_hz,
            amplitude,
            phase: 0.0,
        }
    }

    /// `format` の 1 バッファ分（フレーム数 × チャンネル数）のインターリーブ f32 PCM を
    /// 生成し、内部位相をそのバッファ分だけ進める。
    pub fn fill_buffer(&mut self, format: AudioFormat) -> Vec<f32> {
        let channels = format.channel_count as usize;
        let phase_per_frame = TAU * self.frequency_hz as f64 / format.sample_rate_hz as f64;

        let mut buffer = Vec::with_capacity(format.buffer_len());
        for _ in 0..format.buffer_frames {
            let sample = (self.phase.sin() as f32) * self.amplitude;
            for _ in 0..channels {
                buffer.push(sample);
            }
            self.phase += phase_per_frame;
            if self.phase >= TAU {
                self.phase -= TAU;
            }
        }
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AudioOutput` 契約を満たす host 用 double。導線が公開インターフェース越しに
    /// open→submit×N→close を正しく駆動するかを実機なしで確かめる。
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
    fn play_test_tone_opens_submits_each_buffer_then_closes() {
        let format = AudioFormat::DEFAULT;
        let duration_ms = TEST_TONE_DURATION_MS;
        let mut out = RecordingOutput::default();

        play_test_tone(&mut out, format, duration_ms);

        assert_eq!(out.opened, Some(format), "must open the stream with the format");
        let expected_buffers = buffers_for_duration(format, duration_ms);
        assert_eq!(
            out.submitted.len(),
            expected_buffers,
            "must submit exactly one buffer per slice of the duration"
        );
        assert!(
            out.submitted.iter().all(|&len| len == format.buffer_len()),
            "every submitted buffer is one interleaved AudioFormat buffer"
        );
        assert!(out.closed, "must close the stream when the tone finishes");
    }

    #[test]
    fn buffer_has_one_interleaved_buffer_of_samples() {
        let mut tone = TestTone::default_tone();
        let format = AudioFormat::DEFAULT;
        let buffer = tone.fill_buffer(format);
        assert_eq!(buffer.len(), format.buffer_len());
    }

    #[test]
    fn samples_stay_within_the_amplitude_envelope() {
        let amplitude = 0.2;
        let mut tone = TestTone::new(DEFAULT_FREQUENCY_HZ, amplitude);
        let buffer = tone.fill_buffer(AudioFormat::DEFAULT);
        for sample in buffer {
            assert!(
                sample.abs() <= amplitude + 1e-6,
                "sample {sample} escaped the +/-{amplitude} envelope (would clip / be too loud)"
            );
        }
    }

    #[test]
    fn waveform_matches_a_sine_at_the_requested_frequency() {
        let frequency = DEFAULT_FREQUENCY_HZ;
        let amplitude = 0.2;
        let format = AudioFormat::DEFAULT;
        let mut tone = TestTone::new(frequency, amplitude);
        let buffer = tone.fill_buffer(format);

        let channels = format.channel_count as usize;
        // frame n のサンプルは sin(2π·f·n/sr)·amp に一致する（位相 0 始まり）。
        for frame in 0..format.buffer_frames as usize {
            let t = frame as f64 / format.sample_rate_hz as f64;
            let expected = (TAU * frequency as f64 * t).sin() as f32 * amplitude;
            assert!(
                (buffer[frame * channels] - expected).abs() < 1e-5,
                "frame {frame}: got {}, expected {expected}",
                buffer[frame * channels]
            );
        }
    }

    #[test]
    fn every_channel_in_a_frame_carries_the_same_sample() {
        let format = AudioFormat {
            channel_count: 2,
            ..AudioFormat::DEFAULT
        };
        let mut tone = TestTone::default_tone();
        let buffer = tone.fill_buffer(format);
        // モノのトーンを全チャンネルへ複製する（ステレオでも位相差を持たせない）。
        for frame in buffer.chunks_exact(format.channel_count as usize) {
            assert_eq!(frame[0], frame[1], "stereo channels must carry one mono tone");
        }
    }

    #[test]
    fn buffers_for_duration_covers_the_requested_milliseconds() {
        let format = AudioFormat::DEFAULT; // 48kHz, 1024 frames/buffer
                                           // 300ms @48kHz = 14400 frames -> ceil(14400/1024) = 15 buffers.
        assert_eq!(buffers_for_duration(format, 300), 15);
        // ちょうど割り切れる場合は端数バッファを足さない。
        // 1024 frames @48kHz ≈ 21.333ms。2 バッファ分ぴったり = 2048 frames。
        let exact_ms = (2 * format.buffer_frames as u64 * 1000 / format.sample_rate_hz as u64) as u32;
        assert_eq!(buffers_for_duration(format, exact_ms), 2);
        // 0ms は鳴らさない。
        assert_eq!(buffers_for_duration(format, 0), 0);
    }

    #[test]
    fn phase_is_continuous_across_consecutive_buffers() {
        let frequency = DEFAULT_FREQUENCY_HZ;
        let amplitude = 0.2;
        let format = AudioFormat::DEFAULT;
        let mut tone = TestTone::new(frequency, amplitude);

        let _first = tone.fill_buffer(format);
        let second = tone.fill_buffer(format);

        // 2 バッファ目の先頭は位相 0 へ戻らず、連結したサインの続き（frame =
        // buffer_frames）になる。戻ってしまうと境界でクリックが出る。
        let n = format.buffer_frames as f64;
        let t = n / format.sample_rate_hz as f64;
        let expected = (TAU * frequency as f64 * t).sin() as f32 * amplitude;
        assert!(
            (second[0] - expected).abs() < 1e-3,
            "second buffer restarted phase: got {}, expected continuation {expected}",
            second[0]
        );
    }
}
