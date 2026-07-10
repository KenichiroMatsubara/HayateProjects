//! Torimi Android ホストの Device Log 送信シーム（#787-789・ADR-0005）。**純 Rust シーム**。
//!
//! 端末で起きたことを USB/adb 無しに開発機の Dev Server へ届ける送り側（CONTEXT.md
//! 「Device Log」）。JS の `console.*`（`__hayateLog`）と host 系イベントをエントリ化して
//! バッファに積み、端末ごと単調増加の `seq` を採番し、定期間隔でバッチにまとめて注入ポートで
//! `POST <logRoutePrefix><deviceId>` する。受け側（#785）は `(deviceId, seq)` で再送重複を捨てる
//! （at-least-once）ので、送り側は再送を恐れない。
//!
//! ここは transport 非依存の純粋シーム — 時刻は呼び出し側が渡し（注入 clock）、送信は
//! [`LogSendPort`] に委譲する（device では Kotlin/OkHttp、テストではモック）。OkHttp ポートと
//! JSI/`__hayateLog` 配線は既存 fetch / reload ポートと同じ薄い I/O 委譲で、実駆動は実機で検証する。
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::path::Path;

use crate::dev_server_target::DevServerTarget;

/// 定期フラッシュ間隔（ms）。この間隔ごとにバッファをまとめて 1 バッチにして送る。**プレースホルダ値**
/// 2 秒（実値調整は運用を見て・ADR-0005）。マジックナンバー禁止のため名前付き定数に抽出。
pub const FLUSH_INTERVAL_MS: f64 = 2_000.0;

/// dev-server が Device Log を受ける HTTP ルート接頭辞。`@torimi/dev-server-contract` の
/// `logRoutePrefix` と一致させる wire 契約（node 依存を持ち込まないため値で複製する）。
pub const LOG_ROUTE_PREFIX: &str = "/log/";

/// Device Log 1 エントリのログレベル（`console.*` の別名・wire 契約 `LogLevel` のミラー）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    /// wire に載せる文字列（TS `LogLevel` と値で一致させる）。
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Log => "log",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Debug => "debug",
        }
    }

    /// JS 側（`__hayateLog`）から来た level 文字列を写す。未知の別名は `log` に丸める
    /// （additive-only 互換：新レベルが来ても落とさない・ADR-0005）。
    pub fn from_wire(s: &str) -> LogLevel {
        match s {
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "debug" => LogLevel::Debug,
            _ => LogLevel::Log,
        }
    }
}

/// Device Log 1 エントリのログ源（wire 契約 `LogSource` のミラー）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Js,
    Host,
}

impl LogSource {
    /// wire に載せる文字列（TS `LogSource` と値で一致させる）。
    pub fn as_str(self) -> &'static str {
        match self {
            LogSource::Js => "js",
            LogSource::Host => "host",
        }
    }
}

/// Device Log の 1 エントリ（wire 契約 `LogEntry` のミラー）。
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// 端末ごと単調増加の連番。受け側が `(deviceId, seq)` で再送重複を捨てる。
    pub seq: u64,
    /// 端末側で記録した時刻（epoch ms）。
    pub ts_ms: f64,
    pub source: LogSource,
    pub level: LogLevel,
    pub message: String,
}

/// `POST <logRoutePrefix><deviceId>` で送るバッチ（wire 契約 `LogBatch` のミラー）。
#[derive(Debug, Clone, PartialEq)]
pub struct LogBatch {
    /// 表示用の端末ラベル（端末モデル名等）。
    pub device_label: String,
    pub entries: Vec<LogEntry>,
}

/// バッチを Dev Server へ送る注入ポート。device では Kotlin/OkHttp の `POST /log/<deviceId>`、
/// テストではモック。fetch / reload ポートと同じ席（薄い I/O 委譲）。
pub trait LogSendPort {
    /// `device_id` のバッチを送る。
    fn send(&self, device_id: &str, batch: &LogBatch);
}

