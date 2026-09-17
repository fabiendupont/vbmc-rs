//! Cross-BMC behavioral/conformance harness runner (twin-facade P6 track b).
//!
//! `vbmc-rs-scenario` replays the *same* twin scenario files that the in-process
//! replay test drives (`tests/sequences/twin/*.json`) — but against a **live**
//! Redfish BMC over the wire, with real wall-clock pacing. It is the dynamic
//! counterpart to schema conformance: schema conformance asks "is the document
//! shaped right?", this asks "does the device *behave* right as state evolves?".
//!
//! The IEC-61850 test-set split holds throughout:
//!
//! - **Stimulus face** ("inject") is simulator-specific → a [`StimulusDriver`]
//!   chosen once via `--driver`. `twin` drives a vbmc-rs simulate instance through
//!   its twin control-plane; `observe` is a no-op for real hardware (an operator
//!   injects stimulus out-of-band on a bench, and the harness only observes).
//! - **Verification face** ("observe the trip") is standard Redfish and therefore
//!   DUT-agnostic: a [`RedfishProbe`] reads the resolved read path, MetricReports,
//!   and the `EventService` SSE stream, and the *identical* verifiers from
//!   [`vbmc_rs::scenario`] (`json_contains`, `check_matchspec`, `event_matches`)
//!   decide pass/fail — the same logic the in-process replay test applies.
//!
//! Unlike the in-process path (paused tokio clock, drained in-memory event bus),
//! the remote clock cannot be paused, so each step's `advance_seconds` becomes a
//! real sleep of `advance_seconds * timescale` seconds, and events are collected
//! over that same wall-clock window from a freshly opened SSE stream.
//!
//! ```text
//! vbmc-rs-scenario \
//!   --target https://127.0.0.1:8443 --insecure --driver twin \
//!   --scenario tests/sequences/twin/scenario_read_path.json
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use serde_json::Value;

use vbmc_rs::scenario::driver::{ObserveOnlyDriver, StimulusDriver, TwinIngestDriver};
use vbmc_rs::scenario::probe::RedfishProbe;
use vbmc_rs::scenario::{
    ExpectSpec, ObservedEvent, TwinSequence, TwinStep, check_matchspec, event_matches,
    json_contains, json_path_present,
};

/// Which stimulus driver the runner presses "inject" through.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum DriverKind {
    /// Drive a vbmc-rs simulate instance via its twin control-plane.
    Twin,
    /// No-op: stimulus is injected out-of-band; the harness only observes.
    Observe,
}

#[derive(Parser)]
#[command(
    name = "vbmc-rs-scenario",
    about = "Replay twin scenario files against a live Redfish BMC and verify behavior"
)]
struct Args {
    /// Base URL of the BMC under test (e.g. https://127.0.0.1:8443).
    #[arg(long)]
    target: String,

    /// Scenario file, or a directory of `*.json` scenarios. Repeatable.
    #[arg(long = "scenario", value_name = "FILE|DIR", required = true)]
    scenarios: Vec<PathBuf>,

    /// Stimulus driver: `twin` presses inject via the twin control-plane;
    /// `observe` injects nothing (real HW, operator injects out-of-band).
    #[arg(long, value_enum, default_value_t = DriverKind::Twin)]
    driver: DriverKind,

    /// Accept invalid/self-signed TLS certs (simulate mode serves self-signed).
    #[arg(long)]
    insecure: bool,

    /// Real seconds per scenario second. The remote clock can't be paused, so
    /// each step sleeps `advance_seconds * timescale`. Below 1.0 runs faster than
    /// scenario time (only sound if the BMC's tick interval is scaled to match).
    #[arg(long, default_value_t = 1.0)]
    timescale: f64,
}

