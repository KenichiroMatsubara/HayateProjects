// Hermes(JSI) ホスト + ランタイムの C++ 宣言（ADR-0112）。**device 未検証**。
//
// cxx の extern "C++" 側。Rust（hermes_bridge.rs）から `new_hermes_app` で生成し、
// 毎フレーム `pump_frame` / `resize` を呼ぶ。実装は hermes_app.cpp。
#pragma once

#include <memory>

#include "rust/cxx.h"

// cxx 生成のブリッジヘッダ（JsHostBridge / FfiEventRow 等）。include パスは
// cxx-build の出力レイアウトに依存し、device ビルドで調整が要る可能性がある。
#include "hayate-adapter-android/src/hermes_bridge.rs.h"

namespace hayate {

// Hermes ランタイム + 注入済み __hayateHost + ロード済みバンドルを保持する。
class HermesApp {
 public:
  HermesApp(rust::Box<JsHostBridge> host, rust::Str bundle);
  ~HermesApp();

  // globalThis.__tsubame.pumpFrame(timestamp_ms) を呼び、続けて Hermes の
  // マイクロタスクキューを排出する。
  void pump_frame(double timestamp_ms);

  // globalThis.__tsubame.resize(width, height, scale) を呼ぶ。
  void resize(float width, float height, float scale);

 private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

// cxx extern "C++" のファクトリ。host を __hayateHost として注入し bundle を eval。
std::unique_ptr<HermesApp> new_hermes_app(rust::Box<JsHostBridge> host,
                                          rust::Str bundle);

}  // namespace hayate
