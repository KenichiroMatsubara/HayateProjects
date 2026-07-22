use std::fmt::Write;

use crate::{AcceptanceReport, FindingSeverity, GateVerdict, MatrixAcceptanceReport};

impl AcceptanceReport {
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Self-contained reviewer artifact. It contains the same raw evidence as the JSON output,
    /// escaped inside a details block, so a human and automation can audit one result bundle.
    pub fn to_html(&self) -> serde_json::Result<String> {
        let raw = self.to_json_pretty()?;
        let mut findings = String::new();
        if self.findings.is_empty() {
            findings.push_str("<tr><td colspan=\"6\">No regressions observed</td></tr>");
        } else {
            for finding in &self.findings {
                let severity = match finding.severity {
                    FindingSeverity::Minor => "minor",
                    FindingSeverity::Material => "material",
                };
                let percent = finding
                    .increase_percent_x100
                    .map(|value| format!("{}.{:02}%", value / 100, value % 100))
                    .unwrap_or_else(|| "n/a".into());
                write!(
                    findings,
                    "<tr class=\"{severity}\"><td>{}</td><td>{severity}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape_html(&finding.metric),
                    finding.baseline,
                    finding.candidate,
                    percent,
                    escape_html(&finding.rule),
                )
                .expect("writing to String cannot fail");
            }
        }

        let candidate = &self.evidence.candidate;
        Ok(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Hayate performance acceptance</title><style>{STYLE}</style></head><body><main><h1>Hayate performance acceptance</h1><section class=\"verdicts\"><strong>Overall: {overall}</strong><span>Performance: {performance}</span><span>Structural: {structural}</span></section><section><h2>Matched environment</h2><dl><dt>Device</dt><dd>{device} ({model})</dd><dt>Refresh rate</dt><dd>{refresh}Hz</dd><dt>Candidate commit</dt><dd>{commit}</dd><dt>Build</dt><dd>{build}</dd><dt>Renderer selection reason</dt><dd>{renderer}</dd><dt>Failure category</dt><dd>{failure}</dd><dt>Battery / condition</dt><dd>{battery} / {condition}</dd></dl></section><section><h2>Acceptance findings</h2><table><thead><tr><th>Metric</th><th>Severity</th><th>Baseline</th><th>Candidate</th><th>Increase</th><th>Rule</th></tr></thead><tbody>{findings}</tbody></table></section><details><summary>Raw evidence</summary><pre>{raw}</pre></details></main></body></html>",
            overall = verdict_name(self.overall),
            performance = verdict_name(self.performance),
            structural = verdict_name(self.structural),
            device = escape_html(&candidate.device_id),
            model = escape_html(&candidate.device_model),
            refresh = candidate.refresh_rate_hz,
            commit = escape_html(&candidate.commit),
            build = escape_html(&candidate.build_id),
            renderer = escape_html(&candidate.renderer_selection_reason),
            failure = escape_html(candidate.failure_category.as_deref().unwrap_or("none")),
            battery = escape_html(&candidate.battery_power_state),
            condition = escape_html(&candidate.warm_condition),
            raw = escape_html(&raw),
        ))
    }
}

impl MatrixAcceptanceReport {
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_html(&self) -> serde_json::Result<String> {
        let raw = self.to_json_pretty()?;
        let mut rows = String::new();
        for case in &self.cases {
            let evidence = &case.evidence.candidate;
            write!(
                rows,
                "<tr><td>{:?}</td><td>{}Hz</td><td>{:?}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                evidence.target,
                evidence.refresh_rate_hz,
                evidence.workload,
                verdict_name(case.performance),
                case.findings
                    .iter()
                    .filter(|finding| finding.is_material())
                    .count(),
                escape_html(&evidence.commit),
            )
            .expect("writing to String cannot fail");
        }
        Ok(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Hayate performance matrix</title><style>{STYLE}</style></head><body><main><h1>Hayate performance matrix</h1><section class=\"verdicts\"><strong>Overall: {overall}</strong><span>Performance: {performance}</span><span>Structural: {structural}</span></section><section><h2>Matched cases</h2><table><thead><tr><th>Target</th><th>Refresh</th><th>Workload</th><th>Performance</th><th>Material findings</th><th>Candidate commit</th></tr></thead><tbody>{rows}</tbody></table></section><details><summary>Raw evidence</summary><pre>{raw}</pre></details></main></body></html>",
            overall = verdict_name(self.overall),
            performance = verdict_name(self.performance),
            structural = verdict_name(self.structural),
            raw = escape_html(&raw),
        ))
    }
}

fn verdict_name(verdict: GateVerdict) -> &'static str {
    match verdict {
        GateVerdict::Pass => "pass",
        GateVerdict::Fail => "fail",
        GateVerdict::NotEvaluated => "not evaluated",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const STYLE: &str = r#"
:root { color-scheme: light dark; font-family: system-ui, sans-serif; }
body { margin: 0; background: #111827; color: #e5e7eb; }
main { max-width: 1100px; margin: 0 auto; padding: 32px; }
.verdicts { display: flex; gap: 24px; padding: 16px; background: #1f2937; border-radius: 8px; }
dl { display: grid; grid-template-columns: 220px 1fr; gap: 8px; }
dt { color: #9ca3af; } dd { margin: 0; }
table { width: 100%; border-collapse: collapse; }
th, td { padding: 8px; text-align: left; border-bottom: 1px solid #374151; }
.material { color: #fca5a5; } .minor { color: #fde68a; }
pre { overflow: auto; padding: 16px; background: #030712; }
details { margin-top: 24px; }
"#;
