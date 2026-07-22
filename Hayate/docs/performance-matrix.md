# Performance matrix runner

`hayate-performance-matrix` is the executable acceptance gate for ADR-0156. One run covers the
representative Tsubame Task Studio workload plus named layer, scene-graph, resource, and wake
stress fixtures at fixed Android 60Hz and 90Hz (five runs each), then executes the same stress
vocabulary in the host 120Hz fixture.

Build and install the profileable benchmark APK first:

```bash
./scripts/build-android-performance-benchmark.sh
```

Capture the exact environment for each installed baseline or candidate build. `BUILD_ID` should be
an immutable APK/build identifier; the other identity fields must describe the same assets, fonts,
and surface configuration on both sides.

```bash
cargo run -p hayate-performance-matrix -- environment \
  BUILD_ID ASSETS_ID FONTS_ID rgba8 vello:selected none environment.json
```

Run the matrix. The runner saves and pins Android refresh settings, waits for its named thermal
guard, restores the exact settings on success, error, SIGINT, SIGTERM, or SIGHUP, and stores the
raw logcat, Perfetto, gfxinfo, meminfo, thermal, battery, environment, and startup artifacts.

```bash
HAYATE_MATRIX_ARTIFACT_ROOT=performance-matrix-raw/baseline \
HAYATE_CORRECTNESS=passed HAYATE_PIXEL_PARITY=passed HAYATE_INPUT_RESULT=passed \
cargo run -p hayate-performance-matrix -- run \
  ./scripts/collect-performance-matrix-case.py environment.json baseline.json
```

If correctness, pixel parity, or input results are not supplied, the collector records explicit
manual blockers and the acceptance gate fails; it never assumes those checks passed. Repeat after
installing the candidate build, then evaluate the matched bundles:

```bash
cargo run -p hayate-performance-matrix -- evaluate \
  baseline.json candidate.json passed performance-acceptance
```

This writes `performance-acceptance.json` with all raw evidence and
`performance-acceptance.html` for review. Replace `passed` with `not-evaluated` or
`failed:REASON` to keep structural acceptance separate from the performance verdict.
