// Root build script: declares plugin versions; modules apply them (ADR-0094).
// Versions are validated on a local machine with the Android SDK/NDK + Gradle —
// the Rust sandbox cannot run Gradle (see ADR-0087 / issue #195).
plugins {
    id("com.android.application") version "8.13.2" apply false
    // AGP 8.13.2 がサポートする Kotlin 2.3 系の最新パッチ。
    id("org.jetbrains.kotlin.android") version "2.3.21" apply false
    id("org.mozilla.rust-android-gradle.rust-android") version "0.9.6" apply false
}
