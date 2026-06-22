import UIKit
import QuartzCore

/// The native host view for `hayate-adapter-ios` (ADR-0113).
///
/// Owns the `CAMetalLayer` (wgpu's Metal surface), a `CADisplayLink` (the vsync frame
/// loop), and forwards UIScene lifecycle, `UITouch`, and keyboard input to the Rust
/// staticlib via the `hayate_ios_*` C FFI. All decode/diff/apply logic lives in Rust's
/// host-tested seams (`surface_lifecycle` / `touch_input` / `ime_input`); this view is
/// the thin objc glue, the iOS analogue of Android's `app.rs` platform plumbing.
///
/// `becomeFirstResponder` / `resignFirstResponder` (the soft-keyboard control) are driven
/// only by Rust through `hayate_ios_set_keyboard_visible` below — never decided here — so
/// the editability gate stays in core (`ElementTree::drive_ime`), matching the Android
/// `ime_bridge` encapsulation guard.
final class HayateView: UIView {
    /// Rust が `hayate_ios_set_keyboard_visible` から到達するための現在のホストビュー。
    static weak var current: HayateView?

    private var app: OpaquePointer?
    private var displayLink: CADisplayLink?
    private var startTime = CACurrentMediaTime()

    override class var layerClass: AnyClass { CAMetalLayer.self }
    private var metalLayer: CAMetalLayer { layer as! CAMetalLayer }

    override init(frame: CGRect) {
        super.init(frame: frame)
        HayateView.current = self
        contentScaleFactor = UIScreen.main.scale
        metalLayer.contentsScale = UIScreen.main.scale
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    // MARK: - Surface lifecycle (SceneDelegate が論理イベントへ畳む)

    override func layoutSubviews() {
        super.layoutSubviews()
        let scale = Float(contentScaleFactor)
        let pxW = Int32(bounds.width * contentScaleFactor)
        let pxH = Int32(bounds.height * contentScaleFactor)
        metalLayer.drawableSize = CGSize(width: Int(pxW), height: Int(pxH))
        if app == nil {
            // 初回 sized layer = InitWindow（CreateSurface）。Swift が CAMetalLayer を作り、
            // raw-window-metal でそこから wgpu Metal サーフェスを張る。
            let layerPtr = Unmanaged.passUnretained(metalLayer).toOpaque()
            app = hayate_ios_app_new(layerPtr, scale)
            startDisplayLink()
        } else {
            // 以降の layout = WindowResized（ResizeSurface）。
            hayate_ios_resize(app, pxW, pxH, scale)
        }
    }

    func onBecomeActive() { startDisplayLink() }

    func onResignActive() {
        // TerminateWindow: ドローアブルが背景で無効になるためループを止める。
        stopDisplayLink()
    }

    func onDisconnect() {
        // Destroy: Rust アプリ状態を解放する。
        stopDisplayLink()
        if let app { hayate_ios_app_free(app) }
        app = nil
    }

    private func startDisplayLink() {
        guard displayLink == nil else { return }
        let link = CADisplayLink(target: self, selector: #selector(frame(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    private func stopDisplayLink() {
        displayLink?.invalidate()
        displayLink = nil
    }

    @objc private func frame(_ link: CADisplayLink) {
        guard let app else { return }
        let timestampMs = (CACurrentMediaTime() - startTime) * 1000.0
        hayate_ios_render(app, timestampMs)
    }

    // MARK: - Touch (UITouch.phase → Rust translate_touch)

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, 0) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, 1) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, 2) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, 3) }

    /// phase: 0=Down 1=Move 2=Up 3=Cancel（Rust `TouchAction` と対応）。座標は points。
    private func forward(_ touches: Set<UITouch>, _ phase: Int32) {
        guard let app, let touch = touches.first else { return }
        let p = touch.location(in: self)
        hayate_ios_touch(app, phase, Float(p.x), Float(p.y))
    }

    // MARK: - Keyboard input (UIKeyInput → Rust ime_input commands)
    //
    // 基本のコミット入力（insertText / deleteBackward）を UIKeyInput で配線する。変換中の
    // marked text（preedit）に必要な完全な UITextInput 準拠（position/range geometry 群）は
    // groundwork では defer する（Rust `ime_input` は SetMarked/Unmark コマンドを既に持ち
    // ホストテスト済み。Swift 側の UITextInput 準拠が次段階）。

    override var canBecomeFirstResponder: Bool { true }

    /// IME command kind: 0=Insert 1=DeleteBackward 2=SetMarked 3=Unmark（Rust `ImeCommand`）。
    private func sendIme(_ kind: Int32, _ text: String?) {
        guard let app else { return }
        if let text {
            text.withCString { hayate_ios_ime(app, kind, $0) }
        } else {
            hayate_ios_ime(app, kind, nil)
        }
    }
}

extension HayateView: UIKeyInput {
    var hasText: Bool { true }
    func insertText(_ text: String) { sendIme(0, text) }
    func deleteBackward() { sendIme(1, nil) }
}

/// Rust の `IosImeBridge`（`ime_bridge.rs`）だけが呼ぶソフトキーボード制御。core が
/// `ElementTree::drive_ime` で編集可否を一度決め、Rust ブリッジがここを叩く。Swift では
/// first responder の取得/解放に写す。アプリ内の他所はこの FFI を呼ばない（Rust 側は
/// `tests/ime_api_encapsulation.rs` が `ime_bridge.rs` 限定を強制する）。
@_cdecl("hayate_ios_set_keyboard_visible")
func hayate_ios_set_keyboard_visible(_ visible: Bool) {
    guard let view = HayateView.current else { return }
    if visible {
        _ = view.becomeFirstResponder()
    } else {
        _ = view.resignFirstResponder()
    }
}
