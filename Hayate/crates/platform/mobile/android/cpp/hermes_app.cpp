// Hermes(JSI) ホスト + ランタイムの C++ 実装（ADR-0112）。**device 未検証**。
//
// この TU はホスト `cargo check` ではコンパイルされない（build.rs が
// target_os=android かつ feature=tsubame-js のときだけビルドする）。JSI / Hermes
// ヘッダ（<jsi/jsi.h> / <hermes/hermes.h>）と libhermes は Gradle/NDK 側から供給する。
//
// 設計（ADR-0112）:
//  - `__hayateHost` を jsi::HostObject として注入。各プロパティは jsi::Function を
//    返し、引数（Float64Array/Float32Array/string[]）を取り出して
//    `rust::Box<JsHostBridge>` のメソッドへ降ろす。
//  - バンドル（main.android.tsx 由来）は `globalThis.__tsubame` を公開するので、
//    pump_frame はそれを呼ぶだけ。resize は native→tree 直結（app.rs が
//    `set_viewport` を直接駆動）で JS を経路から外したため、`__hayateHost.on_resize`
//    も `__tsubame.resize` も持たない（issue #475）。
//  - スレッドは android_main 単一（ADR-0003）。
// 生成ブリッジヘッダ（JsHostBridge の完全定義 + 共有構造体 FfiEventRow/FfiWireAtom
// + crate コピーの hermes_app.h 経由で HermesApp 宣言）を取り込む。hermes_app.h を
// 直接 include しないのは、相対パスと crate コピーパスで二重に取り込まれて
// #pragma once が効かず多重定義になるのを避けるため（ADR-0112）。
#include "hayate-adapter-android/src/hermes_bridge.rs.h"

#include <jsi/jsi.h>
#include <hermes/hermes.h>

#include <android/log.h>

#include <cmath>
#include <exception>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#define HAYATE_LOG_TAG "hayate-tsubame"
#define HAYATE_LOGE(...) \
  __android_log_print(ANDROID_LOG_ERROR, HAYATE_LOG_TAG, __VA_ARGS__)

namespace hayate {

using namespace facebook;

namespace {

// jsi の TypedArray から連続データのスライスを取り出すヘルパ。Hermes は
// ArrayBuffer の data() を公開する。byteOffset/length は TypedArray から読む。
template <typename T>
std::vector<T> typed_array_to_vec(jsi::Runtime& rt, const jsi::Value& value) {
  std::vector<T> out;
  if (!value.isObject()) return out;
  jsi::Object obj = value.getObject(rt);
  if (!obj.isArrayBuffer(rt) && !obj.hasProperty(rt, "buffer")) return out;
  // TypedArray: { buffer, byteOffset, length }
  jsi::ArrayBuffer buf = obj.hasProperty(rt, "buffer")
                             ? obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt)
                             : obj.getArrayBuffer(rt);
  size_t byte_offset = obj.hasProperty(rt, "byteOffset")
                           ? static_cast<size_t>(obj.getProperty(rt, "byteOffset").asNumber())
                           : 0;
  size_t length = obj.hasProperty(rt, "length")
                      ? static_cast<size_t>(obj.getProperty(rt, "length").asNumber())
                      : buf.size(rt) / sizeof(T);
  const uint8_t* base = buf.data(rt) + byte_offset;
  const T* typed = reinterpret_cast<const T*>(base);
  out.assign(typed, typed + length);
  return out;
}

// string[] → std::vector<std::string>。
std::vector<std::string> string_array_to_vec(jsi::Runtime& rt, const jsi::Value& value) {
  std::vector<std::string> out;
  if (!value.isObject()) return out;
  jsi::Array arr = value.getObject(rt).asArray(rt);
  size_t n = arr.size(rt);
  out.reserve(n);
  for (size_t i = 0; i < n; ++i) {
    out.push_back(arr.getValueAtIndex(rt, i).asString(rt).utf8(rt));
  }
  return out;
}

// Vec<f64>/Vec<f32> → JS Array（element_subtree_ids / element_get_bounds 用）。
template <typename Vec>
jsi::Value vec_to_js_array(jsi::Runtime& rt, const Vec& v) {
  jsi::Array arr(rt, v.size());
  for (size_t i = 0; i < v.size(); ++i) {
    arr.setValueAtIndex(rt, i, jsi::Value(static_cast<double>(v[i])));
  }
  return arr;
}

// `set_request_redraw` が登録した JS コールバックの置き場（ADR-0080/0126 の Android 延長）。
// HayateHostObject（JS から書き込む）と HermesApp::Impl（native の wake から読む）の両方が
// 共有する必要があるため shared_ptr で持つ。
struct RedrawSlot {
  std::optional<jsi::Function> fn;
};

// `request_pump` が立てるフラグの置き場。JS の frame ループが armed になるたびに
// HayateHostObject が true を書き込み、HermesApp::consume_wants_pump() が読んで消す
// （ADR-0003: 単一スレッドなので plain bool で足りる）。
struct PumpFlag {
  bool wanted = false;
};

// __hayateHost: RawHayate を満たす jsi::HostObject。
class HayateHostObject : public jsi::HostObject {
 public:
  HayateHostObject(rust::Box<JsHostBridge> bridge,
                    std::shared_ptr<RedrawSlot> redraw_slot,
                    std::shared_ptr<PumpFlag> pump_flag)
      : bridge_(std::move(bridge)),
        redraw_slot_(std::move(redraw_slot)),
        pump_flag_(std::move(pump_flag)) {}

