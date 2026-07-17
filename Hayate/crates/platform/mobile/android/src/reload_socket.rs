//! Torimi Android ホストの reload WS クライアント（#533, #742）。
//!
//! `torimi_reload::subscribe_reload` の `connect` シームの device 既定実装。当初は
//! 「依存追加なし」の素の TCP 上に RFC6455 ハンドシェイクを手書きしていたが、公開 Demo
//! Endpoint（wss・ADR-0003）を機に **OS プラットフォームのネットワークスタックへ委譲**した
//! （ADR-0002 後半・#742）。Kotlin 側（`ReloadSocketBridge`・OkHttp の WebSocket）が WS(S) と
//! TLS を担い、Rust は URL（純粋シーム `torimi_reload::reload_ws_url`。https 由来は wss）を
//! 渡して受信イベント（テキスト / 切断）だけを受け取る。Rust に TLS 依存は入れない。reload の
//! 意味づけ（`reload` で full reload・切断時の backoff 再接続 orchestration）は
//! `torimi_reload` の純粋シームに残る — ホストは WS を中継するだけで HMR を解さない
//! （CONTEXT.md「Reload」/ ADR-0001 不変）。
//!
//! 単一スレッド契約（ADR-0003）を守るため、Kotlin のイベント待ち（blocking）は背景スレッドが
//! 行い、受信を mpsc で main へ渡す。main の poll ループが毎フレーム [`ReloadWsSocket::pump`] を
//! 呼んで、登録済みの `on_message` / `on_close` を **main スレッドで** 発火する（boot_runtime
//! 再実行＝ Hermes/tree に触るため main で動かす必要がある）。実 WS 配線は実機で検証する
//! （ローカル検証の領分・ADR-0001）。
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::cell::{Cell, RefCell};
use std::sync::mpsc::Receiver;

use crate::torimi_reload::ReloadSocket;

/// 背景スレッド → main へ渡す reload 受信イベント。
enum WsEvent {
    /// サーバが送ったテキストフレーム本文（`reload` か否かは購読側が判定する）。
    Message(String),
    /// 接続が閉じた（サーバ close / 接続・TLS 失敗 / I/O エラー）。main は backoff 再接続する。
    Closed,
}

/// reload WS への接続ハンドル。`subscribe_reload` の `connect` が返す [`ReloadSocket`]。
pub struct ReloadWsSocket {
    events: Receiver<WsEvent>,
    /// Kotlin 側 WS のハンドル（close の宛先。open が JNI 段階で失敗したら `None`）。
    handle: Option<i64>,
    on_message: RefCell<Option<Box<dyn FnMut(&str)>>>,
    on_close: RefCell<Option<Box<dyn FnMut()>>>,
    closed: Cell<bool>,
}

impl ReloadWsSocket {
    /// 背景スレッドが mpsc に積んだ受信を main で排出し、登録済みコールバックを発火する。
    /// poll ループが毎フレーム呼ぶ（blocking なイベント待ちを main から外すための drain シーム）。
    pub fn pump(&self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                WsEvent::Message(text) => {
                    if let Some(cb) = self.on_message.borrow_mut().as_mut() {
                        cb(&text);
                    }
                }
                WsEvent::Closed => {
                    if self.closed.replace(true) {
                        continue; // 二重 close は 1 度だけ通知する。
                    }
                    if let Some(cb) = self.on_close.borrow_mut().as_mut() {
                        cb();
                    }
                }
            }
        }
    }
}

impl ReloadSocket for ReloadWsSocket {
    fn on_message(&self, cb: Box<dyn FnMut(&str)>) {
        *self.on_message.borrow_mut() = Some(cb);
    }
    fn on_close(&self, cb: Box<dyn FnMut()>) {
        *self.on_close.borrow_mut() = Some(cb);
    }
    fn close(&self) {
        // Kotlin 側 WS を閉じる。閉じたことは Kotlin のイベントとして返り、背景スレッドが解けて
        // 次の pump で Closed が（再接続抑止下で）流れる。
        #[cfg(target_os = "android")]
        if let Some(handle) = self.handle {
            platform::close_ws(handle);
        }
    }
}

#[cfg(target_os = "android")]
pub use platform::connect_reload_ws;

/// OS スタック委譲の JNI glue（device 専用）。bundle_source と同じ leaf パターン
/// （`jni::` の直接使用は `jni_bridge` に封じ込め、leaf はそれだけを使う・ADR-0125）。
#[cfg(target_os = "android")]
mod platform {
    use std::sync::mpsc::{channel, Sender};
    use std::thread;

    use super::*;
    use crate::jni_bridge::JString;