/// bundle 取得元の種別。Device Log を送るのは Dev Server 経由のときだけで、Demo Endpoint 経由は
/// 送らない（CONTEXT.md「Device Log」・ADR-0005）。boot 経路（`demo_manifest::BootPlan`）から
/// device 側で写す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleOrigin {
    DevServer,
    DemoEndpoint,
}

/// Device Log の送り側シーム。JS/host ログを積み、seq を採番し、定期間隔でバッチ化して送る。
pub struct DeviceLog<P: LogSendPort> {
    device_id: String,
    device_label: String,
    port: P,
    /// 送信が有効か。bundle 取得元が Demo Endpoint のときは false で、record 自体を捨てる
    /// （ログがどこへも出ていかない・ADR-0005）。
    enabled: bool,
    next_seq: u64,
    buffer: Vec<LogEntry>,
    last_flush_ms: f64,
}

impl<P: LogSendPort> DeviceLog<P> {
    /// Dev Server 経由の送り側シームを作る（送信有効）。`start_ms` は最初のフラッシュ間隔の起点。
    pub fn new(device_id: String, device_label: String, port: P, start_ms: f64) -> Self {
        Self::with_origin(device_id, device_label, port, start_ms, BundleOrigin::DevServer)
    }

    /// bundle 取得元を指定して作る。Demo Endpoint 経由なら送信を無効化する（ADR-0005）。
    pub fn with_origin(
        device_id: String,
        device_label: String,
        port: P,
        start_ms: f64,
        origin: BundleOrigin,
    ) -> Self {
        DeviceLog {
            device_id,
            device_label,
            port,
            enabled: matches!(origin, BundleOrigin::DevServer),
            next_seq: 1,
            buffer: Vec::new(),
            last_flush_ms: start_ms,
        }
    }

    /// 1 エントリを積む。seq を採番し、エントリの記録時刻 `ts_ms`（端末側 wall-clock epoch ms）を
    /// 焼き、バッファに載せる。送信無効（Demo Endpoint 経由）なら何もしない。フラッシュ間隔は
    /// [`tick`](Self::tick) が別途 monotonic clock で計るので、この `ts_ms` は間隔判定に使わない。
    pub fn record(&mut self, source: LogSource, level: LogLevel, message: String, ts_ms: f64) {
        if !self.enabled {
            return;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.buffer.push(LogEntry { seq, ts_ms, source, level, message });
    }

    /// clock tick。前回フラッシュから [`FLUSH_INTERVAL_MS`] 以上経っていれば、バッファが空でない限り
    /// まとめて 1 バッチにして送る（空なら送らない）。
    pub fn tick(&mut self, now_ms: f64) {
        if now_ms - self.last_flush_ms < FLUSH_INTERVAL_MS {
            return;
        }
        self.last_flush_ms = now_ms;
        self.flush();
    }

    /// バッファが空でなければ 1 バッチにまとめて送り、バッファを空にする。
    fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let batch = LogBatch {
            device_label: self.device_label.clone(),
            entries: std::mem::take(&mut self.buffer),
        };
        self.port.send(&self.device_id, &batch);
    }
}

/// ホストが Device ID を読み戻すファイル名（app internal data dir 直下）。Kotlin↔Rust の wire で
/// なく Rust 単独所有（ホスト発行・インストール単位・ADR-0005 / CONTEXT.md「Device ID」）。
pub const DEVICE_ID_FILE: &str = "torimi-device-id.txt";

