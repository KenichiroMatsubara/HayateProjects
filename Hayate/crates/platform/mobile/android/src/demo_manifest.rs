//! Torimi の Android ホストが消費する **Demo Manifest**（`/demos.json`・ADR-0003, #743）。
//!
//! 公開 Demo Endpoint（Cloudflare Worker・#738）は、テスター・審査者向けにデモ一覧
//! （各エントリ＝表示名 + バンドル URL）を `/demos.json` で配る。ホストは起動時にこれを取得して
//! デモ選択メニューを構成し、**初回起動（接続先未設定）は先頭デモを自動ロード**する（ゼロ入力で
//! デモが動く）。デモの追加・改名はマニフェスト更新であり、ホストの Play 審査を要しない
//! （Torimi CONTEXT.md「Demo Manifest」）。
//!
//! ここは **プラットフォーム非依存のピュアなシーム**（wire JSON のパース・エントリ選択 → boot
//! target 解決・取得失敗の明示エラー）なのでホストで契約テストする（`dev_server_target` /
//! `protocol_handshake` と同じ流儀）。実 UI（右上メニュー）と実機 fetch/boot はローカル実機で
//! 検証する（本 issue 外・ADR-0001）。
//!
//! wire 型の正本は `@torimi/wire-contract`の language-neutral schema。そこから生成した
//! Rust DTO / serde 実装で JSON の required field と型を検証し、その後に handwritten の
//! [`BootPlan`] へ変換する。ホストにとってバンドル URL は**不透明**で、中身の
//! フレームワーク知識は持ち込まない（ADR-0001 / ADR-0003 / ADR-0006）。

use crate::dev_server_target::{self, DevServerTarget};
use crate::generated::torimi_wire::{
    DemoManifest as WireDemoManifest, DemoManifestEntry as WireDemoManifestEntry,
    DEMO_MANIFEST_ROUTE,
};

/// Demo Manifest の 1 エントリ。TS `DemoManifestEntry`（表示名 + バンドル URL）の Rust ミラー。
/// バンドル URL は Demo Endpoint origin からの相対パス可（`/solid/bundle.js`）。ホストにとって
/// バンドルは不透明（FW 知識を持ち込まない・ADR-0003）。`bundleUrl` 以外の wire フィールド
/// （`$comment` / `source` 等の正本 metadata）は serde が既定で無視する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoEntry {
    /// デモ選択メニューに出す表示名。
    pub name: String,
    /// App Bundle の URL（Demo Endpoint origin からの相対パス可）。
    pub bundle_url: String,
}

/// Demo Endpoint が配信するデモ一覧。ホストはこれでメニューを構成し、初回起動は先頭エントリを
/// 自動ロードする（ADR-0003）。TS `DemoManifest` の Rust ミラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoManifest {
    /// 配信順のデモエントリ。先頭が初回自動ロード対象（ADR-0003）。
    pub demos: Vec<DemoEntry>,
}

impl From<WireDemoManifest> for DemoManifest {
    fn from(wire: WireDemoManifest) -> Self {
        Self {
            demos: wire.demos.into_iter().map(DemoEntry::from).collect(),
        }
    }
}

impl From<WireDemoManifestEntry> for DemoEntry {
    fn from(wire: WireDemoManifestEntry) -> Self {
        Self {
            name: wire.name,
            bundle_url: wire.bundle_url,
        }
    }
}

impl DemoManifest {
    /// 初回起動で自動ロードするデモ（先頭エントリ・ADR-0003）。空マニフェストなら `None`。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn first(&self) -> Option<&DemoEntry> {
        self.demos.first()
    }

    /// メニュー表示用の全エントリ（配信順を保つ）。デモ選択メニューは Kotlin ランチャ側が持つため
    /// （#743・ADR-0001）device の Rust では未使用だが、seam の一部として契約テストが固定する。
    #[allow(dead_code)]
    pub fn entries(&self) -> &[DemoEntry] {
        &self.demos
    }
}

