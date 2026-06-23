//! Mobile Family Adapter（ADR-0117）。
//!
//! family（android + ios）で統一できる platform-bound capability（音声出力）を、ビルド時
//! `cfg(target_os)` で片方の leaf 実装をリンクして単一 facade として上位へ露出する。これは
//! ランタイム dispatch ではない（Flutter channel / RN bridge の機構は借りない）— cargo が
//! ターゲットごとに正確に片方の leaf をリンクする。capability 契約の正本は常に Core
//! （[`hayate_core::AudioOutput`]）であり、本 crate はそれを再露出するだけで別契約を切らない。
//! `web` は family of 1 のため Family Adapter を持たず leaf を直接置く。
//!
//! 今 facade に載る capability は音声出力のみ（android = `AudioTrack` / ios = `AVAudioEngine`）。
//! 他 capability は 2 実装が揃ってから足す（空 facade を先置きしない）。

// 契約・形式・named constant は Core が正本。上位はこの再露出を通じて使う。
pub use hayate_core::{
    AudioFormat, AudioOutput, DEFAULT_BUFFER_FRAMES, DEFAULT_CHANNEL_COUNT, DEFAULT_SAMPLE_RATE_HZ,
};

/// family 統一の音声出力 facade。ビルド対象に応じて、Core の [`AudioOutput`] を満たす leaf
/// 実装（android = `AudioTrack` / ios = `AVAudioEngine`）へ解決する単一の型名。上位は leaf を
/// 名指しせず本 facade だけを参照する。
#[cfg(target_os = "android")]
pub type MobileAudioOutput = hayate_adapter_android::audio_output::AudioTrackOutput;

/// family 統一の音声出力 facade。ビルド対象に応じて、Core の [`AudioOutput`] を満たす leaf
/// 実装（android = `AudioTrack` / ios = `AVAudioEngine`）へ解決する単一の型名。上位は leaf を
/// 名指しせず本 facade だけを参照する。
#[cfg(target_os = "ios")]
pub type MobileAudioOutput = hayate_adapter_ios::audio_output::AvAudioEngineOutput;
