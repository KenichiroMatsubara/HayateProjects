plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.mozilla.rust-android-gradle.rust-android")
}

android {
    namespace = "com.hayateprojects.hayate.adapter_android_demo"
    compileSdk = 34
    // 既定はこれまで動作実績のあるバージョン。マシンによって異なる場合は
    // Gradle プロパティ `hayate.ndkVersion` か環境変数 `HAYATE_NDK_VERSION` で
    // 上書きできる（ADR-0112）。未指定でもこの既定で従来どおりビルドできる。
    ndkVersion = (project.findProperty("hayate.ndkVersion") as String?
        ?: System.getenv("HAYATE_NDK_VERSION")
        ?: "30.0.14904198")

    defaultConfig {
        applicationId = "com.hayateprojects.hayate.adapter_android_demo"
        // GameActivity / GameTextInput supported floor (ADR-0094).
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
        // wgpu uses Vulkan on Android; ship arm64-v8a only for now.
        ndk { abiFilters += "arm64-v8a" }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // GameActivity + GameTextInput: the soft-keyboard InputConnection path that
    // android-activity's game-activity backend reads for the stage C IME bridge.
    implementation("androidx.games:games-activity:3.0.5")
    // GameActivity extends AppCompatActivity and implements OnApplyWindowInsetsListener;
    // games-activity does not bring these transitively, so declare them explicitly.
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.core:core:1.13.1")
    // embedded Hermes（ADR-0112）は実行時に libhermesvm/libjsi/libfbjni/libc++_shared を
    // 要する。libjsi は react-android AAR にしか無く、依存すると不要な libreactnative.so で
    // APK が肥大化するため react-android には依存せず、libhermesvm/libjsi だけを
    // src/main/jniLibs/arm64-v8a に vendor 済み（ADR-0007）。
    //
    // libfbjni は JNI_OnLoad で Java クラス com.facebook.jni.*（HybridData$Destructor 等）を
    // 要求するので .so だけでは ClassNotFoundException で落ちる。これらは fbjni AAR が
    // .so + Java クラス + libc++_shared をまとめて供給する。fbjni は React 本体ではない
    // 汎用 JNI ヘルパ（バージョンは react-android 0.82.1 が使う 0.7.0 に一致）。
    if (!project.hasProperty("nativedemo")) {
        implementation("com.facebook.fbjni:fbjni:0.7.0")
    }

    // Miharashi: DevServerSetupActivity の「QR スキャン」で dev-server の LAN URL を読む。
    // Google Code Scanner は Play services のスキャナ UI を使うので、CameraX も独自カメラ権限も
    // 要らず、起動コマンドが端末に出した QR をそのまま読み取って URL 欄に入れられる。
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}

// Build the `hayate-adapter-android` cdylib and fold it into the APK's jniLibs.
cargo {
    module = "../.."
    libname = "hayate_adapter_android"
    targets = listOf("arm64")
    profile = "release"
    // 既定は Tsubame Todo（tsubame-js は Cargo.toml の default feature, ADR-0112）。
    // Hayate 単体のネイティブデモ（build_demo_tree）を見たいときだけ `-Pnativedemo` で
    // default features を外す。
    if (project.hasProperty("nativedemo")) {
        // default features を外す（= cargo の --no-default-features）。空配列で追加なし。
        featureSpec.noDefaultBut(arrayOf<String>())
    }
    // Hermes/JSI のヘッダ・.so は vendor 済みで、build.rs が CARGO_MANIFEST_DIR 相対で
    // 自動解決する（third_party/include と src/main/jniLibs/arm64-v8a）。別バージョンを
    // 検証したいときだけ `-Phermes.include` / `-Phermes.lib`（または同名 env）で上書く。
    val hermesInclude = (project.findProperty("hermes.include") as String?)
        ?: System.getenv("HERMES_INCLUDE")
    val hermesLib = (project.findProperty("hermes.lib") as String?)
        ?: System.getenv("HERMES_LIB")
    if (hermesInclude != null || hermesLib != null) {
        exec = { spec, _ ->
            hermesInclude?.let { spec.environment("HERMES_INCLUDE", it) }
            hermesLib?.let { spec.environment("HERMES_LIB", it) }
        }
    }
}

tasks.matching { it.name.matches(Regex("merge.*JniLibFolders")) }.configureEach {
    dependsOn("cargoBuild")
}

// Tsubame バンドル（tsubame.js）は src/main/assets/ にコミット済み（ADR-0112）。AGP が
// そのまま APK に同梱する。以前は Gradle から pnpm で毎回生成していたが、Gradle
// デーモンの環境に pnpm/node が無いと失敗する（CreateProcess error=2 や exit 1）ため、
// ビルド時の Node 依存を排除した。JS を変更したら手動で再生成して差し替える:
//   cd Tsubame && pnpm --filter @tsubame/example-todo run build:android
//   cp examples/todo/dist-android/tsubame.js \
//      ../Hayate/crates/platform/mobile/android/android-app/app/src/main/assets/tsubame.js
