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
//    pump_frame / resize はそれを呼ぶだけ。
//  - スレッドは android_main 単一（ADR-0003）。
// 生成ブリッジヘッダ（JsHostBridge の完全定義 + 共有構造体 FfiEventRow/FfiWireAtom
// + crate コピーの hermes_app.h 経由で HermesApp 宣言）を取り込む。hermes_app.h を
// 直接 include しないのは、相対パスと crate コピーパスで二重に取り込まれて
// #pragma once が効かず多重定義になるのを避けるため（ADR-0112）。
#include "hayate-adapter-android/src/hermes_bridge.rs.h"

#include <jsi/jsi.h>
#include <hermes/hermes.h>

#include <android/log.h>

#include <exception>
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

// __hayateHost: RawHayate を満たす jsi::HostObject。
class HayateHostObject : public jsi::HostObject {
 public:
  explicit HayateHostObject(rust::Box<JsHostBridge> bridge)
      : bridge_(std::move(bridge)) {}

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

    if (prop == "on_resize") {
      return jsi::Function::createFromHostFunction(
          rt, name, 3,
          [&b](jsi::Runtime&, const jsi::Value&, const jsi::Value* args,
               size_t) -> jsi::Value {
            b.on_resize(static_cast<float>(args[0].asNumber()),
                        static_cast<float>(args[1].asNumber()),
                        static_cast<float>(args[2].asNumber()));
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

    return jsi::Value::undefined();
  }

 private:
  rust::Box<JsHostBridge> bridge_;
};

}  // namespace

struct HermesApp::Impl {
  std::unique_ptr<jsi::Runtime> runtime;
  // バンドルの eval に成功し __tsubame が公開されたか。false の間は frame/resize を
  // 呼ばない（JS エラーで __tsubame 未定義のときに毎フレーム例外を投げないため）。
  bool ready = false;
};

HermesApp::HermesApp(rust::Box<JsHostBridge> host, rust::Str bundle)
    : impl_(std::make_unique<Impl>()) {
  impl_->runtime = facebook::hermes::makeHermesRuntime();
  jsi::Runtime& rt = *impl_->runtime;

  // __hayateHost を注入。
  auto host_obj = std::make_shared<HayateHostObject>(std::move(host));
  rt.global().setProperty(rt, "__hayateHost",
                          jsi::Object::createFromHostObject(rt, host_obj));

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

void HermesApp::resize(float width, float height, float scale) {
  if (!impl_->ready) return;
  jsi::Runtime& rt = *impl_->runtime;
  try {
    jsi::Object tsubame = rt.global().getPropertyAsObject(rt, "__tsubame");
    jsi::Function fn = tsubame.getPropertyAsFunction(rt, "resize");
    fn.callWithThis(rt, tsubame,
                    {jsi::Value(static_cast<double>(width)),
                     jsi::Value(static_cast<double>(height)),
                     jsi::Value(static_cast<double>(scale))});
  } catch (const jsi::JSError& e) {
    HAYATE_LOGE("resize で JS 例外: %s", e.getMessage().c_str());
  } catch (const std::exception& e) {
    HAYATE_LOGE("resize で例外: %s", e.what());
  }
}

std::unique_ptr<HermesApp> new_hermes_app(rust::Box<JsHostBridge> host,
                                          rust::Str bundle) {
  return std::make_unique<HermesApp>(std::move(host), bundle);
}

}  // namespace hayate
