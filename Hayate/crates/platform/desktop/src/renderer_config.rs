//! desktop のレンダラ強制指定（env / CLI フラグ）の純粋シーム（issue #801、spec §4
//! REND-15、ADR-0146 §5）。
//!
//! ADR-0138/0140/0145 の「常時コンパイル＋ランタイムフラグ」流儀 — vello と skia は
//! 両方リンクされ、再ビルドなしにここで切り替える。Android の intent extra
//! （`hayate.renderer`、issue #802）と対になる desktop 側の口。値の解釈は OS にも
//! winit にも触れない純関数で、プロセス環境を偽装せずテストできる（ADR-0145 の
//! `render_config` と同じ着地パターン）。
//!
//! 未知値・リンクされていないレンダラの強制は既定（vello → skia の一方向 fallback、
//! [`hayate_app_host::renderer_selection::NATIVE_RENDERER_ORDER`]）へ落とす。

use hayate_app_host::renderer_selection::SceneRendererKind;

/// レンダラ強制指定の環境変数キー（例: `HAYATE_RENDERER=skia`）。
pub const RENDERER_ENV_VAR: &str = "HAYATE_RENDERER";

/// レンダラ強制指定の CLI フラグ（`--renderer=skia` / `--renderer skia` の両形式）。
/// CLI は env より優先する（より明示的な指定が勝つ）。
pub const RENDERER_CLI_FLAG: &str = "--renderer";

/// 強制指定の値語彙。[`SceneRendererKind::name`] と同一の安定 ID。
pub const RENDERER_VALUE_VELLO: &str = "vello";
pub const RENDERER_VALUE_SKIA: &str = "skia";

/// このビルドが vello/wgpu をリンクしているか（`backend-vello` feature、default on）。
pub const VELLO_LINKED: bool = cfg!(feature = "backend-vello");

/// 強制指定値を解釈する。ネイティブで選べるのは vello / skia のみで、未知値は
/// None（= 既定の selection policy）へ落とす（ADR-0145 の「未知値は既定へ」流儀）。
pub fn parse_renderer_name(value: &str) -> Option<SceneRendererKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        RENDERER_VALUE_VELLO => Some(SceneRendererKind::Vello),
        RENDERER_VALUE_SKIA => Some(SceneRendererKind::Skia),
        _ => None,
    }
}

/// CLI 引数（バイナリ名を除いた `args`）と env 値からレンダラ強制指定を解決する。
/// CLI が env より優先。`--renderer=skia` と `--renderer skia` の両形式を受ける。
pub fn forced_renderer<I>(cli_args: I, env_value: Option<&str>) -> Option<SceneRendererKind>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    if let Some(value) = cli_renderer_value(cli_args) {
        if let Some(kind) = parse_renderer_name(&value) {
            return Some(kind);
        }
    }
    env_value.and_then(parse_renderer_name)
}

/// CLI 引数列から `--renderer` の値部分を抜き出す（`=` 形式と後続引数形式）。
fn cli_renderer_value<I>(cli_args: I) -> Option<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut args = cli_args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        if arg == RENDERER_CLI_FLAG {
            return args.next().map(|v| v.as_ref().to_string());
        }
        if let Some(value) = arg.strip_prefix(RENDERER_CLI_FLAG) {
            if let Some(value) = value.strip_prefix('=') {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_keys_and_default_values_are_named_constants() {
        // issue #801 受け入れ条件: 既定値と切替キー名が名前付き定数であること。
        assert_eq!(RENDERER_ENV_VAR, "HAYATE_RENDERER");
        assert_eq!(RENDERER_CLI_FLAG, "--renderer");
        assert_eq!(RENDERER_VALUE_VELLO, SceneRendererKind::Vello.name());
        assert_eq!(RENDERER_VALUE_SKIA, SceneRendererKind::Skia.name());
    }

    #[test]
    fn env_value_forces_the_renderer() {
        let none: [&str; 0] = [];
        assert_eq!(
            forced_renderer(none, Some("skia")),
            Some(SceneRendererKind::Skia)
        );
        assert_eq!(
            forced_renderer(none, Some("vello")),
            Some(SceneRendererKind::Vello)
        );
    }

    #[test]
    fn unknown_or_absent_values_fall_to_the_default_policy() {
        let none: [&str; 0] = [];
        assert_eq!(forced_renderer(none, None), None);
        assert_eq!(forced_renderer(none, Some("")), None);
        assert_eq!(forced_renderer(none, Some("dawn")), None);
        assert_eq!(forced_renderer(["--verbose"], None), None);
    }

    #[test]
    fn cli_flag_supports_equals_and_separate_arg_forms() {
        assert_eq!(
            forced_renderer(["--renderer=skia"], None),
            Some(SceneRendererKind::Skia)
        );
        assert_eq!(
            forced_renderer(["--renderer", "skia"], None),
            Some(SceneRendererKind::Skia)
        );
    }

    #[test]
    fn cli_flag_wins_over_env() {
        // より明示的な指定（そのプロセス起動専用の CLI）が env より優先。
        assert_eq!(
            forced_renderer(["--renderer=vello"], Some("skia")),
            Some(SceneRendererKind::Vello)
        );
    }

    #[test]
    fn values_are_trimmed_and_case_insensitive() {
        let none: [&str; 0] = [];
        assert_eq!(
            forced_renderer(none, Some(" Skia ")),
            Some(SceneRendererKind::Skia)
        );
        assert_eq!(
            forced_renderer(["--renderer=VELLO"], None),
            Some(SceneRendererKind::Vello)
        );
    }
}
