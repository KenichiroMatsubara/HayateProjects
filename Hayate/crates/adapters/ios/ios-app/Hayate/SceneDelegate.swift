import UIKit

/// Thin UIWindowSceneDelegate host (ADR-0114 / ADR-0115).
///
/// Folds the UIScene lifecycle into the four logical surface-lifecycle events the Rust
/// `surface_lifecycle` state machine consumes (InitWindow / TerminateWindow /
/// WindowResized / Destroy). The `HayateView` owns the CAMetalLayer + CADisplayLink and
/// forwards everything to Rust; this delegate just installs it and relays lifecycle. App
/// logic stays in Rust.
class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    var hayateView: HayateView?

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: windowScene)
        let view = HayateView(frame: windowScene.coordinateSpace.bounds)
        let controller = UIViewController()
        controller.view = view
        window.rootViewController = controller
        self.window = window
        self.hayateView = view
        window.makeKeyAndVisible()
        // 初回 sized layer は HayateView.layoutSubviews が InitWindow として Rust に上げる。
    }

    func sceneWillResignActive(_ scene: UIScene) {
        // 背景化前に Metal ドローアブルが無効になるため、サーフェスを破棄する（TerminateWindow）。
        hayateView?.onResignActive()
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        hayateView?.onBecomeActive()
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        // Destroy: Rust 側のアプリ状態を解放する。
        hayateView?.onDisconnect()
    }
}
