#!/usr/bin/env python3
"""Collect one case for hayate-performance-matrix.

The Rust runner owns matrix ordering, refresh restoration, thermal guard, cooldown and reports.
This adapter owns only one workload execution and returns one RunEvidence JSON object on stdout.
"""

from __future__ import annotations

import json
import os
import re
import resource
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

PACKAGE_NAME = "com.hayateprojects.torimi.benchmark"
ACTIVITY_NAME = "com.hayateprojects.hayate.adapter_android_demo.MainActivity"
REMOTE_TRACE_PATH = "/data/misc/perfetto-traces/hayate-performance-matrix.perfetto-trace"
SUMMARY_TAG = "HayatePerf"
BYTES_PER_KIB = 1024
HOST_SAMPLE_FRAMES = 240
HOST_STATE_SIZE = 16_384
HOST_RESOURCE_BYTES_PER_ENTRY = 64
HOST_HASH_MULTIPLIER = 31
PERFETTO_BUFFER_SIZE_KIB = 8192

PHASE_KEYS = (
    "app_host_p95_ns",
    "core_commit_p95_ns",
    "scene_lowering_p95_ns",
    "layer_presentation_p95_ns",
    "renderer_submit_p95_ns",
    "renderer_present_p95_ns",
)


def run(command: list[str], *, input_text: str | None = None) -> str:
    completed = subprocess.run(
        command,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"{' '.join(command)} failed ({completed.returncode}): {completed.stderr.strip()}"
        )
    return completed.stdout


def adb(*args: str, input_text: str | None = None) -> str:
    return run(["adb", *args], input_text=input_text)


def workload_name(workload: dict[str, Any]) -> str:
    return str(workload["kind"])


def artifact_directory(case: dict[str, Any]) -> Path:
    root = Path(os.environ.get("HAYATE_MATRIX_ARTIFACT_ROOT", "performance-matrix-raw"))
    name = workload_name(case["workload"])
    path = root / case["target"] / f"{case['refresh_rate_hz']}hz" / name / f"run-{case['run_index']}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def write(path: Path, content: str) -> None:
    path.write_text(content, encoding="utf-8")


def correctness_evidence() -> dict[str, Any]:
    names = {
        "correctness": "HAYATE_CORRECTNESS",
        "pixel_parity": "HAYATE_PIXEL_PARITY",
        "input_result": "HAYATE_INPUT_RESULT",
    }
    evidence: dict[str, Any] = {}
    blockers: list[str] = []
    for name, variable in names.items():
        value = os.environ.get(variable)
        if value is None:
            evidence[name] = False
            blockers.append(f"{name} requires explicit manual or automated result")
        else:
            evidence[name] = value.lower() in ("1", "true", "pass", "passed")
    evidence["manual_blockers"] = blockers
    return evidence


def grade_from_android_status(raw: int) -> str:
    if raw == 0:
        return "nominal"
    if raw <= 2:
        return "elevated"
    if raw == 3:
        return "severe"
    return "critical"


def parse_thermal(output: str) -> str:
    match = re.search(r"mStatus=(\d+)", output)
    return grade_from_android_status(int(match.group(1))) if match else "nominal"


def parse_total_pss_bytes(meminfo: str) -> int:
    for pattern in (r"TOTAL PSS:\s*(\d+)", r"^\s*TOTAL\s+(\d+)"):
        match = re.search(pattern, meminfo, re.MULTILINE)
        if match:
            return int(match.group(1)) * BYTES_PER_KIB
    return 0


def parse_gpu_bytes(meminfo: str) -> int:
    total_kib = 0
    for line in meminfo.splitlines():
        if not any(label in line for label in ("GL mtrack", "EGL mtrack", "Gfx dev")):
            continue
        match = re.search(r"(\d+)", line)
        if match:
            total_kib += int(match.group(1))
    return total_kib * BYTES_PER_KIB