/// Verify a response body + status against a step's expectations. Mirrors the
/// in-process `assert_expect` in `tests/replay.rs`, but returns an error string
/// instead of panicking so the runner can tally failures and continue.
fn verify_expect(
    status: u16,
    body: &Value,
    expect: &ExpectSpec,
    series: &mut HashMap<String, f64>,
) -> Result<(), String> {
    if status != expect.status {
        return Err(format!("status {status} != expected {}", expect.status));
    }
    if let Some(expected) = &expect.body_contains {
        json_contains(body, expected).map_err(|loc| format!("body mismatch at {loc}"))?;
    }
    if let Some(expected) = &expect.body_equals
        && body != expected
    {
        return Err(format!("body_equals mismatch (actual: {body})"));
    }
    if let Some(paths) = &expect.body_lacks {
        for p in paths {
            if json_path_present(body, p) {
                return Err(format!("expected path '{p}' to be absent"));
            }
        }
    }
    if let Some(specs) = &expect.body_matches {
        for spec in specs {
            check_matchspec(body, spec, series).map_err(|e| format!("matcher failed: {e}"))?;
        }
    }
    Ok(())
}

/// Check that every expected event was observed on the stream over the window.
fn verify_events(
    observed: &[ObservedEvent],
    wanted: &[vbmc_rs::scenario::EventMatch],
) -> Result<(), String> {
    for want in wanted {
        if !observed.iter().any(|ev| event_matches(ev, want)) {
            let saw = observed
                .iter()
                .map(|e| format!("{}/{}/{}", e.event_type, e.severity, e.message_id))
                .collect::<Vec<_>>()
                .join(", ");
            let saw = if saw.is_empty() {
                "(none)".to_string()
            } else {
                saw
            };
            return Err(format!("no event matched {want:?} (saw {saw})"));
        }
    }
    Ok(())
}

/// Run one step: drive stimulus, pace the window (collecting SSE if events are
/// expected), then verify the read path and events.
async fn run_step<D: StimulusDriver>(
    driver: &D,
    probe: &RedfishProbe,
    step: &TwinStep,
    timescale: f64,
    series: &mut HashMap<String, f64>,
) -> Result<(), String> {
    // --- Stimulus face: press inject (twin driver acts; observe driver skips).
    if let Some(scenario) = &step.arm {
        driver.arm(scenario).await?;
    }
    if let Some(samples) = &step.ingest {
        driver.ingest(samples).await?;
    }

    // --- Pace the step in real time. When events are expected we spend the
    //     window reading the SSE stream (opened now, after stimulus, so alerts
    //     the window produces are captured); otherwise we simply sleep it out.
    let window = Duration::from_secs_f64(step.advance_seconds.max(0.0) * timescale);
    let observed = if step.expect_events.is_some() {
        probe.collect_sse(window).await?
    } else {
        if !window.is_zero() {
            tokio::time::sleep(window).await;
        }
        Vec::new()
    };

    // --- Verification face: read path + matchers. `get_json` requires a 2xx
    //     response (all twin read/MetricReport steps expect 200); a non-2xx is
    //     surfaced as the step failure it is.
    if let Some(req) = &step.request {
        if !req.method.eq_ignore_ascii_case("GET") {
            return Err(format!(
                "unsupported method '{}' (the over-the-wire harness verifies read paths)",
                req.method
            ));
        }
        let expect = step
            .expect
            .as_ref()
            .ok_or_else(|| "a request step needs `expect`".to_string())?;
        let body = probe.get_json(&req.path).await?;
        verify_expect(200, &body, expect, series)?;
    }

    // --- Verification face: events emitted over the window.
    if let Some(wanted) = &step.expect_events {
        verify_events(&observed, wanted)?;
    }

    Ok(())
}

/// Replay one scenario file, returning `(steps_passed, steps_failed)`. Prints a
/// per-step line as it goes.
async fn run_file<D: StimulusDriver>(
    driver: &D,
    probe: &RedfishProbe,
    path: &Path,
    timescale: f64,
) -> (usize, usize) {
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(e) => {
            println!("  FAIL  read {}: {e}", path.display());
            return (0, 1);
        }
    };
    let seq: TwinSequence = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => {
            println!("  FAIL  parse {}: {e}", path.display());
            return (0, 1);
        }
    };

    let file = path.file_name().unwrap_or_default().to_string_lossy();
    println!("▶ {file}: {}", seq.description);

    let mut series: HashMap<String, f64> = HashMap::new();
    let (mut passed, mut failed) = (0, 0);
    for step in &seq.steps {
        match run_step(driver, probe, step, timescale, &mut series).await {
            Ok(()) => {
                passed += 1;
                println!("  ok    {}", step.name);
            }
            Err(e) => {
                failed += 1;
                println!("  FAIL  {}: {e}", step.name);
            }
        }
    }
    (passed, failed)
}

