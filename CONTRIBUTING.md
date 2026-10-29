# Contributing

Thank you for helping. Bug reports, fixes, new tests and documentation are all welcome.

## Reporting a bug

Open an issue with:
- the schema, the route and its extractor;
- the request (method, `Content-Type`, body or query string);
- the response you got and the one you expected;
- the versions of `livr-axum`, `livr`, `axum` and Rust.

A rule that validates wrongly is a bug of [livr](https://github.com/koorchik/rust-livr/issues), unless it only happens through livr-axum. The rules are defined by the [LIVR specification](https://livr-spec.org).

Security issues: see [SECURITY.md](SECURITY.md), not public issues.

## Making a change

livr-axum depends on livr by path: check out [rust-livr](https://github.com/koorchik/rust-livr) next to this repository, as `../rust-livr`.

```bash
cargo test
cargo +1.88.0 test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo build --examples
```

- **Tests:** every fix and every change of behavior comes with a test. The examples of the README are tested too.
- **Style:** comments explain why, in plain sentences.
- **MSRV:** the minimum supported Rust is in `Cargo.toml` (`rust-version`); CI checks it.

## License

By contributing, you agree that your contributions are licensed under the [MIT license](LICENSE).
