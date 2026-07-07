// Hermes(JSI) ホスト + ランタイムの C++ 宣言（ADR-0112）。**device 未検証**。
//
// cxx の extern "C++" 側。Rust（hermes_bridge.rs）から `new_hermes_app` で生成し、
// 毎フレーム `pump_frame` を呼ぶ。実装は hermes_app.cpp。resize は native→tree 直結
// （app.rs が `set_viewport` を直接駆動）で JS を経路から外したため無い（issue #475）。
//
// 注意（cxx 循環 include 回避）: このヘッダは生成ヘッダ `hermes_bridge.rs.h` を
// include しない。bridge 側が `include!("...hermes_app.h")` でこのヘッダを取り込む
// ため、ここで rs.h を include すると循環＋多重定義になる。opaque Rust 型
// `JsHostBridge` は前方宣言だけ行い、完全定義は実装 .cpp が rs.h を include して得る。
#pragma once

#include <memory>

#include "rust/cxx.h"

namespace hayate {

// cxx ブリッジ（hermes_bridge.rs）が定義する opaque Rust 型の前方宣言。
// rust::Box<JsHostBridge> を宣言（非定義）に使う分には不完全型で足りる。
struct JsHostBridge;

// Hermes ランタイム + 注入済み __hayateHost + ロード済みバンドルを保持する。
class HermesApp {
 public:
  HermesApp(rust::Box<JsHostBridge> host, rust::Str bundle);
  ~HermesApp();

  // globalThis.__tsubame.pumpFrame(timestamp_ms) を呼び、続けて Hermes の
  // マイクロタスクキューを排出する。
  void pump_frame(double timestamp_ms);

  // JS が `__hayateHost.set_request_redraw(cb)` で登録したコールバックを呼ぶ（あれば）。
  // Android は入力を native→tree 直結で処理するが（issue #475）、JS 側の frame ループは
  // 別に自分の armed 状態（`pendingFrame`）を持つため、native の on-demand ループが起きた
  // だけでは JS 側は再武装されない。native の入力 wake（タッチ/IME）が起きるたびにこれを
  // 呼び、JS の `scheduleFrame` を叩いて armed 状態を揃える（ADR-0080/0126 を Android へ延長）。
  void request_redraw();

  // JS が `__hayateHost.request_pump()` を呼んだか（＝JS 側の frame ループが armed に
  // なったか）を読んで消費する。native の on-demand ループ（app_tsubame.rs）はこれを
  // 毎イテレーション呼び、true なら wake する。web の requestAnimationFrame に相当する
  // 自走クロックが無い Android では、click ハンドラの `setStyle` 等が自己再武装しても
  // これが無いと二度と pump されず、見た目の更新が永久に止まる。
  bool consume_wants_pump();

  // eval 済みバンドルが立てた globalThis.__torimiProtocolVersion を読む（#533）。有限数なら
  // その値、未埋め込み / 非数値（契約違反 / 壊れた埋め込み）なら -1.0 を返す。ホスト（Rust 側
  // app_tsubame）はこれを Option<u32> に直し、`@torimi/protocol-handshake` 同型の突き合わせに
  // かける。バンドル → ホストの wire global 名は Web（main.torimi.tsx）と共有。
  double protocol_version() const;

 private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

// cxx extern "C++" のファクトリ。host を __hayateHost として注入し bundle を eval。
std::unique_ptr<HermesApp> new_hermes_app(rust::Box<JsHostBridge> host,
                                          rust::Str bundle);

}  // namespace hayate
