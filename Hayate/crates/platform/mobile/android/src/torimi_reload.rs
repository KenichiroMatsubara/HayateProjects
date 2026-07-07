//! Torimi Android ホストの full reload ループ（#533）。
//!
//! Web ホスト（`@torimi/host-web` の `bootTorimiHost` / `subscribeReload` /
//! `startTorimiHost`, #529）と対称の**ホスト側 full reload**。dev-server からの WS `reload`
//! を受けるたび、バンドルを取り直して Hermes ランタイムを**作り直し**再 eval する（state は飛ぶ・
//! CONTEXT.md「Reload」/ ADR-0001）。HMR ではなく full reload で全 FW に一様に効く。
//!
//! ホストのネイティブ契約（host bootstrap）は full reload / HMR で不変なので、ここには reload の
//! **意味づけ（再 fetch → 再構築 → 再 eval → version 突き合わせ）だけ**を、device 依存
//! （Hermes / 実 WS / 実 socket）を注入シームに逃がした純 Rust orchestration として置く。実
//! ランタイム再構築（`new_hermes_app`）と実 WS 配線は device グルー（`app_tsubame`）で、ここでは
//! ホストで `cargo test` できる配線契約だけを固定する。実描画と reload 体験はローカル実機で検証
//! する（本 issue 外）。

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::bundle_source::BundleFetchError;
use crate::dev_server_target::{DevServerTarget, Scheme};
use crate::protocol_handshake::{check_protocol_version, ProtocolMismatch};

/// dev-server がホストに full reload を促す WS メッセージ本文。`@torimi/dev-server` の
/// `RELOAD_MESSAGE` / Web ホストの `RELOAD_MESSAGE` と一致させる wire 契約（値で複製する）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub const RELOAD_MESSAGE: &str = "reload";

/// dev-server が reload シグナルを流す WS ルート。`@torimi/dev-server` の `RELOAD_ROUTE` /
/// Web ホストの `DEFAULT_RELOAD_ROUTE` と一致させる wire 契約（値で複製する）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub const RELOAD_ROUTE: &str = "/reload";

/// WS 切断後に再接続するまでの待ち時間。dev-server 再起動・瞬断の後に繋ぎ直す。
/// **プレースホルダ値**（実値調整は #8, ADR-0001。Web ホストの `WS_RECONNECT_BACKOFF_MS = 1_000`
/// と対称）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub const WS_RECONNECT_BACKOFF: Duration = Duration::from_secs(1);

/// 1 回分の boot（fetch → 構築/eval → version 突き合わせ）の失敗。どちらも明示エラーにし、
/// ランタイムの pump（描画）に進ませない＝謎クラッシュにしない（#530 / #533）。
#[derive(Debug, PartialEq, Eq)]
pub enum BootError {
    /// dev-server からのバンドル取得に失敗した。
    Fetch(BundleFetchError),
    /// バンドル encoder の版数がホスト decoder と不一致（明示エラー UI 用に両版数を持つ）。
    ProtocolMismatch(ProtocolMismatch),
}

/// Torimi Android ホストの 1 boot：dev-server からバンドルを取得 → Hermes ランタイムを
/// **構築（= eval。`new_hermes_app(.., &bundle)`）** → バンドルが立てた protocol version を読み、
/// ホスト decoder 版数と突き合わせる。一致時のみランタイムを返し（pump に進める）、不一致／取得失敗は
/// [`BootError`] で止める（mount もクラッシュもさせない）。full reload はこれを**もう一度呼ぶだけ**で
/// ランタイムが作り直され新バンドルが再 eval される（state は飛ぶ）。
///
/// device 依存（実 fetch / Hermes 構築 / global 読み）は注入シームに逃がし、ここは順序と
/// 突き合わせ配線だけを担うのでホストで契約テストできる（Web の `bootTorimiHost` と対称）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn boot_runtime<R>(
    host_version: u32,
    fetch: impl FnOnce() -> Result<String, BundleFetchError>,
    build: impl FnOnce(&str) -> R,
    read_bundle_version: impl FnOnce(&R) -> Option<u32>,
) -> Result<R, BootError> {
    let bundle = fetch().map_err(BootError::Fetch)?;
    // ランタイム構築 = バンドルの eval（global `__torimiProtocolVersion` / `__tsubame` が立つ）。
    let runtime = build(&bundle);
    let bundle_version = read_bundle_version(&runtime);
    // version 突き合わせは Web #530 と同じ純ロジック。不一致なら pump させずに明示エラーで返す。
    match check_protocol_version(host_version, bundle_version) {
        Ok(()) => Ok(runtime),
        Err(mismatch) => Err(BootError::ProtocolMismatch(mismatch)),
    }
}

