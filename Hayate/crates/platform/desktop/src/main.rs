//! Desktop demo binary — native window に共有 demo fixture を静的 1 枚で present する（ADR-0118）。

fn main() {
    if let Err(e) = hayate_platform_desktop::run() {
        eprintln!("hayate-desktop exited with error: {e}");
        std::process::exit(1);
    }
}