/// Device ID を読み戻す。無ければ `generate` で作って永続化し、以降の全バッチで同じ ID を使う。
/// 再起動後も同じ値を返す（初回だけ生成）。data dir が無いときは生成値を返すが永続化しない
/// （ホストは殺さない）。ハードウェア由来でない不透明 ID で、`generate` は device 側が乱数源から
/// 供給する（テストは決定的な値を渡す）。
pub fn load_or_create_device_id(
    internal_data_path: Option<&Path>,
    generate: impl FnOnce() -> String,
) -> String {
    let Some(dir) = internal_data_path else {
        return generate();
    };
    let path = dir.join(DEVICE_ID_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    let id = generate();
    let _ = std::fs::write(&path, &id);
    id
}

/// Device Log の POST 先フル URL を作る（`<scheme>://<host>:<port>/log/<deviceId>`）。target の
/// path は使わない — path は bundle/demo 選択用であってログ受け口ではなく、ログは常に Dev Server の
/// ルート `/log/` へ送る（bundle_url が path を保持するのと対称的にこちらは base のみ）。
pub fn log_url(target: &DevServerTarget, device_id: &str) -> String {
    format!(
        "{}://{}:{}{}{}",
        target.scheme().as_str(),
        target.host(),
        target.port(),
        LOG_ROUTE_PREFIX,
        device_id,
    )
}

/// バッチを wire JSON（`@torimi/dev-server-contract` の `LogBatch` 形）に符号化する。device の
/// 送信ポートはこれを body にして `POST <logRoutePrefix><deviceId>` する。手書き結合ではなく
/// `serde_json` でエスケープを正しく通す（既存 demo_manifest と同じく serde_json 依存を使う）。
pub fn to_wire_json(batch: &LogBatch) -> String {
    let entries: Vec<serde_json::Value> = batch
        .entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "seq": e.seq,
                "ts": e.ts_ms,
                "source": e.source.as_str(),
                "level": e.level.as_str(),
                "message": e.message,
            })
        })
        .collect();
    serde_json::json!({ "deviceLabel": batch.device_label, "entries": entries }).to_string()
}

#[cfg(target_os = "android")]
pub use platform::{device_label, random_device_id, KotlinLogPort};

/// OS スタック委譲の JNI glue（device 専用）。bundle_source / reload_socket と同じ leaf パターン
/// （`jni::` の直接使用は `jni_bridge` に封じ込め、leaf はそれだけを使う・ADR-0125）。実駆動は
/// 実機で検証する（Kotlin/OkHttp ポートは既存 fetch / reload と同じ薄い I/O 委譲・ADR-0005）。
#[cfg(target_os = "android")]
mod platform {
    use std::time::Duration;

    use super::*;
    use crate::jni_bridge::JString;

    /// Device Log POST のタイムアウト。応答しない配信点で永久に待たない上限（bundle fetch の
    /// FETCH_TIMEOUT と対称。名前付き定数）。
    const LOG_POST_TIMEOUT: Duration = Duration::from_secs(5);

    /// Kotlin の橋渡しクラス（android-app の `DeviceLogBridge`）の JNI 名。
    const BRIDGE_CLASS: &str = "com/hayateprojects/hayate/adapter_android_demo/DeviceLogBridge";
    /// `String url, String jsonBody, int timeoutMs` を受けて POST し、成功可否 `boolean` を返す。
    const POST_METHOD: &str = "postBlocking";
    const POST_SIG: &str = "(Ljava/lang/String;Ljava/lang/String;I)Z";
    /// `String` の不透明ランダム Device ID を作る（`UUID.randomUUID()`・インストール単位）。
    const RANDOM_ID_METHOD: &str = "randomId";
    const RANDOM_ID_SIG: &str = "()Ljava/lang/String;";
    /// 表示用の端末ラベル（`Build.MODEL`）を返す。
    const DEVICE_LABEL_METHOD: &str = "deviceLabel";
    const DEVICE_LABEL_SIG: &str = "()Ljava/lang/String;";

    /// Kotlin の static メソッドを呼んで `String` を得る共通ヘルパ（randomId / deviceLabel 用）。
    fn call_static_string(method: &str, sig: &str) -> Result<String, String> {
        crate::jni_bridge::with_activity_env(|env, activity| {
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            let obj = env
                .call_static_method(&class, method, sig, &[])
                .and_then(|value| value.l())
                .map_err(|e| crate::jni_bridge::describe_java_error(env, e))?;
            env.get_string(&JString::from(obj))
                .map(|s| s.into())
                .map_err(|e| crate::jni_bridge::describe_java_error(env, e))
        })
    }