  jsi::Value get(jsi::Runtime& rt, const jsi::PropNameID& name) override {
    std::string prop = name.utf8(rt);
    JsHostBridge& b = *bridge_;

    if (prop == "apply_mutations") {
      return jsi::Function::createFromHostFunction(
          rt, name, 3,
          [&b](jsi::Runtime& rt, const jsi::Value&, const jsi::Value* args,
               size_t /*count*/) -> jsi::Value {
            auto ops = typed_array_to_vec<double>(rt, args[0]);
            auto styles = typed_array_to_vec<float>(rt, args[1]);
            auto texts = string_array_to_vec(rt, args[2]);
            std::vector<std::string> texts_v(texts.begin(), texts.end());
            // rust::Vec<rust::String> ではなく std::vector を & で渡す（cxx の
            // CxxVector<CxxString>）。
            b.apply_mutations(rust::Slice<const double>(ops.data(), ops.size()),
                              rust::Slice<const float>(styles.data(), styles.size()),
                              texts_v);
            return jsi::Value::undefined();
          });
    }

    if (prop == "render") {
      return jsi::Function::createFromHostFunction(
          rt, name, 1,
          [&b](jsi::Runtime&, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            b.render(args[0].asNumber());
            return jsi::Value::undefined();
          });
    }

    if (prop == "register_listener") {
      return jsi::Function::createFromHostFunction(
          rt, name, 2,
          [&b](jsi::Runtime&, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            double id = b.register_listener(args[0].asNumber(),
                                            static_cast<uint32_t>(args[1].asNumber()));
            return jsi::Value(id);
          });
    }

    if (prop == "element_get_text_content") {
      return jsi::Function::createFromHostFunction(
          rt, name, 1,
          [&b](jsi::Runtime& rt, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            rust::String s = b.element_get_text_content(args[0].asNumber());
            return jsi::String::createFromUtf8(rt, std::string(s));
          });
    }

    if (prop == "element_subtree_ids") {
      return jsi::Function::createFromHostFunction(
          rt, name, 1,
          [&b](jsi::Runtime& rt, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            rust::Vec<double> v = b.element_subtree_ids(args[0].asNumber());
            return vec_to_js_array(rt, v);
          });
    }

    if (prop == "element_get_bounds") {
      return jsi::Function::createFromHostFunction(
          rt, name, 1,
          [&b](jsi::Runtime& rt, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            rust::Vec<float> v = b.element_get_bounds(args[0].asNumber());
            return vec_to_js_array(rt, v);
          });
    }

    if (prop == "poll_events") {
      return jsi::Function::createFromHostFunction(
          rt, name, 0,
          [&b](jsi::Runtime& rt, const jsi::Value&, const jsi::Value*,
               size_t) -> jsi::Value {
            rust::Vec<FfiEventRow> rows = b.poll_events();
            jsi::Array out(rt, rows.size());
            for (size_t i = 0; i < rows.size(); ++i) {
              const FfiEventRow& row = rows[i];
              jsi::Array jsrow(rt, row.atoms.size());
              for (size_t j = 0; j < row.atoms.size(); ++j) {
                const FfiWireAtom& a = row.atoms[j];
                if (a.is_text) {
                  jsrow.setValueAtIndex(
                      rt, j, jsi::String::createFromUtf8(rt, std::string(a.text)));
                } else {
                  jsrow.setValueAtIndex(rt, j, jsi::Value(a.number));
                }
              }
              out.setValueAtIndex(rt, i, std::move(jsrow));
            }
            return out;
          });
    }

    if (prop == "has_pending_visual_work") {
      return jsi::Function::createFromHostFunction(
          rt, name, 0,
          [&b](jsi::Runtime&, const jsi::Value&, const jsi::Value*,
               size_t) -> jsi::Value {
            return jsi::Value(b.has_pending_visual_work());
          });
    }

    if (prop == "set_request_redraw") {
      return jsi::Function::createFromHostFunction(
          rt, name, 1,
          [slot = redraw_slot_](jsi::Runtime& rt, const jsi::Value&,
                                 const jsi::Value* args, size_t) -> jsi::Value {
            if (args[0].isObject() && args[0].getObject(rt).isFunction(rt)) {
              slot->fn = args[0].getObject(rt).asFunction(rt);
            }
            return jsi::Value::undefined();
          });
    }

    if (prop == "request_pump") {
      return jsi::Function::createFromHostFunction(
          rt, name, 0,
          [flag = pump_flag_](jsi::Runtime&, const jsi::Value&, const jsi::Value*,
                               size_t) -> jsi::Value {
            flag->wanted = true;
            return jsi::Value::undefined();
          });
    }

    return jsi::Value::undefined();
  }