/// Demo Manifest まわりの失敗。**謎クラッシュにせず**明示エラーにし、既存の URL 入力経路へ誘導する
/// （Protocol Version 突き合わせと同じ姿勢・ADR-0003 / #743）。[`message`](DemoManifestError::message)
/// が画面（`error_overlay`）に出す読める文言を作る。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum DemoManifestError {
    /// Demo Endpoint への `/demos.json` 取得自体が失敗した（オフライン・非 200・TLS 等。OS スタックの
    /// 例外文言を運ぶ）。ADR-0003：謎クラッシュにせず明示エラーにして URL 入力経路へ誘導する。
    Fetch(String),
    /// JSON syntax / required field / type mismatch at the generated serde boundary.
    Wire(WireManifestError),
    /// Schema-valid DTO that cannot form a meaningful boot plan.
    Boot(BootError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoManifestFailureCategory {
    Fetch,
    WireValidation,
    BootDomain,
}

impl DemoManifestFailureCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::WireValidation => "wire-validation",
            Self::BootDomain => "boot-domain",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireManifestError {
    InvalidJson(String),
    InvalidShape(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootError {
    EmptyManifest,
    UnselectableEntry { index: usize },
    UnresolvableEntry(String),
}

impl DemoManifestError {
    pub fn category(&self) -> DemoManifestFailureCategory {
        match self {
            DemoManifestError::Fetch(_) => DemoManifestFailureCategory::Fetch,
            DemoManifestError::Wire(_) => DemoManifestFailureCategory::WireValidation,
            DemoManifestError::Boot(_) => DemoManifestFailureCategory::BootDomain,
        }
    }

    /// 画面・ログ向けの明示メッセージ。取得/解釈に失敗した旨と、**既存の URL 入力経路へ誘導**する
    /// 案内を含む（ADR-0003：オフライン等でも謎クラッシュにしない）。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn message(&self) -> String {
        let detail = match self {
            DemoManifestError::Fetch(why) => {
                format!("デモ一覧（{DEMO_MANIFEST_ROUTE}）を取得できませんでした（{why}）")
            }
            DemoManifestError::Wire(error) => match error {
                WireManifestError::InvalidJson(why) | WireManifestError::InvalidShape(why) => {
                    format!("デモ一覧（{DEMO_MANIFEST_ROUTE}）の wire 検証に失敗しました（{why}）")
                }
            },
            DemoManifestError::Boot(error) => match error {
                BootError::EmptyManifest => {
                    format!("デモ一覧（{DEMO_MANIFEST_ROUTE}）にデモがありません")
                }
                BootError::UnselectableEntry { index } => {
                    format!("デモ一覧の {index} 番目を起動対象として選択できません")
                }
                BootError::UnresolvableEntry(url) => {
                    format!("デモのバンドル URL を解決できませんでした（{url:?}）")
                }
            },
        };
        format!("Torimi: {detail}。URL 入力画面から接続先を指定してください。")
    }
}

/// `/demos.json` を生成 serde DTO に読む。JSON syntax と schema shape の失敗は
/// [`WireManifestError`] として、後段の [`BootError`] と分離する。追加フィールドは serde の
/// 既定により無視される。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn decode_wire(json: &str) -> Result<WireDemoManifest, WireManifestError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| WireManifestError::InvalidJson(error.to_string()))?;
    serde_json::from_value(value)
        .map_err(|error| WireManifestError::InvalidShape(error.to_string()))
}

pub fn parse(json: &str) -> Result<DemoManifest, DemoManifestError> {
    decode_wire(json)
        .map(DemoManifest::from)
        .map_err(DemoManifestError::Wire)
}

/// Demo Endpoint の Demo Manifest を取りに行くフル URL。マニフェストは常に **origin 直下**
/// （[`DEMO_MANIFEST_ROUTE`]）にあり、base target の path（あるデモを指していても）は使わない。
/// scheme 既定ポートの正規化は `dev_server_target::parse` が済ませている（#740）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn manifest_url(base: &DevServerTarget) -> String {
    origin_url(base, DEMO_MANIFEST_ROUTE)
}

