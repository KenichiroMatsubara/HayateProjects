//! Android leaf の audio glue（ADR-0117 / #562）。
//!
//! platform-free な音声出力契約 [`AudioOutput`] の正本は `hayate_core` が持つ。本モジュールは
//! その型を re-export し、Android 固有の glue — f32 PCM（`ENCODING_PCM_FLOAT` 相当）を NDK の
//! **AAudio**（`libaaudio.so`）へ書く native 呼び出し — だけを `#[cfg(target_os = "android")]`
//! で薄く置く。Kotlin は介さず純 native（全ロジックを Rust に寄せる本アプリの方針, `ime_bridge`
//! と同型）。バッファ長やチャンネルマスクの算術は host-testable な純粋部分に寄せ、汚い FFI glue を
//! 薄く保つ（`surface_lifecycle` / `ime_bridge` と同パターン）。
//!
//! 公開型名 [`AudioTrackOutput`] は「AudioTrack 級の PCM-float 出力」という leaf の役割を指す
//! （アーキ文書の `android = AudioTrack 相当`）。具体バックエンドは AAudio。AAudio は API 26+ なので
//! `libaaudio.so` を実行時 `dlopen` でロードし、非対応端末（API<26）では発音せず no-op に落とす
//! （`minSdk=24` でもクラッシュさせない）。実機での発音検証は NDK + 端末が要るため AFK 範囲外。
//! 純粋部分はホストで単体テストし、native FFI はソース走査ガードで本モジュールに封じ込める。

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
    use std::ffi::c_void;
    use std::os::raw::{c_char, c_int};
    use std::ptr;

    /// AAudio の不透明ハンドル型（`libaaudio.so` 内で定義）。
    #[allow(non_camel_case_types)]
    enum AAudioStreamBuilder {}
    #[allow(non_camel_case_types)]
    enum AAudioStream {}

    /// `AAUDIO_FORMAT_PCM_FLOAT`。`ENCODING_PCM_FLOAT` 相当の f32 PCM 出力。
    const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
    /// `AAUDIO_OK`。AAudio 呼び出しの成功コード。
    const AAUDIO_OK: i32 = 0;
    /// `AAudioStream_write` のブロック上限（ns）。1 バッファ供給は短時間で返す。
    const WRITE_TIMEOUT_NANOS: i64 = 1_000_000_000;
    /// `RTLD_NOW`（`dlfcn.h`）。
    const RTLD_NOW: c_int = 2;

    // libc/libdl の動的ロード。AAudio は API 26+ なので、リンク時に `-laaudio` を要求せず
    // 実行時 `dlopen` で解決する（`minSdk=24` のリンカに libaaudio.so が無くてもビルドが通る）。
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    type CreateStreamBuilderFn = unsafe extern "C" fn(*mut *mut AAudioStreamBuilder) -> i32;
    type BuilderSetI32Fn = unsafe extern "C" fn(*mut AAudioStreamBuilder, i32);
    type OpenStreamFn =
        unsafe extern "C" fn(*mut AAudioStreamBuilder, *mut *mut AAudioStream) -> i32;
    type BuilderDeleteFn = unsafe extern "C" fn(*mut AAudioStreamBuilder) -> i32;
    type StreamActionFn = unsafe extern "C" fn(*mut AAudioStream) -> i32;
    type StreamWriteFn = unsafe extern "C" fn(*mut AAudioStream, *const c_void, i32, i64) -> i32;

    /// `libaaudio.so` から `dlsym` で解決した必要関数群。`ime_bridge` 同様、この native
    /// 境界は本モジュールに封じ込める。
    struct AAudioApi {
        create_builder: CreateStreamBuilderFn,
        set_sample_rate: BuilderSetI32Fn,
        set_channel_count: BuilderSetI32Fn,
        set_format: BuilderSetI32Fn,
        open_stream: OpenStreamFn,
        delete_builder: BuilderDeleteFn,
        request_start: StreamActionFn,
        write: StreamWriteFn,
        request_stop: StreamActionFn,
        close: StreamActionFn,
    }

    impl AAudioApi {
        /// `libaaudio.so` を `dlopen` し、要る関数を `dlsym` で引く。API<26 等で不在なら `None`。
        unsafe fn load() -> Option<Self> {
            let handle = dlopen(b"libaaudio.so\0".as_ptr() as *const c_char, RTLD_NOW);
            if handle.is_null() {
                log::error!(
                    "hayate-adapter-android: libaaudio.so を dlopen できません（API<26?）— テストトーンを無音化"
                );
                return None;
            }
            macro_rules! sym {
                ($name:literal, $ty:ty) => {{
                    let p = dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char);
                    if p.is_null() {
                        log::error!(concat!(
                            "hayate-adapter-android: AAudio シンボル不在 ",
                            $name
                        ));
                        return None;
                    }
                    std::mem::transmute::<*mut c_void, $ty>(p)
                }};
            }
            Some(AAudioApi {
                create_builder: sym!("AAudio_createStreamBuilder", CreateStreamBuilderFn),
                set_sample_rate: sym!("AAudioStreamBuilder_setSampleRate", BuilderSetI32Fn),
                set_channel_count: sym!("AAudioStreamBuilder_setChannelCount", BuilderSetI32Fn),
                set_format: sym!("AAudioStreamBuilder_setFormat", BuilderSetI32Fn),
                open_stream: sym!("AAudioStreamBuilder_openStream", OpenStreamFn),
                delete_builder: sym!("AAudioStreamBuilder_delete", BuilderDeleteFn),
                request_start: sym!("AAudioStream_requestStart", StreamActionFn),
                write: sym!("AAudioStream_write", StreamWriteFn),
                request_stop: sym!("AAudioStream_requestStop", StreamActionFn),
                close: sym!("AAudioStream_close", StreamActionFn),
            })
        }
    }

    /// Android の [`AudioOutput`] 実装。NDK AAudio で f32 PCM を発音する薄い glue。
    /// `open` で AAudio ストリームを起こし、`submit` で 1 バッファを `AAudioStream_write` へ流し、
    /// `close` で停止・解放する。AAudio 不在端末では各操作が no-op（無音）になる。
    #[derive(Default)]
    pub struct AudioTrackOutput {
        api: Option<AAudioApi>,
        stream: *mut AAudioStream,
        channel_count: i32,
    }

    impl AudioOutput for AudioTrackOutput {
        fn open(&mut self, format: AudioFormat) {
            unsafe {
                let Some(api) = AAudioApi::load() else {
                    return;
                };
                let mut builder: *mut AAudioStreamBuilder = ptr::null_mut();
                if (api.create_builder)(&mut builder) != AAUDIO_OK || builder.is_null() {
                    log::error!("hayate-adapter-android: AAudio createStreamBuilder 失敗");
                    return;
                }
                (api.set_sample_rate)(builder, format.sample_rate_hz as i32);
                (api.set_channel_count)(builder, format.channel_count as i32);
                (api.set_format)(builder, AAUDIO_FORMAT_PCM_FLOAT);

                let mut stream: *mut AAudioStream = ptr::null_mut();
                let rc = (api.open_stream)(builder, &mut stream);
                (api.delete_builder)(builder);
                if rc != AAUDIO_OK || stream.is_null() {
                    log::error!("hayate-adapter-android: AAudio openStream 失敗（rc={rc}）");
                    return;
                }
                if (api.request_start)(stream) != AAUDIO_OK {
                    log::error!("hayate-adapter-android: AAudio requestStart 失敗");
                    (api.close)(stream);
                    return;
                }
                log::info!(
                    "hayate-adapter-android: AAudio ストリーム開始（{} Hz, {} ch, PCM_FLOAT）",
                    format.sample_rate_hz,
                    format.channel_count
                );
                self.api = Some(api);
                self.stream = stream;
                self.channel_count = format.channel_count as i32;
            }
        }

        fn submit(&mut self, frames: &[f32]) {
            let Some(api) = self.api.as_ref() else {
                return;
            };
            if self.stream.is_null() || self.channel_count == 0 {
                return;
            }
            let num_frames = frames.len() as i32 / self.channel_count;
            unsafe {
                let written = (api.write)(
                    self.stream,
                    frames.as_ptr() as *const c_void,
                    num_frames,
                    WRITE_TIMEOUT_NANOS,
                );
                if written < 0 {
                    log::error!("hayate-adapter-android: AAudio write 失敗（rc={written}）");
                }
            }
        }

        fn close(&mut self) {
            if let (Some(api), false) = (self.api.as_ref(), self.stream.is_null()) {
                unsafe {
                    (api.request_stop)(self.stream);
                    (api.close)(self.stream);
                }
                log::info!("hayate-adapter-android: AAudio ストリーム停止・解放");
            }
            self.stream = ptr::null_mut();
            self.channel_count = 0;
            self.api = None;
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
