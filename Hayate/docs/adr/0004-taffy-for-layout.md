# レイアウトエンジンに Taffy を採用する

仕様書は Yoga（Meta, C++）の利用を想定していたが、コア言語を Rust に変更したため Taffy（Pure Rust）を採用する。Taffy は Flexbox + CSS Grid + Block layout を cargo 一行で追加でき、C++ ビルドチェーンの混入がない。Bevy / Dioxus / Slint / Zed 等の Rust UI エコシステムで実績があり、Yoga と同等のレイアウト品質を持つ。