    /// Kotlin の橋渡しクラス（android-app の `ReloadSocketBridge`）の JNI 名。
    const BRIDGE_CLASS: &str = "com/hayateprojects/hayate/adapter_android_demo/ReloadSocketBridge";
    /// `String url` を受けて WS を開き、ハンドル `long` を返す（接続は非同期・失敗はイベントで返る）。
    const OPEN_METHOD: &str = "open";
    const OPEN_SIG: &str = "(Ljava/lang/String;)J";
    /// ハンドルの次イベントまで**呼び出しスレッドをブロック**して `String` を返す。
    const AWAIT_METHOD: &str = "awaitEvent";
    const AWAIT_SIG: &str = "(J)Ljava/lang/String;";
    /// ハンドルの WS を閉じる（イベント待ちは Closed イベントで解ける）。
    const CLOSE_METHOD: &str = "close";
    const CLOSE_SIG: &str = "(J)V";

    /// Kotlin → Rust のイベント符号（wire 契約：`ReloadSocketBridge` と値で一致させる）。
    /// テキストフレームは本文を prefix の後ろに乗せる。タブは WS テキストに現れ得るが
    /// prefix 位置（先頭）の判定にしか使わないので衝突しない。
    const EVENT_TEXT_PREFIX: &str = "text\t";

    /// reload WS を OS スタック（Kotlin・ADR-0002）で開き、イベント待ちの背景スレッドを起こす。
    /// open が JNI 段階で失敗しても「即 Closed を積んだ」socket を返す — 購読側はそれを backoff
    /// 再接続のトリガにする（Web の `WebSocket` が onclose を出すのと同型。接続・TLS の失敗は
    /// Kotlin 側から Closed イベントとして返る）。
    pub fn connect_reload_ws(ws_url: &str) -> std::rc::Rc<ReloadWsSocket> {
        let (tx, rx) = channel();

        let handle = match open_platform_ws(ws_url) {
            Ok(handle) => {
                thread::spawn(move || await_events(handle, &tx));
                Some(handle)
            }
            Err(err) => {
                log::warn!(
                    "hayate-adapter-android: reload WS の open に失敗（backoff 再試行）: {err}"
                );
                let _ = tx.send(WsEvent::Closed);
                None
            }
        };
        std::rc::Rc::new(ReloadWsSocket {
            events: rx,
            handle,
            on_message: RefCell::new(None),
            on_close: RefCell::new(None),
            closed: Cell::new(false),
        })
    }

    /// Kotlin 側で WS を開き、イベント取り出し用のハンドルを得る。
    fn open_platform_ws(url: &str) -> Result<i64, String> {
        crate::jni_bridge::with_activity_env(|env, activity| {
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            let jurl = match env.new_string(url) {
                Ok(s) => s,
                Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
            };
            match env
                .call_static_method(&class, OPEN_METHOD, OPEN_SIG, &[(&jurl).into()])
                .and_then(|value| value.j())
            {
                Ok(handle) => Ok(handle),
                Err(e) => Err(crate::jni_bridge::describe_java_error(env, e)),
            }
        })
    }

    /// Kotlin 側のイベントを取り出し続け、テキストは [`WsEvent::Message`]、切断（および JNI
    /// エラー・未知イベント）は [`WsEvent::Closed`] で main へ渡して抜ける。取り出しは
    /// ブロッキングなので専用の背景スレッドで回す（attach はスレッドの寿命で 1 回）。
    fn await_events(handle: i64, tx: &Sender<WsEvent>) {
        let result = crate::jni_bridge::with_activity_env(|env, activity| {
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            loop {
                let obj = match env
                    .call_static_method(&class, AWAIT_METHOD, AWAIT_SIG, &[handle.into()])
                    .and_then(|value| value.l())
                {
                    Ok(obj) => obj,
                    Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
                };
                let jevent = JString::from(obj);
                let event: String = match env.get_string(&jevent) {
                    Ok(s) => s.into(),
                    Err(e) => return Err(crate::jni_bridge::describe_java_error(env, e)),
                };
                // このスレッドは attach したままループするので、イベントごとの local 参照は
                // 明示的に返す（返さないと JNI local reference table が接続の寿命で溢れる）。
                let _ = env.delete_local_ref(jevent);
                match event.strip_prefix(EVENT_TEXT_PREFIX) {
                    Some(text) => {
                        let _ = tx.send(WsEvent::Message(text.to_owned()));
                    }
                    // "closed"（と未知イベント）は切断として扱い、スレッドを解く。
                    None => return Ok(()),
                }
            }
        });
        if let Err(err) = result {
            log::warn!("hayate-adapter-android: reload WS のイベント待ちが失敗: {err}");
        }
        let _ = tx.send(WsEvent::Closed);
    }

    /// Kotlin 側 WS を閉じる。イベント待ちスレッドには Kotlin から Closed イベントが流れて解ける。
    pub(super) fn close_ws(handle: i64) {
        let result = crate::jni_bridge::with_activity_env(|env, activity| {
            let class = crate::jni_bridge::app_class(env, activity, BRIDGE_CLASS)?;
            match env.call_static_method(&class, CLOSE_METHOD, CLOSE_SIG, &[handle.into()]) {
                Ok(_) => Ok(()),
                Err(e) => Err(crate::jni_bridge::describe_java_error(env, e)),
            }
        });
        if let Err(err) = result {
            log::warn!("hayate-adapter-android: reload WS の close に失敗: {err}");
        }
    }
}
