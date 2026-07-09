import java.nio.file.Files

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

    // ── Play 内部テスト向けリリース署名 ─────────────────────────────────────────────
    // keystore とパスワードはリポジトリに絶対に置かない。CI は環境変数、ローカルは
    // ~/.gradle/gradle.properties か local.properties（= Gradle プロパティ）から読む。
    // 4 つすべて揃ったときだけ release 署名を構成し、未設定なら署名を付けない
    // （assembleDebug など従来のデバッグビルドは一切影響を受けない）。keystore 生成と
    // 各値の設定・署名 AAB ビルド・Play Console 初回アップロード手順は同ディレクトリの
    // RELEASE-SIGNING.md を参照。
    val releaseStoreFile = (project.findProperty("hayate.release.storeFile") as String?)
        ?: System.getenv("HAYATE_RELEASE_STORE_FILE")
    val releaseStorePassword = (project.findProperty("hayate.release.storePassword") as String?)
        ?: System.getenv("HAYATE_RELEASE_STORE_PASSWORD")
    val releaseKeyAlias = (project.findProperty("hayate.release.keyAlias") as String?)
        ?: System.getenv("HAYATE_RELEASE_KEY_ALIAS")
    val releaseKeyPassword = (project.findProperty("hayate.release.keyPassword") as String?)
        ?: System.getenv("HAYATE_RELEASE_KEY_PASSWORD")
    val hasReleaseSigning = listOf(
        releaseStoreFile, releaseStorePassword, releaseKeyAlias, releaseKeyPassword,
    ).all { !it.isNullOrBlank() }

    signingConfigs {
        if (hasReleaseSigning) {
            create("release") {
                storeFile = file(releaseStoreFile!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // 署名が構成されているときだけ release に紐付ける。未設定なら bundleRelease は
            // 未署名 AAB を作る（Play には出せないが、CI/ローカルの構成ミスを黙って壊れた
            // 署名で通すよりは、未署名で明示的に失敗させる方が安全）。
            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
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

    // Torimi: reload 購読 WS の OS スタック実装（ADR-0002 後半・#742）。HttpURLConnection と
    // 違い WebSocket はプラットフォーム標準 API に無いので、OkHttp を明示依存する（ADR-0002 の
    // 「Android は OkHttp 系」）。TLS（wss）は OS の信頼ストアから得る。
    implementation("com.squareup.okhttp3:okhttp:4.12.0")

    // Torimi: DevServerSetupActivity の「QR スキャン」で dev-server の LAN URL を読む。
    // Google Code Scanner は Play services のスキャナ UI を使うので、CameraX も独自カメラ権限も
    // 要らず、起動コマンドが端末に出した QR をそのまま読み取って URL 欄に入れられる。
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}

// rust-android-gradle は既定で `${module}/target`（= crates/.../android/target）から .so を
// 拾うが、このクレートはワークスペース共有の `Hayate/target` に出力するため、既定のままだと
// cargoBuild は「コピー対象なし」を黙って成功させ、.so 無しの起動即クラッシュ APK ができる。
// プラグインの解決順は local.properties(rust.cargoTargetDir) → env CARGO_TARGET_DIR →
// targetDirectory → 既定。env を設定する scripts/build-android.sh 経由でも矛盾しない。
val workspaceTargetDir = project.file("../../../../../../target").canonicalPath

// Build the `hayate-adapter-android` cdylib and fold it into the APK's jniLibs.
cargo {
    module = "../.."
    libname = "hayate_adapter_android"
    targets = listOf("arm64")
    profile = "release"
    targetDirectory = workspaceTargetDir
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

// cargoBuild が .so をコピーし損ねても Gradle は成功してしまうため、パッケージング前に
// 実物の存在を検証して欠落を即失敗にする（無言で壊れた APK を作らせない）。
val verifyRustJniLib = tasks.register("verifyRustJniLib") {
    dependsOn("cargoBuild")
    doLast {
        val copied = layout.buildDirectory
            .file("rustJniLibs/android/arm64-v8a/libhayate_adapter_android.so").get().asFile
        if (!copied.exists()) {
            throw GradleException(
                "libhayate_adapter_android.so が rustJniLibs にコピーされていません。" +
                    "このまま進むと起動直後に UnsatisfiedLink で落ちる APK ができます。" +
                    "cargo の出力先（rust.cargoTargetDir / CARGO_TARGET_DIR / cargo.targetDirectory）" +
                    "と実際の出力（$workspaceTargetDir）が一致しているか確認してください。"
            )
        }
        val built = File(
            System.getenv("CARGO_TARGET_DIR") ?: workspaceTargetDir,
            "aarch64-linux-android/release/libhayate_adapter_android.so"
        )
        if (built.exists() &&
            Files.mismatch(copied.toPath(), built.toPath()) != -1L
        ) {
            throw GradleException(
                "rustJniLibs の libhayate_adapter_android.so が cargo の最新出力（$built）と" +
                    "一致しません。古い .so が APK に入るのを防ぐため失敗させます。"
            )
        }
    }
}

tasks.matching { it.name.matches(Regex("merge.*JniLibFolders")) }.configureEach {
    dependsOn("cargoBuild", verifyRustJniLib)
    // AGP 8 では plugin が後付けする rustJniLibs srcDir が merge タスクの入力追跡に
    // 乗らず、.so が更新されても UP-TO-DATE 扱いで古い/空の APK ができる。
    // 明示的に入力登録して、.so の変化で必ず再マージさせる。
    inputs.dir(layout.buildDirectory.dir("rustJniLibs/android"))
}

// Tsubame バンドル（tsubame.js）は src/main/assets/ にコミット済み（ADR-0112）。AGP が
// そのまま APK に同梱する。以前は Gradle から pnpm で毎回生成していたが、Gradle
// デーモンの環境に pnpm/node が無いと失敗する（CreateProcess error=2 や exit 1）ため、
// ビルド時の Node 依存を排除した。JS を変更したら手動で再生成して差し替える:
//   cd Tsubame && pnpm --filter @tsubame/example-todo run torimi:native:build
//   cp examples/todo/dist-android/tsubame.js \
//      ../Hayate/crates/platform/mobile/android/android-app/app/src/main/assets/tsubame.js