/// 選択したエントリ → boot target の解決（ADR-0003）。バンドル URL がフル URL（`https://…`）なら
/// そのまま正規化し、origin 相対パス（`/solid/bundle.js`）なら Demo Endpoint origin（`base`）に
/// 載せて解決する。解決不能な URL は [`BootError::UnresolvableEntry`]。得た target は
/// そのまま `bundle_source::fetch_from` と reload 購読を駆動する（既存経路に合流・保持）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn resolve_entry_target(
    entry: &DemoEntry,
    base: &DevServerTarget,
) -> Result<DevServerTarget, BootError> {
    let raw = entry.bundle_url.trim();
    let normalized = if raw.contains("://") {
        // フル URL（別 origin の CDN 等）はそのまま正規化する。
        raw.to_owned()
    } else {
        // origin 相対：Demo Endpoint の scheme/host/port に載せる。先頭 `/` を保証して path 化する。
        let path = if raw.starts_with('/') {
            raw.to_owned()
        } else {
            format!("/{raw}")
        };
        origin_url(base, &path)
    };
    dev_server_target::parse(&normalized)
        .ok_or_else(|| BootError::UnresolvableEntry(entry.bundle_url.clone()))
}

/// Convert a schema-valid generated DTO into the handwritten boot domain. This is where empty
/// lists, display names, bundle URL meaning, and selectability are decided; none belong to codegen.
pub fn boot_plan_from_manifest(
    wire: WireDemoManifest,
    base: &DevServerTarget,
) -> Result<BootPlan, BootError> {
    if wire.demos.is_empty() {
        return Err(BootError::EmptyManifest);
    }

    let mut initial_target = None;
    for (index, wire_entry) in wire.demos.into_iter().enumerate() {
        let entry = DemoEntry::from(wire_entry);
        if entry.name.trim().is_empty() || entry.bundle_url.trim().is_empty() {
            return Err(BootError::UnselectableEntry { index });
        }
        let target = resolve_entry_target(&entry, base)
            .map_err(|_| BootError::UnselectableEntry { index })?;
        if initial_target.is_none() {
            initial_target = Some(target);
        }
    }

    Ok(BootPlan::Direct(
        initial_target.expect("non-empty manifest produced an initial target"),
    ))
}

/// 初回起動の boot target（ADR-0003）：取得済みマニフェスト JSON を読み、**先頭デモ**を base origin に
/// 解決する 1 つの純関数。パース失敗・デモ 0 件・URL 解決不能はすべて明示エラーで返し、呼び出し側
/// （`app_tsubame`）が `error_overlay` に出して URL 入力経路へ誘導する（謎クラッシュ回避・#743）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn first_boot_target(
    manifest_json: &str,
    base: &DevServerTarget,
) -> Result<DevServerTarget, DemoManifestError> {
    let wire = decode_wire(manifest_json).map_err(DemoManifestError::Wire)?;
    match boot_plan_from_manifest(wire, base).map_err(DemoManifestError::Boot)? {
        BootPlan::Direct(target) => Ok(target),
        BootPlan::ManifestAutoload(_) => {
            unreachable!("manifest conversion always resolves to a direct target")
        }
    }
}

/// 起動時の boot 経路（接続先の決め方で分岐する・#743）。純粋な**ルーティング判断**だけを表し、
/// 実 fetch はしない（それは device の [`fetch_manifest`] / `app_tsubame`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum BootPlan {
    /// 単一バンドルを target から直 boot する（URL 入力済み／QR、debug loopback。既存経路・不変）。
    Direct(DevServerTarget),
    /// 接続先未設定の初回起動：この Demo Endpoint origin から manifest を取り、先頭デモを自動ロード
    /// する（release 既定・ADR-0003）。
    ManifestAutoload(DevServerTarget),
}

/// 起動時の boot 経路を決める（#743）。URL 入力済み（QR 含む）は従来どおり単一バンドル直 boot
/// （[`BootPlan::Direct`]・既存経路不変）。**接続先未設定の初回起動**は、release なら公開 Demo Endpoint の
/// manifest 先頭デモを自動ロード（[`BootPlan::ManifestAutoload`]・ゼロ入力で動く）、debug なら従来の
/// エミュレータ loopback を単一バンドル直 boot（既存経路不変・#534）。`is_release` は device では
/// `!cfg!(debug_assertions)` を渡す。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn plan_boot(entered: Option<&str>, is_release: bool) -> BootPlan {
    match entered {
        Some(url) => BootPlan::Direct(dev_server_target::resolve(Some(url))),
        None if is_release => {
            BootPlan::ManifestAutoload(dev_server_target::release_default_target())
        }
        None => BootPlan::Direct(dev_server_target::build_default_target()),
    }
}