/// Expand each `--scenario` argument (a file or a directory of `*.json`) into a
/// sorted, flat list of scenario files.
fn collect_scenario_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut dir_files: Vec<PathBuf> = fs::read_dir(p)
                .map_err(|e| format!("read dir {}: {e}", p.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            dir_files.sort();
            files.extend(dir_files);
        } else {
            files.push(p.clone());
        }
    }
    if files.is_empty() {
        return Err("no scenario files to run".into());
    }
    Ok(files)
}

/// Run every file through the selected driver; returns `(passed, failed)` totals.
async fn run_all<D: StimulusDriver>(
    driver: &D,
    probe: &RedfishProbe,
    files: &[PathBuf],
    timescale: f64,
) -> (usize, usize) {
    let (mut passed, mut failed) = (0, 0);
    for path in files {
        let (p, f) = run_file(driver, probe, path, timescale).await;
        passed += p;
        failed += f;
    }
    (passed, failed)
}

async fn run() -> Result<bool, String> {
    let args = Args::parse();
    if args.timescale <= 0.0 {
        return Err(format!(
            "--timescale must be positive (got {})",
            args.timescale
        ));
    }
    let files = collect_scenario_files(&args.scenarios)?;
    let probe = RedfishProbe::new(&args.target, args.insecure)?;

    println!(
        "harness: target={} driver={:?} timescale={} — {} scenario file(s)",
        args.target,
        args.driver,
        args.timescale,
        files.len()
    );

    let (passed, failed) = match args.driver {
        DriverKind::Twin => {
            let driver = TwinIngestDriver::new(&args.target, args.insecure)?;
            run_all(&driver, &probe, &files, args.timescale).await
        }
        DriverKind::Observe => run_all(&ObserveOnlyDriver, &probe, &files, args.timescale).await,
    };

    println!(
        "\n{} passed, {} failed ({} steps)",
        passed,
        failed,
        passed + failed
    );
    Ok(failed == 0)
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn expect(v: Value) -> ExpectSpec {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn verify_expect_status_and_contains() {
        let mut series = HashMap::new();
        let body = json!({ "Id": "Temp0", "Reading": 20.0 });
        let e = expect(json!({ "status": 200, "body_contains": { "Id": "Temp0" } }));
        assert!(verify_expect(200, &body, &e, &mut series).is_ok());

        // Wrong status is a failure.
        assert!(verify_expect(500, &body, &e, &mut series).is_err());

        // Missing key is a failure with a JSON-path-ish location.
        let e2 = expect(json!({ "status": 200, "body_contains": { "Missing": 1 } }));
        assert!(verify_expect(200, &body, &e2, &mut series).is_err());
    }

    #[test]
    fn verify_expect_matchers_run() {
        let mut series = HashMap::new();
        let body = json!({ "Reading": 35.0 });
        let e = expect(json!({
            "status": 200,
            "body_matches": [ { "path": "Reading", "range": [30.0, 40.0] } ]
        }));
        assert!(verify_expect(200, &body, &e, &mut series).is_ok());

        let out = json!({ "Reading": 99.0 });
        assert!(verify_expect(200, &out, &e, &mut series).is_err());
    }

    #[test]
    fn verify_events_needs_every_wanted() {
        let observed = vec![ObservedEvent {
            event_type: "Alert".into(),
            severity: "Warning".into(),
            message_id: "TwinAlert.1.0.ThresholdCrossed".into(),
            origin_of_condition: None,
        }];
        let want: Vec<vbmc_rs::scenario::EventMatch> = serde_json::from_value(json!([
            { "severity": "Warning", "message_id_contains": "ThresholdCrossed" }
        ]))
        .unwrap();
        assert!(verify_events(&observed, &want).is_ok());

        let miss: Vec<vbmc_rs::scenario::EventMatch> =
            serde_json::from_value(json!([{ "severity": "Critical" }])).unwrap();
        assert!(verify_events(&observed, &miss).is_err());
    }

    #[test]
    fn collect_scenario_files_empty_is_error() {
        assert!(collect_scenario_files(&[]).is_err());
    }
}