/// reload WS の URL を、バンドル fetch と共有する target（ADR-0002 / #740）から組み立てる。
/// scheme は target の HTTP(S) を WS(S) へ写す — `https` 由来の接続（公開 Demo Endpoint・
/// ADR-0003）では `wss` になる（#742）。route はサーバルートの [`RELOAD_ROUTE`] 固定
/// （target の path はバンドルの置き場所であって reload の場所ではない）。Web ホストが
/// `new URL(reloadRoute, devServerUrl).href.replace(/^http/, 'ws')` で作る URL と同形の wire。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn reload_ws_url(target: &DevServerTarget) -> String {
    let ws_scheme = match target.scheme() {
        Scheme::Http => "ws",
        Scheme::Https => "wss",
    };
    format!("{ws_scheme}://{}:{}{RELOAD_ROUTE}", target.host(), target.port())
}

/// reload シグナルを運ぶ WS への最小ポート。device 既定は OS スタック（Kotlin・ADR-0002）へ
/// 委譲する薄いアダプタ（`reload_socket`）だが、テストはこれを注入して実 WS / 実 socket を
/// 巻き込まずに配線を観測する（Web の `ReloadSocket` と対称）。`Rc` 越しに後から配線するので
/// 各メソッドは `&self`。
pub trait ReloadSocket {
    /// テキストメッセージ受信時のコールバックを登録する。
    fn on_message(&self, cb: Box<dyn FnMut(&str)>);
    /// 切断時のコールバックを登録する。
    fn on_close(&self, cb: Box<dyn FnMut()>);
    /// 接続を閉じる。
    fn close(&self);
}

/// {@link subscribe_reload} の注入シーム / コールバック束。device グルーは実 WS connect と
/// 実タイマーを、テストは偽 socket と手動スケジューラを渡す。
pub struct SubscribeReloadOptions {
    /// 接続先（バンドル fetch と同じ target を共有する＝同じ配信点を指す・保持）。
    pub target: DevServerTarget,
    /// `reload` 受信時に呼ぶ。ホストはここで full reload（boot_runtime 再実行）を起こす。
    pub on_reload: Box<dyn FnMut()>,
    /// WS を張るシーム。既定は OS スタック委譲（`reload_socket::connect_reload_ws`）。テストは
    /// 偽 socket を返す。
    pub connect: Box<dyn Fn(&str) -> Rc<dyn ReloadSocket>>,
    /// 再接続の遅延スケジュールシーム。既定は実タイマー。テストは手動で発火する。
    pub schedule_reconnect: Box<dyn Fn(Box<dyn FnOnce()>, Duration)>,
}

/// reload 購読のハンドル。閉じると以後の再接続も止める。
pub struct ReloadSubscription {
    controller: Rc<ReloadController>,
}

impl ReloadSubscription {
    /// 購読を止める：WS を閉じ、以後の再接続も行わない。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn close(&self) {
        self.controller.close();
    }
}

struct ReloadState {
    stopped: bool,
    socket: Option<Rc<dyn ReloadSocket>>,
}

struct ReloadController {
    ws_url: String,
    connect: Box<dyn Fn(&str) -> Rc<dyn ReloadSocket>>,
    schedule_reconnect: Box<dyn Fn(Box<dyn FnOnce()>, Duration)>,
    on_reload: RefCell<Box<dyn FnMut()>>,
    state: RefCell<ReloadState>,
}