    /// [`LogSendPort`] の device 実装。組み上がったバッチを wire JSON にして Kotlin/OkHttp の
    /// `POST <logRoutePrefix><deviceId>` へ委譲する（fetch / reload と同じ席）。送信先 URL の base
    /// （scheme/host/port）は解決済み target が持ち、bundle fetch / reload と同じ target を共有する。
    pub struct KotlinLogPort {
        target: DevServerTarget,
    }

    impl KotlinLogPort {
        pub fn new(target: DevServerTarget) -> Self {
            KotlinLogPort { target }
        }
    }

    impl LogSendPort for KotlinLogPort {
        fn send(&self, device_id: &str, batch: &LogBatch) {
            let url = log_url(&self.target, device_id);
            let body = to_wire_json(batch);
            // fire-and-forget（#787）。成否は #788 のリングバッファ再送が使う。失敗は握って
            // ホストを止めない（LAN dev の空振りは安い・dev-server 復帰で自然回復・ADR-0005）。
            let _ = post_blocking(&url, &body, LOG_POST_TIMEOUT);
        }
    }

    /// Kotlin の `DeviceLogBridge.postBlocking` を共通 JNI 下地（`jni_bridge`・ADR-0125）経由で呼ぶ。
    /// 呼び出しスレッドをブロックする（送信は非 UI スレッドで駆動する契約）。成功可否を返す。
    fn post_blocking(url: &str, body: &str, timeout: Duration) -> bool {
        let result = crate::jni_bridge::with_activity_env(|env, activity| {
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            let jurl = env
                .new_string(url)
                .map_err(|e| crate::jni_bridge::describe_java_error(env, e))?;
            let jbody = env
                .new_string(body)
                .map_err(|e| crate::jni_bridge::describe_java_error(env, e))?;
            let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
            env.call_static_method(
                &class,
                POST_METHOD,
                POST_SIG,
                &[(&jurl).into(), (&jbody).into(), timeout_ms.into()],
            )
            .and_then(|value| value.z())
            .map_err(|e| crate::jni_bridge::describe_java_error(env, e))
        });
        match result {
            Ok(ok) => ok,
            Err(err) => {
                log::debug!("hayate-adapter-android: Device Log POST 失敗（次間隔で再送）: {err}");
                false
            }
        }
    }

    /// 初回起動時の Device ID 生成源（`load_or_create_device_id` の `generate`）。ハードウェア由来で
    /// ない不透明 ID を OS の `UUID.randomUUID()` で作る（インストール単位・ADR-0005）。JNI 失敗時は
    /// プロセス情報由来のフォールバックで空 ID を避ける。
    pub fn random_device_id() -> String {
        call_static_string(RANDOM_ID_METHOD, RANDOM_ID_SIG)
            .unwrap_or_else(|_| format!("torimi-{}", std::process::id()))
    }

