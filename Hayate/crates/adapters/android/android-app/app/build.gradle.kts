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
    // Tsubame JS 駆動（ADR-0112）を `-Ptsubame.enabled=true` で有効化したときだけ
    // 埋め込み Hermes を取り込む。libhermes.so を APK の jniLibs に入れ、cdylib が
    // JSI でリンクする。ヘッダ（<jsi/jsi.h> / <hermes/hermes.h>）は build.rs に
    // HERMES_INCLUDE で渡す（cargo ブロック参照）。standalone 埋め込みは device
    // 未検証 — バージョン/座標は環境で要調整。既定（未指定）は従来のネイティブ
    // デモ経路でビルドし、この依存も C++ コンパイルも発生しない。
    if (project.hasProperty("tsubame.enabled")) {
        implementation("com.facebook.react:hermes-android:0.76.0")
    }
}

// Build the `hayate-adapter-android` cdylib and fold it into the APK's jniLibs.
cargo {
    module = "../.."
    libname = "hayate_adapter_android"
    targets = listOf("arm64")
    profile = "release"
    // `-Ptsubame.enabled=true` のときだけ Tsubame JS 駆動経路（ADR-0112）を有効化する。
    // 既定（未指定）は OFF: feature が立たないので build.rs も C++/Hermes を
    // コンパイルせず、従来どおりネイティブデモがビルドできる。
    if (project.hasProperty("tsubame.enabled")) {
        featureSpec.defaultAnd(arrayOf("tsubame-js"))
        // build.rs に Hermes/JSI ヘッダ探索パスを渡す（device 環境で設定）。
        System.getenv("HERMES_INCLUDE")?.let { hermesInc ->
            exec = { spec, _ -> spec.environment("HERMES_INCLUDE", hermesInc) }
        }
    }
}

tasks.matching { it.name.matches(Regex("merge.*JniLibFolders")) }.configureEach {
    dependsOn("cargoBuild")
}

// Tsubame バンドル（tsubame.js）の生成＋assets 同梱は opt-in（-Ptsubame.enabled=true）。
// 既定では登録しないので、pnpm 不在の環境でも従来どおりビルドできる（ADR-0112）。
if (project.hasProperty("tsubame.enabled")) {
    // パスは Gradle プロパティ `tsubame.dir` で上書き可能（既定はリポジトリ相対）。
    val tsubameDir = (project.findProperty("tsubame.dir") as String?)
        ?.let { file(it) }
        ?: rootProject.file("../../../../../Tsubame")

    val bundleTsubameJs by tasks.registering(Exec::class) {
        workingDir = tsubameDir
        commandLine("pnpm", "--filter", "@tsubame/example-todo", "run", "build:android")
    }

    val copyTsubameBundle by tasks.registering(Copy::class) {
        dependsOn(bundleTsubameJs)
        from(tsubameDir.resolve("examples/todo/dist-android/tsubame.js"))
        into(layout.projectDirectory.dir("src/main/assets"))
    }

    tasks.matching { it.name.matches(Regex("merge.*Assets")) }.configureEach {
        dependsOn(copyTsubameBundle)
    }
}