impl ReloadController {
    fn open(self: &Rc<Self>) {
        if self.state.borrow().stopped {
            return;
        }
        let socket = (self.connect)(&self.ws_url);

        // `reload` 受信で full reload を起こす。それ以外のメッセージは無視する。
        let me = Rc::clone(self);
        socket.on_message(Box::new(move |data| {
            if data == RELOAD_MESSAGE {
                (me.on_reload.borrow_mut())();
            }
        }));

        // 切断時は名前付き backoff で繋ぎ直す（停止後はしない）。dev-server 再起動・瞬断に耐える。
        let me = Rc::clone(self);
        socket.on_close(Box::new(move || {
            if me.state.borrow().stopped {
                return;
            }
            let reopen = Rc::clone(&me);
            (me.schedule_reconnect)(Box::new(move || reopen.open()), WS_RECONNECT_BACKOFF);
        }));

        self.state.borrow_mut().socket = Some(socket);
    }

    fn close(&self) {
        self.state.borrow_mut().stopped = true;
        if let Some(socket) = self.state.borrow().socket.as_ref() {
            socket.close();
        }
    }
}

/// dev-server の reload WS を購読し、`reload` 受信ごとに `on_reload` を起こす。切断時は名前付き
/// backoff（{@link WS_RECONNECT_BACKOFF}）で繋ぎ直す。ホスト側は WS を中継するだけで、reload の
/// 意味づけ（再 fetch → ランタイム再構築 → 再 eval）は `on_reload`（= boot_runtime 再実行）が担う
/// （ADR-0001：ホストのネイティブ契約は full reload / HMR で不変）。Web の `subscribeReload` と対称。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn subscribe_reload(options: SubscribeReloadOptions) -> ReloadSubscription {
    let controller = Rc::new(ReloadController {
        ws_url: reload_ws_url(&options.target),
        connect: options.connect,
        schedule_reconnect: options.schedule_reconnect,
        on_reload: RefCell::new(options.on_reload),
        state: RefCell::new(ReloadState {
            stopped: false,
            socket: None,
        }),
    });
    controller.open();
    ReloadSubscription { controller }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// build/eval された回数を数える偽ランタイム（full reload で作り直されることの観測用）。
    #[derive(Debug, PartialEq, Eq)]
    struct FakeRuntime {
        bundle: String,
    }

    #[test]
    fn boot_builds_the_runtime_and_returns_it_when_versions_match() {
        let runtime = boot_runtime(
            1,
            || Ok("globalThis.__torimiProtocolVersion = 1;".to_owned()),
            |src| FakeRuntime { bundle: src.to_owned() },
            |_rt| Some(1),
        )
        .expect("matching versions should boot");
        assert_eq!(runtime.bundle, "globalThis.__torimiProtocolVersion = 1;");
    }

    #[test]
    fn version_mismatch_is_an_explicit_error_not_a_crash() {
        // バンドルを eval（構築）した後で版数が食い違ったら、pump させずに明示エラーで返す。
        // ランタイムは作られても（eval 済み）、突き合わせで弾いてクラッシュさせない（#530）。
        let mut built = false;
        let result = boot_runtime(
            1,
            || Ok("src".to_owned()),
            |src| {
                built = true;
                FakeRuntime { bundle: src.to_owned() }
            },
            |_rt| Some(2),
        );
        assert!(built, "runtime is still built (bundle eval'd) before the handshake");
        match result {
            Err(BootError::ProtocolMismatch(m)) => {
                assert_eq!(m.host_version, 1);
                assert_eq!(m.bundle_version, Some(2));
            }
            other => panic!("expected an explicit protocol mismatch, got {other:?}"),
        }
    }

    #[test]
    fn fetch_failure_short_circuits_before_building_the_runtime() {
        let mut built = false;
        let result = boot_runtime(
            1,
            || Err(BundleFetchError::Platform("HTTP 404 from http://10.0.2.2:5179/bundle.js".to_owned())),
            |src| {
                built = true;
                FakeRuntime { bundle: src.to_owned() }
            },
            |_rt| Some(1),
        );
        assert!(!built, "the runtime must not be built when the bundle fetch fails");
        assert_eq!(
            result,
            Err(BootError::Fetch(BundleFetchError::Platform(
                "HTTP 404 from http://10.0.2.2:5179/bundle.js".to_owned()
            )))
        );
    }

    #[test]
    fn reload_rebuilds_the_runtime_and_re_evals_the_new_bundle() {
        // full reload = boot_runtime をもう一度呼ぶだけ。新しいバンドルが取り直され、ランタイムが
        // 作り直されて（再 eval）返る（state は飛ぶ・#529 と対称）。
        let builds = RefCell::new(0u32);
        let boot = |bundle: &'static str| {
            boot_runtime(
                1,
                || Ok(bundle.to_owned()),
                |src| {
                    *builds.borrow_mut() += 1;
                    FakeRuntime { bundle: src.to_owned() }
                },
                |_rt| Some(1),
            )
        };

        let first = boot("globalThis.__tsubame = /* v1 */ {};").unwrap();
        let second = boot("globalThis.__tsubame = /* edited */ {};").unwrap();

        assert_eq!(*builds.borrow(), 2, "each reload rebuilds the Hermes runtime");
        assert_ne!(first.bundle, second.bundle, "the new (re-fetched) bundle is re-eval'd");
    }

    // ── subscribe_reload（Web の reload.test.ts と対称）──────────────────────────

    /// メッセージ / close を後から差し込めるテスト用 socket（Web の fakeSocket と対称）。
    #[derive(Default)]
    struct FakeSocket {
        on_message: RefCell<Option<Box<dyn FnMut(&str)>>>,
        on_close: RefCell<Option<Box<dyn FnMut()>>>,
        closed: std::cell::Cell<bool>,
    }

    impl FakeSocket {
        fn emit_message(&self, data: &str) {
            if let Some(cb) = self.on_message.borrow_mut().as_mut() {
                cb(data);
            }
        }
        fn emit_close(&self) {
            if let Some(cb) = self.on_close.borrow_mut().as_mut() {
                cb();
            }
        }
    }

    impl ReloadSocket for FakeSocket {
        fn on_message(&self, cb: Box<dyn FnMut(&str)>) {
            *self.on_message.borrow_mut() = Some(cb);
        }
        fn on_close(&self, cb: Box<dyn FnMut()>) {
            *self.on_close.borrow_mut() = Some(cb);
        }
        fn close(&self) {
            self.closed.set(true);
        }
    }

    /// schedule_reconnect が捕まえた遅延発火（手動で走らせる）。
    #[derive(Default)]
    struct CapturedSchedule {
        delay: std::cell::Cell<Option<Duration>>,
        fire: RefCell<Option<Box<dyn FnOnce()>>>,
    }

    #[test]
    fn ws_url_targets_the_reload_route_over_the_ws_scheme() {
        // LAN dev（http target）は従来どおり ws。reload WS はサーバルートの RELOAD_ROUTE 固定
        // （target の path はバンドルの置き場所であって reload の場所ではない）。
        let lan = crate::dev_server_target::resolve(Some("10.0.2.2:5179"));
        assert_eq!(reload_ws_url(&lan), "ws://10.0.2.2:5179/reload");
    }

    #[test]
    fn an_https_target_subscribes_over_wss() {
        // 公開 Demo Endpoint（ADR-0003）は https で貼られる — reload 購読は wss に乗る
        // （ADR-0002 / #742。バンドル fetch と同じ target を共有し、scheme だけ WS へ写す）。
        let demo = crate::dev_server_target::resolve(Some("https://demo.example/solid/bundle.js"));
        assert_eq!(reload_ws_url(&demo), "wss://demo.example:443/reload");
    }

    #[test]
    fn invokes_on_reload_when_the_dev_server_sends_a_reload_message() {
        let socket = Rc::new(FakeSocket::default());
        let reloads = Rc::new(std::cell::Cell::new(0u32));
        let reloads_cb = Rc::clone(&reloads);
        let socket_for_connect = Rc::clone(&socket);

        subscribe_reload(SubscribeReloadOptions {
            target: crate::dev_server_target::resolve(Some("dev.example:5179")),
            on_reload: Box::new(move || reloads_cb.set(reloads_cb.get() + 1)),
            connect: Box::new(move |_url| Rc::clone(&socket_for_connect) as Rc<dyn ReloadSocket>),
            schedule_reconnect: Box::new(|_fire, _delay| {}),
        });

        socket.emit_message(RELOAD_MESSAGE);
        assert_eq!(reloads.get(), 1);
    }

    #[test]
    fn ignores_non_reload_messages() {
        let socket = Rc::new(FakeSocket::default());
        let reloads = Rc::new(std::cell::Cell::new(0u32));
        let reloads_cb = Rc::clone(&reloads);
        let socket_for_connect = Rc::clone(&socket);

        subscribe_reload(SubscribeReloadOptions {
            target: crate::dev_server_target::resolve(Some("dev.example:5179")),
            on_reload: Box::new(move || reloads_cb.set(reloads_cb.get() + 1)),
            connect: Box::new(move |_url| Rc::clone(&socket_for_connect) as Rc<dyn ReloadSocket>),
            schedule_reconnect: Box::new(|_fire, _delay| {}),
        });

        socket.emit_message("something-else");
        assert_eq!(reloads.get(), 0);
    }

    #[test]
    fn connects_to_the_dev_server_reload_route() {
        let connected = Rc::new(RefCell::new(Vec::<String>::new()));
        let connected_cb = Rc::clone(&connected);

        subscribe_reload(SubscribeReloadOptions {
            target: crate::dev_server_target::resolve(Some("127.0.0.1:5181")),
            on_reload: Box::new(|| {}),
            connect: Box::new(move |url| {
                connected_cb.borrow_mut().push(url.to_owned());
                Rc::new(FakeSocket::default()) as Rc<dyn ReloadSocket>
            }),
            schedule_reconnect: Box::new(|_fire, _delay| {}),
        });

        assert_eq!(connected.borrow().as_slice(), ["ws://127.0.0.1:5181/reload"]);
    }

    #[test]
    fn reconnects_after_the_named_backoff_when_the_socket_closes() {
        let sockets = Rc::new(RefCell::new(Vec::<Rc<FakeSocket>>::new()));
        let connect_count = Rc::new(std::cell::Cell::new(0u32));
        let schedule = Rc::new(CapturedSchedule::default());

        let sockets_cb = Rc::clone(&sockets);
        let connect_count_cb = Rc::clone(&connect_count);
        let schedule_cb = Rc::clone(&schedule);

        subscribe_reload(SubscribeReloadOptions {
            target: crate::dev_server_target::resolve(Some("dev.example:5179")),
            on_reload: Box::new(|| {}),
            connect: Box::new(move |_url| {
                connect_count_cb.set(connect_count_cb.get() + 1);
                let socket = Rc::new(FakeSocket::default());
                sockets_cb.borrow_mut().push(Rc::clone(&socket));
                socket as Rc<dyn ReloadSocket>
            }),
            schedule_reconnect: Box::new(move |fire, delay| {
                schedule_cb.delay.set(Some(delay));
                *schedule_cb.fire.borrow_mut() = Some(fire);
            }),
        });

        assert_eq!(connect_count.get(), 1);

        // 切断 → 名前付き backoff で再接続がスケジュールされる。
        sockets.borrow()[0].emit_close();
        assert_eq!(schedule.delay.get(), Some(WS_RECONNECT_BACKOFF));

        // スケジュールされた再接続が走ると、新しい接続が張られる。
        let fire = schedule.fire.borrow_mut().take().expect("a reconnect was scheduled");
        fire();
        assert_eq!(connect_count.get(), 2);
    }

    #[test]
    fn stops_reconnecting_once_the_subscription_is_closed() {
        let socket = Rc::new(FakeSocket::default());
        let scheduled = Rc::new(std::cell::Cell::new(false));
        let socket_for_connect = Rc::clone(&socket);
        let scheduled_cb = Rc::clone(&scheduled);

        let subscription = subscribe_reload(SubscribeReloadOptions {
            target: crate::dev_server_target::resolve(Some("dev.example:5179")),
            on_reload: Box::new(|| {}),
            connect: Box::new(move |_url| Rc::clone(&socket_for_connect) as Rc<dyn ReloadSocket>),
            schedule_reconnect: Box::new(move |_fire, _delay| scheduled_cb.set(true)),
        });

        subscription.close();
        assert!(socket.closed.get(), "closing the subscription closes the socket");

        // 閉じた後の切断イベントでは再接続をスケジュールしない。
        socket.emit_close();
        assert!(!scheduled.get(), "no reconnect is scheduled after close");
    }
}
