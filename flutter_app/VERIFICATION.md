# Oculus App Verification Guide

[![CI](https://github.com/ronisaguey-ux/OCULUS/actions/workflows/verification.yml/badge.svg)](https://github.com/ronisaguey-ux/OCULUS/actions/workflows/verification.yml)

> **Authority:** This guide is the single source of truth for verification. It is enforced in CI by [.github/workflows/verification.yml](../.github/workflows/verification.yml).

## Rollback Guarantee

To restore the app to a known good state, run:

```bash
./tool/rollback.sh --dry-run   # show what would happen (default)
./tool/rollback.sh --force     # actually perform the rollback
```

A convenience wrapper around the commands above is provided at `./tool/rollback.sh`:

```bash
./tool/rollback.sh            # dry-run by default (safe)
./tool/rollback.sh --dry-run  # show what would happen
./tool/rollback.sh --force    # stash, hard reset, and clean (destructive)
```

The wrapper **defaults to dry-run** and requires the explicit `--force` flag to perform any destructive operation. When `--force` is used, it stashes any uncommitted changes (with `--include-untracked`) before running `git reset --hard HEAD && git clean -fd`, so no work is silently lost. Use `git stash pop` to restore stashed changes afterward.

**Note:** This command respects `.gitignore` - files listed in `.gitignore` (such as `.env*`, `*.key`, `*.pem`) will NOT be removed. Only untracked files that are NOT ignored will be cleaned.

> **WARNING:** `git clean -fd` permanently deletes untracked files. Never rely on it before backing up uncommitted `.env`, credential, or secret files that are not covered by `.gitignore`. Manually copy any such files to a safe location first, and never commit secrets to the repository as a backup strategy.

If you have uncommitted changes to tracked files that you want to keep, commit or stash them first:
```bash
git stash --include-untracked
# ... do rollback ...
git stash pop
```

## Self‑Executing Verification

Before each release, run the automated verification script:

```bash
python oculus_app/verify.py
```

The script runs the same checks as CI (static analysis, unit tests with coverage, integration tests, golden tests, performance benchmarks, dependency audit, and secret scan) and exits non-zero if any check fails, so verification is a single command with no manual step tracking.

If you need to run individual checks, use the commands in the [Quick Reference](#quick-reference).

Integration tests (located in `integration_test/`) cover critical live-trading paths:
- `integration_test/test_kill_switch.dart` — kill-switch activation and state recovery
- `integration_test/test_ws_reconnect.dart` — WebSocket reconnection and message replay
- `integration_test/test_order_flow.dart` — end-to-end order placement, fill, and cancellation

## Golden Tests (Screenshot Diffing)

Automated visual regression tests are located in `test/golden/`. To run:

```bash
flutter test --update-goldens  # Update baselines (run intentionally when UI changes)
flutter test test/golden/      # Run golden tests
```

Golden test driver: `test/golden/driver.dart` captures each documented screen state.

### Creating/Updating Baselines

When UI changes intentionally, update baselines with:

```bash
flutter test --update-goldens test/golden/
```

Then review the diff in `test/golden/baselines/` before committing.

### Adding a New Golden Test

1. Create a test file in `test/golden/` (e.g., `test/golden/screen_test.dart`).
2. Use `matchesGoldenFile('baselines/screen.png')` in the test.
3. Run `flutter test --update-goldens test/golden/` to generate the baseline.
4. Commit both the test and the baseline image.

## CI Enforcement

This guide is enforced in CI via [`.github/workflows/verification.yml`](../.github/workflows/verification.yml):

- **Static analysis** via `flutter analyze`
- **Unit tests** via `flutter test --coverage`
- **Integration tests** via `flutter test integration_test/`
- **Coverage gate** — fails if line coverage < 80%
- **Golden tests** — fails if rendered UI deviates from baselines
- **Performance benchmarks** via `flutter test --profile test/performance/`
- **Dependency vulnerabilities** — `flutter pub audit` fails on known vulnerabilities
- **Outdated dependencies** — `dart pub outdated` identifies outdated dependencies
- **Secret scanning** — `gitleaks detect --source .` identifies leaked secrets such as tokens

## Performance Benchmarks

Automated performance regression tests are located in `test/performance/`. To run:

```bash
flutter test --profile test/performance/
```

For end-to-end cache and memory benchmarks driven through the integration-test harness, run:

```bash
flutter drive --driver=test_driver/perf.dart --target=integration_test/perf_test.dart
```

Performance test driver: `test/performance/driver.dart` measures and asserts against documented thresholds for:

| Metric | Threshold | CI Assert |
|--------|-----------|-----------|
| Startup latency | < 2000 ms | `expect(duration.inMilliseconds, lessThan(2000))` |
| Frame build time (p50) | < 16 ms | `expect(p50Micros, lessThan(16000))` |
| Frame build time (p99) | < 32 ms | `expect(p99Micros, lessThan(32000))` |
| Memory (heap baseline) | < 50 MB | `expect(heapMB, lessThan(50))` |
| Memory (after 100 frames) | < 80 MB | `expect(heapMB, lessThan(80))` |

### Adding a New Benchmark

1. Create a test file in `test/performance/` (e.g., `test/performance/screen_bench.dart`).
2. Use `flutter_driver` with `enableFlutterDriverExtension()` and `measure()`:
   ```dart
   app.main();
   await driver.waitFor(find.byType(MyScreen));
   final frameTimes = await driver.traceAction(() async {
     for (int i = 0; i < 100; i++) {
       await driver.tap(find.byKey(Key('refresh_$i')));
     }
   });
   final stats = frameTimes.getSortedMetrics();
   expect(stats.percentile99th.inMicroseconds, lessThan(32000));
   ```
3. Run locally with `flutter test --profile test/performance/` to establish a baseline.
4. Commit the benchmark test alongside your code changes.

## Coverage Reports

After running tests locally, view coverage at:

```bash
open coverage/lcov.info   # or upload to Codecov manually
```

CI automatically uploads coverage to Codecov for trend tracking.

## Troubleshooting

### Tests fail after UI changes

1. Review the golden test diff in `test/golden/baselines/`.
2. If the new behavior is intentional, update baselines with `flutter test --update-goldens test/golden/`.
3. Commit the updated baselines alongside your code changes.

### Coverage below 80%

Add unit tests for uncovered paths. Focus on:
- Error handling branches
- Edge cases in business logic
- Utility functions

### Integration tests timeout

Check that all required services are running:
```bash
docker-compose up -d   # Start backend dependencies
```

## Quick Reference

| Command | Purpose |
|---------|---------|
| `flutter analyze` | Static analysis |
| `flutter test --coverage` | Unit tests + coverage |
| `flutter test integration_test/` | Integration tests |
| `flutter test --profile test/performance/` | Performance benchmarks |
| `flutter test --update-goldens test/golden/` | Update visual baselines |
| `./tool/rollback.sh --dry-run` | Show what rollback would do (safe) |
| `./tool/rollback.sh --force` | Restore clean state (destructive) |
| `flutter pub audit` | Check dependencies for known vulnerabilities |
| `dart pub outdated` | Identify outdated dependencies |
| `gitleaks detect --source .` | Scan for leaked secrets/tokens |

---

*Last verified: see the latest green run of [.github/workflows/verification.yml](../.github/workflows/verification.yml).*
