//! Android（device）かつ `tsubame-js` feature 有効時のみ、cxx の C++ ブリッジ
//! （JSI/Hermes ホスト）をコンパイルする（ADR-0112）。
//!
//! ホスト（x86_64/wasm 以外）の `cargo check` ではこの分岐に入らないため、
//! libhermes / NDK が無くてもビルドが通り、純 Rust の検証が回り続ける。
//! device ビルド（Gradle + rust-android-gradle + NDK）では libhermes を
//! リンクする必要がある（Gradle 側で jniLibs / linker 設定を行う）。
use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let tsubame_js = env::var("CARGO_FEATURE_TSUBAME_JS").is_ok();

    if target_os != "android" || !tsubame_js {
        return;
    }

    // cxx ブリッジ（src/hermes_bridge.rs）と C++ 実装（cpp/hermes_app.cpp）を
    // 一緒にコンパイルする。ヘッダ探索パスは Hermes/JSI を含む必要があり、
    // Gradle/NDK 側から HERMES_INCLUDE 等で渡す想定（device 未検証）。
    let mut build = cxx_build::bridge("src/hermes_bridge.rs");
    build.file("cpp/hermes_app.cpp");
    build.std("c++17");

    if let Ok(hermes_include) = env::var("HERMES_INCLUDE") {
        for path in env::split_paths(&hermes_include) {
            build.include(path);
        }
    }

    build.compile("hayate_hermes_bridge");

    // libjsi.so / libhermesvm.so を cdylib にリンクする。探索パスは AAR（prefab/jni）
    // 由来で、`HERMES_LIB`（複数可, OS のパス区切り）で渡す。Gradle 側がこの env を
    // 設定する（hermes-android / react-android の prefab/jni ディレクトリ）。
    if let Ok(hermes_lib) = env::var("HERMES_LIB") {
        for path in env::split_paths(&hermes_lib) {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        println!("cargo:rustc-link-lib=dylib=jsi");
        println!("cargo:rustc-link-lib=dylib=hermesvm");
    }

    println!("cargo:rerun-if-changed=src/hermes_bridge.rs");
    println!("cargo:rerun-if-changed=cpp/hermes_app.cpp");
    println!("cargo:rerun-if-changed=cpp/hermes_app.h");
    println!("cargo:rerun-if-env-changed=HERMES_INCLUDE");
    println!("cargo:rerun-if-env-changed=HERMES_LIB");
}
