import UIKit

/// Thin UIApplicationDelegate host for the `hayate-adapter-ios` demo (ADR-0115).
///
/// All application logic lives in Rust: the `hayate_adapter_ios` staticlib is linked
/// into this app binary, and the `HayateView` drives the frame loop / touch / IME via
/// the `hayate_ios_*` C FFI. UIKit is used over a pure-objc2 entry solely so UITextInput
/// protocol conformance stays ergonomic in Swift — there is deliberately no app
/// behaviour here. This mirrors Android's thin `class MainActivity : GameActivity()`.
@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Rust 側の一度きりの起動フック（ロガー初期化等）。Android の `android_main`
        // 冒頭の `android_logger::init_once` に対応する名前付きエントリ。
        ios_main()
        return true
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default Configuration", sessionRole: connectingSceneSession.role)
    }
}
