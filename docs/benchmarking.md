# Benchmarking

Benchmarks exist to protect the hot path that people and agents run during coding. They are not marketing numbers. Compare changes on the same machine, with the same target directory shape, and with the same benchmark command.

The CLI benchmarks spawn a real process, so scheduler noise can dominate short cases. Treat one Criterion regression flag as a prompt to rerun that focused case, not as a conclusion. Require three focused runs with the same direction before claiming a regression or improvement.

## Command

Run the full suite:

```sh
cargo bench --bench cli
```

Run a quick smoke check without collecting measurements:

```sh
cargo bench --bench cli -- --test
```

`benches/cli.rs#driftless_bin` resolves the binary under test, and `benches/cli.rs#ensure_release_binary` builds the current release binary when needed. That keeps `benches/cli.rs#bench_cli` from measuring a stale `target/release/driftless`.

## Current Local Baseline

These numbers came from one local run on July 5, 2026 after a warm build. They are useful for relative comparisons in this repo, not as cross-machine performance guarantees.

| Scenario | Mean time | What it covers |
| --- | ---: | --- |
| `check_200_refs` | 15.0 ms | Locked check of many Rust refs in one documented file. |
| `check_json_200_refs` | 15.0 ms | JSON mode overhead when refs are valid. |
| `check_mixed_120_refs_80_files` | 6.4 ms | Rust and Go parsing across many small files. |
| `check_cached_source_1000_refs` | 12.9 ms | Repeated refs to one file through `src/check.rs#CheckContext`. |
| `check_json_80_body_drifts` | 8.0 ms | JSON repair records when many refs drift. |

## Fixture Limits

The current fixtures are synthetic by design: small generated source files, deterministic temp repositories, and stable Markdown layouts. That makes regressions easier to attribute, but it does not cover every real repo shape.

Before claiming performance improvement, check whether the changed code affects one of these unmeasured paths:

- deeply nested docs with large enclosing sections
- many Markdown files with only a few refs each
- all supported languages, not only Rust and Go
- editor diagnostics through `src/lsp.rs#run`
- lockfile churn after large ref renames

When a change affects one of those paths, add a scenario to `benches/cli.rs#bench_cli` before relying on the benchmark table.
