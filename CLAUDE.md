# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Overview

`livr-axum`: axum extractors for LIVR 2.0 validation. It depends on `livr` by path, from [rust-livr](https://github.com/koorchik/rust-livr) checked out next to this repository (`../rust-livr`); `cargo publish` uses the version instead.

## Commands

```bash
cargo test                                      # tests, examples, doctests (the README too)
cargo +1.88.0 test                              # the lowest supported Rust
cargo clippy --all-targets -- -D warnings && cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo build --examples
cd ../rust-chista-examples && cargo test   # the backend examples use read.rs and urlencoded.rs: check them after API changes
PORT=3001 cargo run --example basic             # also runtime_schemas, custom_errors
```

## Architecture

- `extract.rs` - `Livr` (JSON body), `LivrQuery`, `LivrForm`, `schema` (a route that serves a schema); generic over `livr::Schema`, which `schema!` implements
- `middleware.rs` - `validate_json`, `validate_json_with::<E>` and `LivrValidated`, for schemas known at run time (`livr::Validator`)
- `read.rs` - `read_body`, `read_json`, `read_form`, `parse_json`: the reading of requests, public for other extractors and frameworks; the extractors and the middleware use them
- `rejection.rs` - `LivrRejection`: 415, 400, 413, 422 and 500 with the errors of the fields
- `urlencoded.rs` - `parse_urlencoded` (query strings and forms to JSON: repeated keys and `key[]` become lists; with `Schema::FIELDS`, list fields are lists and other keys are dropped) and the `Content-Type` checks

axum without default features: no `json` feature, responses are built by hand.

## Release

`livr` must be on crates.io first: `cargo publish` (and its `--dry-run`) resolves `livr` by its `version`, not by the path, so a version of livr that is not published yet fails to resolve. `[patch.crates-io]` does not help, since `cargo package` ignores it.

1. If the release needs a new livr, publish rust-livr first (its CLAUDE.md), then raise `livr = { ..., version = "x.y.z" }` in `Cargo.toml`. A 0.2 of livr is a breaking change for users of `livr::Schema`: release a new minor version of livr-axum too.
2. Bump `version`, add it to `CHANGELOG.md`, run the commands above and `cargo publish --dry-run`.
3. Commit, tag `vX.Y.Z`, push, create a GitHub release from the CHANGELOG entry, `cargo publish`.
4. Run the tests of `../rust-chista-examples`, which uses the helpers of `read.rs`.

## Priorities

A convenient public API comes before small speedups. Big measured wins that keep the API are welcome.

## Code style

`cargo fmt`, clippy without warnings, edition 2024, rust-version 1.88, like rust-livr. Comments explain why, in plain sentences. Every behavior change comes with a test.
