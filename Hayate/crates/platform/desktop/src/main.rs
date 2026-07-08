//! Desktop demo binary — native window に共有 demo fixture を静的 1 枚で present する（ADR-0118）。

fn main() {
    // RUST_LOG（run-desktop.mjs の既定は info）を効かせる。pipeline cache の hit/miss や
    // vello init 時間（issue #777）を含む起動ログが stderr に出る。
    env_logger::init();
    if let Err(e) = hayate_platform_desktop::run() {
        eprintln!("hayate-desktop exited with error: {e}");
        std::process::exit(1);
    }
}