    /// 表示用の端末ラベル（端末モデル名・`Build.MODEL`）。各バッチのペイロードに載る Device Label。
    /// JNI 失敗時は素の `"Android"` に落ちる（表示用途なのでホストは止めない）。
    pub fn device_label() -> String {
        call_static_string(DEVICE_LABEL_METHOD, DEVICE_LABEL_SIG)
            .unwrap_or_else(|_| "Android".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// 送信を記録するモックポート（注入 clock と対で、外部挙動＝「何がいつ送られたか」を検証する）。
    #[derive(Default)]
    struct MockPort {
        sent: RefCell<Vec<(String, LogBatch)>>,
    }

    impl LogSendPort for &MockPort {
        fn send(&self, device_id: &str, batch: &LogBatch) {
            self.sent.borrow_mut().push((device_id.to_owned(), batch.clone()));
        }
    }

    #[test]
    fn a_recorded_js_log_reaches_the_port_as_a_batch_after_the_flush_interval() {
        let port = MockPort::default();
        let mut log = DeviceLog::new("dev-abc".to_owned(), "Pixel 8".to_owned(), &port, 0.0);

        log.record(LogSource::Js, LogLevel::Log, "hello".to_owned(), 10.0);
        log.tick(FLUSH_INTERVAL_MS);

        let sent = port.sent.borrow();
        assert_eq!(sent.len(), 1, "one batch should be flushed");
        let (device_id, batch) = &sent[0];
        assert_eq!(device_id, "dev-abc");
        assert_eq!(batch.device_label, "Pixel 8");
        assert_eq!(
            batch.entries,
            vec![LogEntry {
                seq: 1,
                ts_ms: 10.0,
                source: LogSource::Js,
                level: LogLevel::Log,
                message: "hello".to_owned(),
            }],
        );
    }

    #[test]
    fn an_empty_buffer_sends_nothing_even_when_the_interval_elapses() {
        let port = MockPort::default();
        let mut log = DeviceLog::new("dev-abc".to_owned(), "Pixel".to_owned(), &port, 0.0);

        log.tick(FLUSH_INTERVAL_MS);
        log.tick(FLUSH_INTERVAL_MS * 5.0);

        assert!(port.sent.borrow().is_empty(), "no batch should be sent while the buffer is empty");
    }

    #[test]
    fn entries_accumulate_until_the_interval_and_then_flush_as_one_batch() {
        let port = MockPort::default();
        let mut log = DeviceLog::new("dev-abc".to_owned(), "Pixel".to_owned(), &port, 0.0);

        log.record(LogSource::Js, LogLevel::Log, "one".to_owned(), 100.0);
        // 間隔前の tick では送らない（バッファに溜め続ける）。
        log.tick(FLUSH_INTERVAL_MS - 1.0);
        assert!(port.sent.borrow().is_empty(), "nothing flushes before the interval elapses");

        log.record(LogSource::Js, LogLevel::Warn, "two".to_owned(), 200.0);
        log.tick(FLUSH_INTERVAL_MS);

        let sent = port.sent.borrow();
        assert_eq!(sent.len(), 1, "both entries flush together in one batch");
        let seqs: Vec<u64> = sent[0].1.entries.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1, 2]);
    }