def parse_gfx_frame_totals(gfx: str) -> list[int]:
    header: list[str] | None = None
    totals: list[int] = []
    for line in gfx.splitlines():
        if line.startswith("Flags,IntendedVsync"):
            header = line.split(",")
            continue
        if header is None or not line or not line[0].isdigit():
            continue
        values = line.split(",")
        if len(values) < len(header):
            continue
        try:
            intended = int(values[header.index("IntendedVsync")])
            completed = int(values[header.index("FrameCompleted")])
        except (ValueError, IndexError):
            continue
        if completed >= intended:
            totals.append(completed - intended)
    return totals


def parse_startup_ns(am_start: str) -> int:
    match = re.search(r"^TotalTime:\s*(\d+)", am_start, re.MULTILINE)
    if not match:
        raise RuntimeError("adb am start -W output has no TotalTime")
    return int(match.group(1)) * 1_000_000


def parse_summaries(logcat: str) -> list[dict[str, int]]:
    summaries: list[dict[str, int]] = []
    for line in logcat.splitlines():
        if SUMMARY_TAG not in line or "window samples=" not in line:
            continue
        values = {
            key: int(value)
            for key, value in re.findall(r"([a-zA-Z0-9_]+)=(\d+)", line)
        }
        summaries.append(values)
    if not summaries:
        raise RuntimeError("no HayatePerf window summaries were emitted")
    return summaries


def perfetto_config(duration_millis: int) -> str:
    return f"""
duration_ms: {duration_millis}
buffers: {{ size_kb: {PERFETTO_BUFFER_SIZE_KIB} fill_policy: RING_BUFFER }}
data_sources: {{
  config {{
    name: "linux.ftrace"
    ftrace_config {{
      ftrace_events: "sched/sched_switch"
      ftrace_events: "ftrace/print"
      atrace_categories: "gfx"
      atrace_categories: "view"
      atrace_apps: "{PACKAGE_NAME}"
    }}
  }}
}}
"""


def collect_android(case: dict[str, Any], settings: dict[str, Any]) -> dict[str, Any]:
    artifacts = artifact_directory(case)
    workload_json = json.dumps(case["workload"], separators=(",", ":"))
    adb("logcat", "-c")
    adb("shell", "am", "force-stop", PACKAGE_NAME)
    startup = adb(
        "shell",
        "am",
        "start",
        "-W",
        "-n",
        f"{PACKAGE_NAME}/{ACTIVITY_NAME}",
        "--es",
        "hayate_performance_workload",
        workload_json,
    )
    warmup_seconds = settings["warmup_frames"] / case["refresh_rate_hz"]
    time.sleep(warmup_seconds)
    adb(
        "shell",
        "perfetto",
        "--out",
        REMOTE_TRACE_PATH,
        "--txt",
        "-c",
        "-",
        input_text=perfetto_config(settings["perfetto_duration_millis"]),
    )
    trace = artifacts / "hayate.perfetto-trace"
    adb("pull", REMOTE_TRACE_PATH, str(trace))
    logcat = adb("logcat", "-d", "-s", SUMMARY_TAG)
    gfx = adb("shell", "dumpsys", "gfxinfo", PACKAGE_NAME, "framestats")
    meminfo = adb("shell", "dumpsys", "meminfo", PACKAGE_NAME)
    thermal = adb("shell", "dumpsys", "thermalservice")
    battery = adb("shell", "dumpsys", "battery")
    environment = adb("shell", "getprop")
    write(artifacts / "hayate-adb-summary.txt", logcat)
    write(artifacts / "hayate-gfx-framestats.txt", gfx)
    write(artifacts / "hayate-meminfo.txt", meminfo)
    write(artifacts / "hayate-thermal.txt", thermal)
    write(artifacts / "hayate-battery.txt", battery)
    write(artifacts / "hayate-environment.txt", environment)
    write(artifacts / "hayate-startup.txt", startup)

    summaries = parse_summaries(logcat)
    phase_ns = {
        key.removesuffix("_p95_ns"): [summary.get(key, 0) for summary in summaries]
        for key in PHASE_KEYS
    }
    cpu_resident = max(summary.get("cpu_resident_bytes", 0) for summary in summaries)
    if cpu_resident == 0:
        cpu_resident = parse_total_pss_bytes(meminfo)
    gpu_resident = max(summary.get("gpu_resident_bytes", 0) for summary in summaries)
    if gpu_resident == 0:
        gpu_resident = parse_gpu_bytes(meminfo)
    frame_totals = parse_gfx_frame_totals(gfx)
    if not frame_totals:
        frame_totals = [summary["total_p95_ns"] for summary in summaries]
    interval_ns = (1_000_000_000 + case["refresh_rate_hz"] - 1) // case["refresh_rate_hz"]
    return {
        "frame_total_ns": frame_totals,
        "reported_frames_over_two_intervals": sum(
            sample > interval_ns * 2 for sample in frame_totals
        ),
        "phase_ns": phase_ns,
        "cpu_resident_bytes": cpu_resident,
        "gpu_resident_bytes": gpu_resident,
        "startup_ns": parse_startup_ns(startup),
        "idle_activity_count": int(os.environ.get("HAYATE_IDLE_ACTIVITY_COUNT", "0")),
        "thermal_status": parse_thermal(thermal),
        "memory_pressure_status": os.environ.get(
            "HAYATE_MEMORY_PRESSURE_STATUS", "nominal"
        ),
        "scaling_class": os.environ.get("HAYATE_SCALING_CLASS", "linear"),
        "correctness": correctness_evidence(),
        "raw_artifacts": [str(path) for path in sorted(artifacts.iterdir())],
    }


