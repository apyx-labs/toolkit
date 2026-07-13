# Apyx Toolkit

Open-source Rust crates from [Apyx](https://github.com/apyx-labs).

## Crates

| Crate | Path | Description |
|---|---|---|
| `apyx-reqwest-middleware` | `crates/apyx-reqwest-middleware` | reqwest middleware: header auth, URL rebasing, Prometheus client metrics |
| `dune-client` | `crates/dune-client` | Client for the [Dune Analytics API](https://docs.dune.com/api-reference/overview/introduction) |

## Usage

Crates are consumed as git dependencies pinned to a tag:

```toml
[dependencies]
dune-client = { git = "https://github.com/apyx-labs/toolkit", tag = "v0.1.0" }
```

## Development

```bash
cargo test -p <crate>        # single crate
cargo ci-test                # full suite (cargo-nextest)
cargo +nightly fmt --all     # format (nightly rustfmt; unstable options)
cargo ci-clippy              # lint, warnings denied
cargo ci-sort --check        # Cargo.toml ordering (cargo-sort)
taplo fmt --check            # TOML formatting
cargo ci-audit               # dependency advisories (cargo-audit)
cargo +nightly ci-udeps      # unused dependencies (cargo-udeps)
```

## Disclaimer

All crates in this repository are provided **as-is**, without warranty of any kind,
express or implied, including but not limited to merchantability, fitness for a
particular purpose, or non-infringement. Apyx makes no guarantees regarding
correctness, security, availability, or suitability for any use case.

By using or depending on these crates, you assume **all liability and risk**
arising from that use, including any direct or indirect damages, data loss, or
operational impact.

## License

Apache-2.0