    #[test]
    fn seq_keeps_increasing_monotonically_across_separate_flushes() {
        let port = MockPort::default();
        let mut log = DeviceLog::new("dev-abc".to_owned(), "Pixel".to_owned(), &port, 0.0);

        log.record(LogSource::Js, LogLevel::Log, "a".to_owned(), 10.0);
        log.tick(FLUSH_INTERVAL_MS);
        log.record(LogSource::Js, LogLevel::Log, "b".to_owned(), 20.0);
        log.tick(FLUSH_INTERVAL_MS * 2.0);

        let sent = port.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].1.entries[0].seq, 1);
        assert_eq!(sent[1].1.entries[0].seq, 2, "seq does not reset between batches");
    }

    #[test]
    fn a_demo_endpoint_boot_never_sends_device_logs() {
        // 送るのは Dev Server 経由のときだけ。Demo Endpoint 経由はログ送信自体をしない
        // （CONTEXT.md「Device Log」・ADR-0005）。
        let port = MockPort::default();
        let mut log = DeviceLog::with_origin(
            "dev-abc".to_owned(),
            "Pixel".to_owned(),
            &port,
            0.0,
            BundleOrigin::DemoEndpoint,
        );

        log.record(LogSource::Js, LogLevel::Error, "should not leave the device".to_owned(), 10.0);
        log.tick(FLUSH_INTERVAL_MS * 3.0);

        assert!(port.sent.borrow().is_empty(), "Demo Endpoint boots must not POST logs anywhere");
    }

    /// テスト専用の使い捨て data dir（Kotlin が書く internal data dir の代役・dev_server_target と同流儀）。
    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("torimi-devlog-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn device_id_is_generated_once_and_reused_across_restarts() {
        let dir = temp_data_dir("device-id");
        let _ = std::fs::remove_file(dir.join(DEVICE_ID_FILE));

        let first = load_or_create_device_id(Some(&dir), || "generated-once".to_owned());
        assert_eq!(first, "generated-once");

        // 2 回目（再起動相当）は生成器を呼ばず、永続化した値を読み戻す。
        let second =
            load_or_create_device_id(Some(&dir), || panic!("must not regenerate a persisted id"));
        assert_eq!(second, "generated-once");
    }

    #[test]
    fn device_id_without_a_data_dir_is_generated_but_not_persisted() {
        // data dir が無ければ生成値をそのまま返す（永続化はできないがホストは殺さない）。
        assert_eq!(load_or_create_device_id(None, || "ephemeral".to_owned()), "ephemeral");
    }

    #[test]
    fn log_url_targets_the_log_route_on_the_dev_server_base_ignoring_the_bundle_path() {
        // ログは常に Dev Server ルート `/log/<deviceId>` へ。bundle 選択の path（/solid/bundle.js）は
        // 使わない（bundle_url が path を保持するのと対をなす）。scheme 既定ポートは URL 正規化済み。
        let target = crate::dev_server_target::resolve(Some("192.168.1.5:8080/solid/bundle.js"));
        assert_eq!(log_url(&target, "dev-abc"), "http://192.168.1.5:8080/log/dev-abc");

        let https = crate::dev_server_target::resolve(Some("https://demo.example/react/bundle.js"));
        assert_eq!(log_url(&https, "xyz"), "https://demo.example:443/log/xyz");
    }

    #[test]
    fn wire_level_strings_round_trip_and_unknown_aliases_fall_back_to_log() {
        for level in [LogLevel::Log, LogLevel::Info, LogLevel::Warn, LogLevel::Error, LogLevel::Debug] {
            assert_eq!(LogLevel::from_wire(level.as_str()), level);
        }
        // 未知の別名（将来レベル）は落とさず log に丸める（additive-only 互換）。
        assert_eq!(LogLevel::from_wire("trace"), LogLevel::Log);
        assert_eq!(LogLevel::from_wire(""), LogLevel::Log);
    }

    #[test]
    fn the_log_route_prefix_matches_the_dev_server_contract() {
        // `@torimi/dev-server-contract` の logRoutePrefix と一致する wire 契約。
        assert_eq!(LOG_ROUTE_PREFIX, "/log/");
    }

    #[test]
    fn a_batch_encodes_to_the_wire_contract_shape() {
        let batch = LogBatch {
            device_label: "Pixel 8".to_owned(),
            entries: vec![
                LogEntry { seq: 1, ts_ms: 1_720_000_000_000.0, source: LogSource::Js, level: LogLevel::Warn, message: "hi \"quoted\"".to_owned() },
                LogEntry { seq: 2, ts_ms: 1_720_000_000_001.0, source: LogSource::Host, level: LogLevel::Error, message: "boom".to_owned() },
            ],
        };

        let json: serde_json::Value = serde_json::from_str(&to_wire_json(&batch)).unwrap();

        assert_eq!(json["deviceLabel"], "Pixel 8");
        let entries = json["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["seq"], 1);
        assert_eq!(entries[0]["ts"].as_f64().unwrap(), 1_720_000_000_000.0);
        assert_eq!(entries[0]["source"], "js");
        assert_eq!(entries[0]["level"], "warn");
        assert_eq!(entries[0]["message"], "hi \"quoted\"");
        assert_eq!(entries[1]["source"], "host");
        assert_eq!(entries[1]["level"], "error");
    }
}