 private:
  rust::Box<JsHostBridge> bridge_;
  std::shared_ptr<RedrawSlot> redraw_slot_;
  std::shared_ptr<PumpFlag> pump_flag_;
};

}  // namespace

struct HermesApp::Impl {
  std::unique_ptr<jsi::Runtime> runtime;
  // バンドルの eval に成功し __tsubame が公開されたか。false の間は frame を
  // 呼ばない（JS エラーで __tsubame 未定義のときに毎フレーム例外を投げないため）。
  bool ready = false;
  // `set_request_redraw` で JS が登録したコールバックの置き場（`request_redraw` が読む）。
  std::shared_ptr<RedrawSlot> redraw_slot = std::make_shared<RedrawSlot>();
  // `request_pump` が立てたフラグの置き場（`consume_wants_pump` が読んで消す）。
  std::shared_ptr<PumpFlag> pump_flag = std::make_shared<PumpFlag>();
};

HermesApp::HermesApp(rust::Box<JsHostBridge> host, rust::Str bundle)
    : impl_(std::make_unique<Impl>()) {
  impl_->runtime = facebook::hermes::makeHermesRuntime();
  jsi::Runtime& rt = *impl_->runtime;

  // __hayateHost を注入。bridge を host_obj に move する前に生ポインタを退避する：rust::Box の
  // move は heap 上の JsHostBridge 実体を動かさない（Box はポインタ）ので、退避したポインタは
  // host_obj（＝runtime）の寿命の間ずっと有効。__hayateLog から Device Log シームへ流すのに使う。
  JsHostBridge* log_bridge = &*host;
  auto host_obj = std::make_shared<HayateHostObject>(
      std::move(host), impl_->redraw_slot, impl_->pump_flag);
  rt.global().setProperty(rt, "__hayateHost",
                          jsi::Object::createFromHostObject(rt, host_obj));

  // console.log 等が呼ぶ __hayateLog を **logcat と Device Log の両方** へ橋渡しする（#787）。
  // 従来の logcat 出力はそのまま残し（置き換えない・併存）、加えて Device Log シームへ積んで
  // USB なしで dev-server へ届ける。バッファ・seq 採番・フラッシュは Rust の純粋シームが所有する。
  rt.global().setProperty(
      rt, "__hayateLog",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "__hayateLog"), 2,
          [log_bridge](jsi::Runtime& rt, const jsi::Value&, const jsi::Value* args,
             size_t count) -> jsi::Value {
            std::string level = count > 0 ? args[0].toString(rt).utf8(rt) : "log";
            std::string message = count > 1 ? args[1].toString(rt).utf8(rt) : "";
            HAYATE_LOGE("[JS %s] %s", level.c_str(), message.c_str());
            log_bridge->log(rust::Str(level), rust::Str(message));
            return jsi::Value::undefined();
          }));

  // バンドルを eval（main.android.tsx 由来。__tsubame を公開する）。JS の例外
  // （jsi::JSError）はここで捕捉してメッセージと JS スタックをログに出す。捕まえない
  // と C++ 例外として android_main を抜け std::terminate でプロセスが落ちる。
  try {
    std::string src(bundle);
    rt.evaluateJavaScript(
        std::make_unique<jsi::StringBuffer>(std::move(src)), "tsubame.js");
    impl_->ready = true;
  } catch (const jsi::JSError& e) {
    HAYATE_LOGE("Tsubame バンドルの eval で JS 例外: %s\nJS stack:\n%s",
                e.getMessage().c_str(), e.getStack().c_str());
  } catch (const std::exception& e) {
    HAYATE_LOGE("Tsubame バンドルの eval で例外: %s", e.what());
  }
}

