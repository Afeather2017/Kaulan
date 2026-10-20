# Rust CI checks

The release-input job in `.github/workflows/publish.yml` runs
`cargo clippy-ci -p kaulan -- -D warnings`. The `clippy-ci` alias in
`.cargo/config.toml` checks the backend library and binaries without linting
dependencies.

The async provider traits in `backend/src/file_ops/mod.rs` and
`backend/src/services/download.rs` use `async_trait`. Its expansion marks the
generated futures `must_use`; Clippy 1.99 also considers futures inherently
`must_use` and reports `double_must_use`. Those traits allow only that lint.
Other Clippy warnings remain errors in CI.
