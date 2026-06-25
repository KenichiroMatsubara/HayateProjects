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

 private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

// cxx extern "C++" のファクトリ。host を __hayateHost として注入し bundle を eval。
std::unique_ptr<HermesApp> new_hermes_app(rust::Box<JsHostBridge> host,
                                          rust::Str bundle);

}  // namespace hayate