/// base target の origin（scheme/host/port）に `path` を載せたフル URL。base の path は無視する
/// （マニフェストも別デモも origin 起点で解決するため）。scheme 文字列・既定ポートは target 側で
/// 正規化済み（#740）。
fn origin_url(base: &DevServerTarget, path: &str) -> String {
    format!(
        "{}://{}:{}{}",
        base.scheme().as_str(),
        base.host(),
        base.port(),
        path
    )
}

/// Demo Endpoint から Demo Manifest を取得して解決する device グルー（#740 の OS スタック fetch を
/// 再利用する）。純粋部（[`parse`] / [`resolve_entry_target`] / [`plan_boot`]）はホストで契約テスト済み。
/// 実 I/O（Kotlin の `BundleFetchBridge` = OS ネットワークスタック・ADR-0002）は device のみ。
#[cfg(target_os = "android")]
mod platform {
    use super::*;

    /// 初回起動の boot target を fetch 込みで解決する（`app_tsubame` が接続先未設定の初回に呼ぶ）：
    /// Demo Endpoint の `/demos.json` を OS スタック（`bundle_source::fetch_url`＝#740 の委譲）で GET し、
    /// **先頭デモ**を origin に解決する。取得は [`DemoManifestError::Fetch`]、以降のパース／空／解決不能は
    /// 純粋な [`first_boot_target`] が返す明示エラー（すべて謎クラッシュにせず、呼び出し側が
    /// `error_overlay` に出して URL 入力経路へ誘導する・#743）。
    pub fn first_boot_target_fetched(
        base: &DevServerTarget,
    ) -> Result<DevServerTarget, DemoManifestError> {
        let body = crate::bundle_source::fetch_url(&manifest_url(base))
            .map_err(|e| DemoManifestError::Fetch(format!("{e:?}")))?;
        first_boot_target(&body, base)
    }
}

