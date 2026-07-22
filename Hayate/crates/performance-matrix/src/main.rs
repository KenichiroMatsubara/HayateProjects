use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use hayate_performance_matrix::{
    evaluate_matrix, CollectionEnvironment, GateVerdict, HealthGrade, MatrixAcceptanceEvidence,
    MatrixCase, MatrixCaseExecutor, MatrixEvidenceBundle, MatrixRunner, PerformanceMatrix,
    RefreshRateController, RunEvidence, RunnerSettings, StructuralEvidence,
};

const THERMAL_POLL_INTERVAL_MILLIS: u64 = 2_000;
const CHILD_POLL_INTERVAL_MILLIS: u64 = 100;
const REFRESH_SETTING_KEYS: [&str; 3] =
    ["min_refresh_rate", "peak_refresh_rate", "user_refresh_rate"];

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn record_interrupt(_signal: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[cfg(unix)]
fn install_interrupt_handlers() {
    // SAFETY: the handler only stores to a lock-free atomic, which is async-signal-safe.
    unsafe {
        libc::signal(
            libc::SIGINT,
            record_interrupt as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            record_interrupt as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGHUP,
            record_interrupt as *const () as libc::sighandler_t,
        );
    }
}

#[cfg(not(unix))]
fn install_interrupt_handlers() {}

fn main() {
    install_interrupt_handlers();
    if let Err(error) = run_cli() {
        eprintln!("hayate-performance-matrix: {error}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command] if command == "plan" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&PerformanceMatrix::standard())
                    .map_err(|error| error.to_string())?
            );
            Ok(())
        }
        [command, case_command, environment, output] if command == "run" => {
            let environment: CollectionEnvironment = read_json(environment)?;
            let connected_device = adb_output(&["get-serialno"])?;
            if connected_device.trim() != environment.device_id {
                return Err(format!(
                    "connected device {} does not match captured environment {}",
                    connected_device.trim(),
                    environment.device_id
                ));
            }
            let mut refresh = AdbRefreshController;
            let mut executor = CommandExecutor {
                case_command: PathBuf::from(case_command),
            };
            let outputs = MatrixRunner::new(
                PerformanceMatrix::standard(),
                RunnerSettings::default(),
            )
            .run(&mut refresh, &mut executor)
            .map_err(|error| format!("matrix execution failed: {error:?}"))?;
            write_json(output, &MatrixEvidenceBundle::from_outputs(environment, outputs))
        }
        [command, build_id, assets, fonts, surface, renderer_reason, failure_category, output]
            if command == "environment" =>
        {
            let device_id = adb_output(&["get-serialno"])?;
            let device_model = adb_output(&["shell", "getprop", "ro.product.model"])?;
            let battery = adb_output(&["shell", "dumpsys", "battery"])?;
            let commit = command_output("git", &["rev-parse", "HEAD"])?;
            let environment = CollectionEnvironment {
                device_id: device_id.trim().into(),
                device_model: device_model.trim().into(),
                build_id: build_id.clone(),
                commit: commit.trim().into(),
                build_settings: hayate_performance_matrix::BuildSettings::profileable_release(
                    assets, fonts, surface,
                ),
                renderer_selection_reason: renderer_reason.clone(),
                failure_category: (failure_category != "none").then(|| failure_category.clone()),
                battery_power_state: battery.lines().collect::<Vec<_>>().join("; "),
                warm_condition: format!(
                    "steady_after_{}_frames",
                    hayate_performance_matrix::DEFAULT_WARMUP_FRAMES
                ),
            };
            write_json(output, &environment)
        }
        [command, baseline, candidate, structural, output_prefix] if command == "evaluate" => {
            let baseline = read_json(baseline)?;
            let candidate = read_json(candidate)?;
            let structural = parse_structural(structural);
            let report = evaluate_matrix(MatrixAcceptanceEvidence {
                baseline,
                candidate,
                structural,
            })
            .map_err(|errors| errors.join("; "))?;
            let json_path = format!("{output_prefix}.json");
            let html_path = format!("{output_prefix}.html");
            fs::write(
                &json_path,
                report.to_json_pretty().map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write {json_path}: {error}"))?;
            fs::write(
                &html_path,
                report.to_html().map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write {html_path}: {error}"))?;
            println!(
                "performance={:?} structural={:?} overall={:?} json={} html={}",
                report.performance, report.structural, report.overall, json_path, html_path
            );
            if report.overall == GateVerdict::Fail {
                std::process::exit(2);
            }
            Ok(())
        }
        _ => Err(
            "usage: hayate-performance-matrix plan | environment BUILD_ID ASSETS FONTS SURFACE RENDERER_REASON FAILURE_CATEGORY OUTPUT.json | run CASE_COMMAND ENVIRONMENT.json OUTPUT.json | evaluate BASELINE.json CANDIDATE.json passed|not-evaluated|failed:REASON OUTPUT_PREFIX"
                .into(),
        ),
    }
}

fn parse_structural(value: &str) -> StructuralEvidence {
    match value {
        "passed" => StructuralEvidence::Passed,
        "not-evaluated" => StructuralEvidence::NotEvaluated,
        _ => StructuralEvidence::Failed {
            reason: value.strip_prefix("failed:").unwrap_or(value).into(),
        },
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {path}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {path}: {error}"))
}

fn write_json(path: &str, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| format!("write {path}: {error}"))
}

#[derive(Debug, Clone)]
struct RefreshMode {
    values: Vec<(&'static str, Option<String>)>,
}

struct AdbRefreshController;

impl RefreshRateController for AdbRefreshController {
    type Mode = RefreshMode;
    type Error = String;

    fn current_mode(&mut self) -> Result<Self::Mode, Self::Error> {
        let mut values = Vec::with_capacity(REFRESH_SETTING_KEYS.len());
        for key in REFRESH_SETTING_KEYS {
            let value = adb_output(&["shell", "settings", "get", "system", key])?;
            let value = value.trim();
            values.push((
                key,
                (!value.is_empty() && value != "null").then(|| value.to_string()),
            ));
        }
        Ok(RefreshMode { values })
    }

    fn set_fixed_rate(&mut self, refresh_rate_hz: u32) -> Result<(), Self::Error> {
        let value = refresh_rate_hz.to_string();
        for key in REFRESH_SETTING_KEYS {
            adb_status(&["shell", "settings", "put", "system", key, &value])?;
            let observed = adb_output(&["shell", "settings", "get", "system", key])?;
            let observed = observed
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("refresh setting {key} returned {observed:?}"))?;
            if (observed - refresh_rate_hz as f32).abs() > f32::EPSILON {
                return Err(format!(
                    "refresh setting {key} expected {refresh_rate_hz}, observed {observed}"
                ));
            }
        }
        Ok(())
    }

    fn restore_mode(&mut self, mode: &Self::Mode) -> Result<(), Self::Error> {
        let mut first_error = None;
        for (key, value) in &mode.values {
            let result = match value {
                Some(value) => adb_status(&["shell", "settings", "put", "system", key, value]),
                None => adb_status(&["shell", "settings", "delete", "system", key]),
            };
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

struct CommandExecutor {
    case_command: PathBuf,
}

impl MatrixCaseExecutor for CommandExecutor {
    type Error = String;

    fn execute(
        &mut self,
        case: MatrixCase,
        settings: RunnerSettings,
    ) -> Result<RunEvidence, Self::Error> {
        if INTERRUPTED.load(Ordering::SeqCst) {
            return Err("interrupted".into());
        }
        let case_json = serde_json::to_string(&case).map_err(|error| error.to_string())?;
        let settings_json = serde_json::to_string(&settings).map_err(|error| error.to_string())?;
        let mut child = Command::new(&self.case_command)
            .arg(&case_json)
            .arg(&settings_json)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("spawn {}: {error}", self.case_command.display()))?;
        let status = loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("interrupted".into());
            }
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            std::thread::sleep(Duration::from_millis(CHILD_POLL_INTERVAL_MILLIS));
        };
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .expect("case command stdout is piped")
            .read_to_string(&mut stdout)
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("case command exited with {status}"));
        }
        serde_json::from_str(&stdout).map_err(|error| format!("case evidence JSON: {error}"))
    }

    fn await_thermal_guard(
        &mut self,
        maximum: HealthGrade,
        timeout_millis: u64,
    ) -> Result<(), Self::Error> {
        let started = Instant::now();
        loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                return Err("interrupted".into());
            }
            let output = adb_output(&["shell", "dumpsys", "thermalservice"])?;
            let status = parse_thermal_status(&output)
                .ok_or_else(|| "thermalservice output has no current status".to_string())?;
            if status <= maximum {
                return Ok(());
            }
            if started.elapsed() >= Duration::from_millis(timeout_millis) {
                return Err(format!("thermal guard timed out at {status:?}"));
            }
            std::thread::sleep(Duration::from_millis(THERMAL_POLL_INTERVAL_MILLIS));
        }
    }

    fn cooldown(&mut self, millis: u64) -> Result<(), Self::Error> {
        let deadline = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < deadline {
            if INTERRUPTED.load(Ordering::SeqCst) {
                return Err("interrupted".into());
            }
            std::thread::sleep(Duration::from_millis(
                CHILD_POLL_INTERVAL_MILLIS.min(millis.max(1)),
            ));
        }
        Ok(())
    }
}

fn parse_thermal_status(output: &str) -> Option<HealthGrade> {
    let raw = output
        .lines()
        .find_map(|line| line.split_once("mStatus=").map(|(_, value)| value.trim()))?
        .split_whitespace()
        .next()?
        .parse::<u8>()
        .ok()?;
    Some(match raw {
        0 => HealthGrade::Nominal,
        1 | 2 => HealthGrade::Elevated,
        3 => HealthGrade::Severe,
        _ => HealthGrade::Critical,
    })
}

fn adb_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("adb")
        .args(args)
        .output()
        .map_err(|error| format!("adb {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "adb {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn adb_status(args: &[&str]) -> Result<(), String> {
    adb_output(args).map(|_| ())
}

fn command_output(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("{command} {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "{command} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
