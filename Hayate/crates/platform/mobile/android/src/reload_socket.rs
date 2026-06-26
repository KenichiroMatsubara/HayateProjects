//! Miharashi Android ホストの reload WS クライアント（#533）。**device 未検証**。
//!
//! `miharashi_reload::subscribe_reload` の `connect` シームの device 既定実装。dev-server の
//! reload WS（`@miharashi/dev-server` の RELOAD_ROUTE。Web ホストは `WebSocket` で繋ぐ）へ素の
//! TCP 上で RFC6455 ハンドシェイクを張り、サーバ → クライアントのテキストフレーム（`reload`）を
//! 受ける薄いアダプタ。依存追加なし。
//!
//! 単一スレッド契約（ADR-0003）を守るため、フレーム読みは背景スレッドが行い、受信を mpsc で
//! main へ渡す。main の poll ループが毎フレーム [`ReloadWsSocket::pump`] を呼んで、登録済みの
//! `on_message` / `on_close` を **main スレッドで** 発火する（boot_runtime 再実行＝ Hermes/tree に
//! 触るため main で動かす必要がある）。実 WS 配線は実機で検証する（本 issue 外）。
//!
//! 中身は素の std（TCP / スレッド / mpsc）なので、実際に駆動するのは device の `app_tsubame`
//! だけでもホスト `cargo check` でコンパイル検証はできる（host では未使用なので dead_code は許可）。
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::miharashi_reload::ReloadSocket;

/// RFC6455 ハンドシェイクの `Sec-WebSocket-Key`（16 byte を base64 した固定値）。サーバは値の
/// ランダム性を要求せず Accept ハッシュを返すだけなので、固定キーで足りる（依存追加回避）。
const WS_HANDSHAKE_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
/// 期待する WS アップグレード成功 status line の断片。
const WS_SWITCHING_PROTOCOLS: &str = " 101 ";
/// WS フレーム opcode：テキスト。
const WS_OPCODE_TEXT: u8 = 0x1;
/// WS フレーム opcode：close。
const WS_OPCODE_CLOSE: u8 = 0x8;

/// 背景スレッド → main へ渡す reload 受信イベント。
enum WsEvent {
    /// サーバが送ったテキストフレーム本文（`reload` か否かは購読側が判定する）。
    Message(String),
    /// 接続が閉じた（EOF / close フレーム / I/O エラー）。main は backoff 再接続する。
    Closed,
}

/// reload WS への接続ハンドル。`subscribe_reload` の `connect` が返す [`ReloadSocket`]。
pub struct ReloadWsSocket {
    events: Receiver<WsEvent>,
    /// 読みスレッドを解くための shutdown 用クローン（接続失敗時は `None`）。
    shutdown_handle: Option<TcpStream>,
    on_message: RefCell<Option<Box<dyn FnMut(&str)>>>,
    on_close: RefCell<Option<Box<dyn FnMut()>>>,
    closed: Cell<bool>,
}

impl ReloadWsSocket {
    /// 背景スレッドが mpsc に積んだ受信を main で排出し、登録済みコールバックを発火する。
    /// poll ループが毎フレーム呼ぶ（blocking read を main から外すための drain シーム）。
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
        // 読みスレッドの blocking read を解く。次の pump で Closed が（再接続抑止下で）流れる。
        if let Some(stream) = self.shutdown_handle.as_ref() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

/// `ws://host:port/path` を素の TCP + RFC6455 ハンドシェイクで開き、読みスレッドを起こす。
/// 接続 / ハンドシェイク失敗時も「即 Closed を積んだ」socket を返す — 購読側はそれを backoff
/// 再接続のトリガにする（Web の `WebSocket` が onclose を出すのと同型）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn connect_reload_ws(ws_url: &str) -> std::rc::Rc<ReloadWsSocket> {
    let (tx, rx): (Sender<WsEvent>, Receiver<WsEvent>) = channel();

    match open_ws(ws_url) {
        Ok(stream) => {
            let shutdown_handle = stream.try_clone().ok();
            thread::spawn(move || read_frames(stream, &tx));
            std::rc::Rc::new(ReloadWsSocket {
                events: rx,
                shutdown_handle,
                on_message: RefCell::new(None),
                on_close: RefCell::new(None),
                closed: Cell::new(false),
            })
        }
        Err(err) => {
            log::warn!("hayate-adapter-android: reload WS 接続に失敗（backoff 再試行）: {err}");
            let _ = tx.send(WsEvent::Closed);
            std::rc::Rc::new(ReloadWsSocket {
                events: rx,
                shutdown_handle: None,
                on_message: RefCell::new(None),
                on_close: RefCell::new(None),
                closed: Cell::new(false),
            })
        }
    }
}

/// `ws://host:port/path` をパースし、TCP connect → RFC6455 GET Upgrade を送って 101 を確認する。
fn open_ws(ws_url: &str) -> std::io::Result<TcpStream> {
    let rest = ws_url.strip_prefix("ws://").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "reload URL must be ws://")
    })?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    let mut stream = TcpStream::connect(authority)?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {WS_HANDSHAKE_KEY}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;

    // ハンドシェイク応答のヘッダブロック（\r\n\r\n まで）を読み、101 を確認する。
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "reload WS handshake closed early",
            ));
        }
        head.push(byte[0]);
    }
    let head_text = String::from_utf8_lossy(&head);
    if !head_text.contains(WS_SWITCHING_PROTOCOLS) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "reload WS did not switch protocols (no 101)",
        ));
    }
    Ok(stream)
}

/// サーバ → クライアントのフレーム（unmasked）を読み続け、テキストは [`WsEvent::Message`]、
/// close / EOF / エラーは [`WsEvent::Closed`] で main へ渡す。
fn read_frames(mut stream: TcpStream, tx: &Sender<WsEvent>) {
    loop {
        let mut header = [0u8; 2];
        if stream.read_exact(&mut header).is_err() {
            let _ = tx.send(WsEvent::Closed);
            return;
        }
        let opcode = header[0] & 0x0F;
        let mut len = (header[1] & 0x7F) as usize;
        if len == 126 {
            let mut ext = [0u8; 2];
            if stream.read_exact(&mut ext).is_err() {
                let _ = tx.send(WsEvent::Closed);
                return;
            }
            len = u16::from_be_bytes(ext) as usize;
        } else if len == 127 {
            let mut ext = [0u8; 8];
            if stream.read_exact(&mut ext).is_err() {
                let _ = tx.send(WsEvent::Closed);
                return;
            }
            len = u64::from_be_bytes(ext) as usize;
        }
        // サーバフレームは unmasked（RFC6455）なので mask key は無い。
        let mut payload = vec![0u8; len];
        if stream.read_exact(&mut payload).is_err() {
            let _ = tx.send(WsEvent::Closed);
            return;
        }
        if opcode == WS_OPCODE_CLOSE {
            let _ = tx.send(WsEvent::Closed);
            return;
        }
        if opcode == WS_OPCODE_TEXT {
            let _ = tx.send(WsEvent::Message(String::from_utf8_lossy(&payload).into_owned()));
        }
        // ping/pong 等は無視して読み続ける（reload には不要）。
    }
}