#[cfg(target_os = "android")]
pub use platform::first_boot_target_fetched;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_server_target::Scheme;

    /// 公開 Demo Endpoint（release 既定）を base に使う。ADR-0003 の初回自動ロードはここが起点。
    fn demo_endpoint() -> DevServerTarget {
        dev_server_target::release_default_target()
    }

    /// demos.json（demo-endpoint の正本）と同型の wire。先頭 = Solid、次 = React（origin 相対 URL）。
    const WIRE: &str = r#"{
        "demos": [
            { "name": "Todo (Solid)", "bundleUrl": "/solid/bundle.js" },
            { "name": "Todo (React)", "bundleUrl": "/react/bundle.js" }
        ]
    }"#;

    #[test]
    fn parses_the_wire_manifest_into_named_entries() {
        // TS `DemoManifest`（表示名 + bundleUrl）の Rust ミラーに読める。`bundleUrl` → `bundle_url`。
        let manifest = parse(WIRE).unwrap();
        assert_eq!(manifest.entries().len(), 2);
        assert_eq!(manifest.entries()[0].name, "Todo (Solid)");
        assert_eq!(manifest.entries()[0].bundle_url, "/solid/bundle.js");
        assert_eq!(manifest.entries()[1].name, "Todo (React)");
    }

    #[test]
    fn ignores_extra_wire_fields_from_the_source_of_truth() {
        // demo-endpoint の正本 demos.json は `$comment` や `source` を持つ。wire に流用されても
        // ホストは name/bundleUrl だけ読み、余剰フィールドは無視する（配信物と wire の非対称を許容）。
        let with_extras = r#"{
            "$comment": "single source of truth",
            "demos": [
                { "name": "Todo (Solid)", "bundleUrl": "/solid/bundle.js",
                  "source": { "workspacePackage": "@tsubame/example-solid-demo" } }
            ]
        }"#;
        let manifest = parse(with_extras).unwrap();
        assert_eq!(manifest.entries()[0].name, "Todo (Solid)");
        assert_eq!(manifest.entries()[0].bundle_url, "/solid/bundle.js");
    }

    #[test]
    fn the_manifest_route_matches_the_ts_wire_contract() {
        // TS `demoEndpointContract.demoManifestRoute`（@torimi/wire-contract）と同値の wire 契約。
        assert_eq!(DEMO_MANIFEST_ROUTE, "/demos.json");
    }

    #[test]
    fn the_manifest_is_fetched_from_the_endpoint_origin_root() {
        // マニフェストは常に origin 直下。base が別デモの path を指していても origin + /demos.json に解決。
        let base = dev_server_target::parse("https://demo.example/solid/bundle.js").unwrap();
        assert_eq!(manifest_url(&base), "https://demo.example:443/demos.json");
        // 既定の公開 Demo Endpoint でも origin + /demos.json。
        assert!(manifest_url(&demo_endpoint()).ends_with(":443/demos.json"));
    }

    #[test]
    fn selecting_an_entry_resolves_a_boot_target_on_the_endpoint_origin() {
        // エントリ選択 → boot target 解決（ADR-0003）：origin 相対 bundleUrl を Demo Endpoint の
        // scheme/host/port に載せ、path を保持して target 化する。得た target は既存の fetch/reload を駆動。
        let manifest = parse(WIRE).unwrap();
        let base = dev_server_target::parse("https://torimi-demo-endpoint.workers.dev").unwrap();

        let solid = resolve_entry_target(&manifest.entries()[0], &base).unwrap();
        assert_eq!(solid.scheme(), Scheme::Https);
        assert_eq!(solid.host(), "torimi-demo-endpoint.workers.dev");
        assert_eq!(solid.port(), 443);
        assert_eq!(solid.path(), "/solid/bundle.js");

        // 別エントリ（React）は別 path に解決される（メニュー選択で切り替わる先）。
        let react = resolve_entry_target(&manifest.entries()[1], &base).unwrap();
        assert_eq!(react.path(), "/react/bundle.js");
    }

    #[test]
    fn a_full_url_entry_is_resolved_as_is() {
        // bundleUrl が別 origin のフル URL（CDN 等）なら base に載せずそのまま正規化する。
        let entry = DemoEntry {
            name: "External".to_owned(),
            bundle_url: "https://cdn.example/x/bundle.js".to_owned(),
        };
        let target = resolve_entry_target(&entry, &demo_endpoint()).unwrap();
        assert_eq!(target.host(), "cdn.example");
        assert_eq!(target.path(), "/x/bundle.js");
        assert_eq!(target.port(), 443);
    }

    #[test]
    fn first_launch_auto_loads_the_head_demo() {
        // 初回起動（接続先未設定）は manifest 先頭デモを自動ロードする（ADR-0003）。
        let base = dev_server_target::parse("https://torimi-demo-endpoint.workers.dev").unwrap();
        let target = first_boot_target(WIRE, &base).unwrap();
        // 先頭 = Solid の path に解決される。
        assert_eq!(target.path(), "/solid/bundle.js");
        assert_eq!(target.host(), "torimi-demo-endpoint.workers.dev");
    }

    #[test]
    fn a_malformed_manifest_is_an_explicit_error_that_points_at_url_entry() {
        // オフライン等でボディが空 / 壊れた JSON でも謎クラッシュにせず、明示エラー＋URL 入力誘導にする。
        for bad in ["", "   ", "not json", "{ \"demos\": ", "{ \"demos\": {} }"] {
            let err = first_boot_target(bad, &demo_endpoint()).unwrap_err();
            assert!(
                matches!(err, DemoManifestError::Wire(_)),
                "{bad:?} -> {err:?}"
            );
            assert_eq!(err.category(), DemoManifestFailureCategory::WireValidation);
            let msg = err.message();
            assert!(
                msg.contains(DEMO_MANIFEST_ROUTE),
                "message names the route: {msg}"
            );
            assert!(
                msg.contains("URL 入力"),
                "message guides to the URL-entry path: {msg}"
            );
        }
    }

    #[test]
    fn an_empty_manifest_is_an_explicit_error_not_a_crash() {
        // デモ 0 件は自動ロード対象が無い明示エラー（先頭 unwrap でパニックさせない）。
        let err = first_boot_target(r#"{ "demos": [] }"#, &demo_endpoint()).unwrap_err();
        assert_eq!(err, DemoManifestError::Boot(BootError::EmptyManifest));
        assert_eq!(err.category(), DemoManifestFailureCategory::BootDomain);
        assert!(
            err.message().contains("URL 入力"),
            "empty also guides to URL entry: {}",
            err.message()
        );
        // parse 自体は成功する（空は壊れではない）。first() が None を返すだけ。
        assert!(parse(r#"{ "demos": [] }"#).unwrap().first().is_none());
    }

    #[test]
    fn wire_validation_and_boot_domain_failures_have_separate_categories() {
        let missing_required = decode_wire(r#"{ "demos": [{ "name": "Todo" }] }"#)
            .expect_err("missing bundleUrl is a wire failure");
        assert!(matches!(
            missing_required,
            WireManifestError::InvalidShape(_)
        ));
        assert_eq!(
            DemoManifestError::Wire(missing_required)
                .category()
                .as_str(),
            "wire-validation"
        );

        let empty = decode_wire(r#"{ "demos": [] }"#).unwrap();
        let empty_error = boot_plan_from_manifest(empty, &demo_endpoint())
            .expect_err("empty is wire-valid but cannot boot");
        assert_eq!(empty_error, BootError::EmptyManifest);
        assert_eq!(
            DemoManifestError::Boot(empty_error).category().as_str(),
            "boot-domain"
        );

        let unselectable =
            decode_wire(r#"{ "demos": [{ "name": "", "bundleUrl": "ftp://nope" }] }"#).unwrap();
        let entry_error = boot_plan_from_manifest(unselectable, &demo_endpoint())
            .expect_err("an entry without a display name cannot be selected");
        assert!(matches!(
            entry_error,
            BootError::UnselectableEntry { index: 0 }
        ));
    }

    #[test]
    fn a_fetch_failure_is_an_explicit_error_that_points_at_url_entry() {
        // オフライン等で `/demos.json` の取得自体に失敗しても謎クラッシュにせず、明示エラー＋URL 入力誘導。
        let err = DemoManifestError::Fetch("HTTP 503 from demo endpoint".to_owned());
        let msg = err.message();
        assert!(msg.contains(DEMO_MANIFEST_ROUTE), "names the route: {msg}");
        assert!(
            msg.contains("URL 入力"),
            "guides to the URL-entry path: {msg}"
        );
    }

    #[test]
    fn entered_url_plans_a_direct_boot_unchanged() {
        // URL 入力済み（QR 含む）は従来どおり単一バンドル直 boot。release/debug いずれでも既存経路不変。
        let lan = plan_boot(Some("192.168.1.5:5179"), false);
        assert_eq!(
            lan,
            BootPlan::Direct(dev_server_target::parse("192.168.1.5:5179").unwrap())
        );
        // 貼られた公開デモのフル URL も Direct（その path を直 boot）。release でも Direct のまま。
        let pasted = plan_boot(Some("https://demo.example/react/bundle.js"), true);
        match pasted {
            BootPlan::Direct(t) => assert_eq!(t.path(), "/react/bundle.js"),
            other => panic!("entered URL must plan a Direct boot, got {other:?}"),
        }
    }

    #[test]
    fn first_launch_on_release_plans_manifest_autoload_from_the_demo_endpoint() {
        // 接続先未設定の初回起動（release）は公開 Demo Endpoint の manifest 自動ロード経路に入る。
        match plan_boot(None, true) {
            BootPlan::ManifestAutoload(endpoint) => {
                assert_eq!(endpoint.scheme(), Scheme::Https);
                assert_eq!(endpoint, dev_server_target::release_default_target());
            }
            other => panic!("first release launch must autoload the manifest, got {other:?}"),
        }
    }

    #[test]
    fn first_launch_on_debug_keeps_the_loopback_direct_boot_unchanged() {
        // debug 既定（エミュレータ loopback）は #534 のまま：manifest 経路に入らず単一バンドル直 boot。
        assert_eq!(
            plan_boot(None, false),
            BootPlan::Direct(DevServerTarget::default())
        );
    }

    #[test]
    fn an_unresolvable_bundle_url_is_an_explicit_error() {
        // bundleUrl が壊れていて target 化できないエントリも明示エラーにする（クラッシュしない）。
        let entry = DemoEntry {
            name: "Broken".to_owned(),
            bundle_url: "ftp://nope".to_owned(),
        };
        let err = resolve_entry_target(&entry, &demo_endpoint()).unwrap_err();
        assert!(matches!(err, BootError::UnresolvableEntry(_)), "{err:?}");
        let manifest_error = DemoManifestError::Boot(err);
        assert!(
            manifest_error.message().contains("URL 入力"),
            "{}",
            manifest_error.message()
        );
    }
}