HermesApp::~HermesApp() = default;

void HermesApp::pump_frame(double timestamp_ms) {
  if (!impl_->ready) return;
  jsi::Runtime& rt = *impl_->runtime;
  try {
    jsi::Object tsubame = rt.global().getPropertyAsObject(rt, "__tsubame");
    jsi::Function pump = tsubame.getPropertyAsFunction(rt, "pumpFrame");
    pump.callWithThis(rt, tsubame, {jsi::Value(timestamp_ms)});
    // Hermes のマイクロタスク（Solid のスケジューラ）を排出する。
    if (auto* hermesRt = dynamic_cast<facebook::hermes::HermesRuntime*>(&rt)) {
      hermesRt->drainMicrotasks();
    }
  } catch (const jsi::JSError& e) {
    HAYATE_LOGE("pumpFrame で JS 例外: %s\nJS stack:\n%s", e.getMessage().c_str(),
                e.getStack().c_str());
    impl_->ready = false;  // 毎フレームのスパムを避けて止める。
  } catch (const std::exception& e) {
    HAYATE_LOGE("pumpFrame で例外: %s", e.what());
    impl_->ready = false;
  }
}

void HermesApp::request_redraw() {
  // eval 未成功、または JS がまだ set_request_redraw を呼んでいなければ何もしない。
  if (!impl_->ready || !impl_->redraw_slot->fn.has_value()) return;
  jsi::Runtime& rt = *impl_->runtime;
  try {
    impl_->redraw_slot->fn->call(rt);
  } catch (const jsi::JSError& e) {
    HAYATE_LOGE("request_redraw で JS 例外: %s\nJS stack:\n%s",
                e.getMessage().c_str(), e.getStack().c_str());
  } catch (const std::exception& e) {
    HAYATE_LOGE("request_redraw で例外: %s", e.what());
  }
}

bool HermesApp::consume_wants_pump() {
  bool wanted = impl_->pump_flag->wanted;
  impl_->pump_flag->wanted = false;
  return wanted;
}

double HermesApp::protocol_version() const {
  // eval に失敗していれば版数は読めない＝未埋め込み扱い（ホストが明示エラーにする）。
  if (!impl_->ready) return -1.0;
  jsi::Runtime& rt = *impl_->runtime;
  try {
    jsi::Value value = rt.global().getProperty(rt, "__torimiProtocolVersion");
    if (!value.isNumber()) return -1.0;
    double v = value.asNumber();
    // 非有限（NaN/Inf）は壊れた埋め込み → 未埋め込み扱い。
    if (!std::isfinite(v)) return -1.0;
    return v;
  } catch (const std::exception& e) {
    HAYATE_LOGE("protocol_version の読み取りで例外: %s", e.what());
    return -1.0;
  }
}

std::unique_ptr<HermesApp> new_hermes_app(rust::Box<JsHostBridge> host,
                                          rust::Str bundle) {
  return std::make_unique<HermesApp>(std::move(host), bundle);
}

}  // namespace hayate
