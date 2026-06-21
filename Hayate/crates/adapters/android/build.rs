//! Android（device）かつ `tsubame-js` feature 有効時のみ、cxx の C++ ブリッジ
//! （JSI/Hermes ホスト）をコンパイル & リンクする（ADR-0112）。
//!
//! ホスト（x86_64/wasm 以外）の `cargo check` ではこの分岐に入らないため、
//! libhermes / NDK が無くてもビルドが通り、純 Rust の検証が回り続ける。
//!
//! Hermes/JSI のヘッダと .so はリポジトリに vendor 済み（ADR-0007 の vendored
//! dependencies 方針）。react-android には依存せず、リンクに要る libhermesvm / libjsi
//! だけを jniLibs に置き、JSI/Hermes ヘッダを third_party/include に置く。libfbjni /
//! libc++_shared と fbjni の Java クラスは Gradle 依存 com.facebook.fbjni:fbjni が
//! 供給する（リンク時は cdylib の未定義シンボルとして実行時解決に回るので不要）。
//! `HERMES_INCLUDE` / `HERMES_LIB` を env で与えればそちらを優先する（別バージョン検証用）。
use std::env;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let tsubame_js = env::var("CARGO_FEATURE_TSUBAME_JS").is_ok();

    if target_os != "android" || !tsubame_js {
        return;
    }

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // vendor 既定パス（env 未指定時）。
    let vendored_include = manifest.join("third_party/include");
    let vendored_lib = manifest.join("android-app/app/src/main/jniLibs/arm64-v8a");

    // cxx ブリッジ（src/hermes_bridge.rs）と C++ 実装（cpp/hermes_app.cpp）を
    // 一緒にコンパイルする。JSI/Hermes ヘッダ探索パスを include に足す。
    let mut build = cxx_build::bridge("src/hermes_bridge.rs");
    build.file("cpp/hermes_app.cpp");
    build.std("c++17");

    match env::var("HERMES_INCLUDE") {
        Ok(inc) => {
            for path in env::split_paths(&inc) {
                build.include(path);
            }
        }
        Err(_) => {
            build.include(&vendored_include);
        }
    }

    build.compile("hayate_hermes_bridge");

    // libjsi.so / libhermesvm.so を cdylib にリンクする。探索パスは vendor 済み
    // jniLibs（env HERMES_LIB で上書き可）。libfbjni / libc++_shared は libhermesvm の
    // NEEDED として実行時に解決されるが、リンカが探索できるよう同じ jniLibs に置く。
    let lib_paths: Vec<PathBuf> = match env::var("HERMES_LIB") {
        Ok(libs) => env::split_paths(&libs).collect(),
        Err(_) => vec![vendored_lib],
    };
    for path in &lib_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    println!("cargo:rustc-link-lib=dylib=jsi");
    println!("cargo:rustc-link-lib=dylib=hermesvm");

    println!("cargo:rerun-if-changed=src/hermes_bridge.rs");
    println!("cargo:rerun-if-changed=cpp/hermes_app.cpp");
    println!("cargo:rerun-if-changed=cpp/hermes_app.h");
    println!("cargo:rerun-if-env-changed=HERMES_INCLUDE");
    println!("cargo:rerun-if-env-changed=HERMES_LIB");
}