def host_step(workload: dict[str, Any], state: list[int]) -> None:
    kind = workload["kind"]
    if kind == "layer_pressure":
        count = workload["layer_count"]
        dirty = count * workload["dirty_percent"] // 100
        state[:dirty] = reversed(state[:dirty])
    elif kind == "scene_graph_pressure":
        limit = workload["nodes"]
        accumulator = 0
        for index in range(limit):
            accumulator ^= (index * HOST_HASH_MULTIPLIER) % max(1, workload["siblings"])
        state[0] = accumulator
    elif kind == "resource_pressure":
        count = workload["resource_count"]
        resources = {
            index: bytes((index % 251,)) * HOST_RESOURCE_BYTES_PER_ENTRY
            for index in range(count)
        }
        state[0] = sum(value[0] for value in resources.values())
    elif kind == "wake_pressure":
        for index in range(workload["wakes_per_second"]):
            state[0] ^= index
    else:
        state[0] = (state[0] + 1) % len(state)


def collect_host(case: dict[str, Any], settings: dict[str, Any]) -> dict[str, Any]:
    state = list(range(HOST_STATE_SIZE))
    for _ in range(settings["warmup_frames"]):
        host_step(case["workload"], state)
    samples: list[int] = []
    for _ in range(HOST_SAMPLE_FRAMES):
        started = time.perf_counter_ns()
        host_step(case["workload"], state)
        samples.append(time.perf_counter_ns() - started)
    resident_bytes = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * BYTES_PER_KIB
    interval_ns = (1_000_000_000 + case["refresh_rate_hz"] - 1) // case["refresh_rate_hz"]
    return {
        "frame_total_ns": samples,
        "reported_frames_over_two_intervals": sum(
            sample > interval_ns * 2 for sample in samples
        ),
        "phase_ns": {"host_synthetic": samples},
        "cpu_resident_bytes": resident_bytes,
        "gpu_resident_bytes": 0,
        "startup_ns": 0,
        "idle_activity_count": 0,
        "thermal_status": "nominal",
        "memory_pressure_status": "nominal",
        "scaling_class": "linear",
        "correctness": {
            "correctness": True,
            "pixel_parity": True,
            "input_result": True,
            "manual_blockers": [],
        },
        "raw_artifacts": [],
    }


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: collect-performance-matrix-case.py CASE_JSON SETTINGS_JSON")
    case = json.loads(sys.argv[1])
    settings = json.loads(sys.argv[2])
    if case["target"] == "android_device":
        evidence = collect_android(case, settings)
    else:
        evidence = collect_host(case, settings)
    json.dump(evidence, sys.stdout, separators=(",", ":"))


if __name__ == "__main__":
    main()
