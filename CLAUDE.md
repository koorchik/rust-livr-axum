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
PORT=3001 cargo run --example basic             # also runtime_schemas, custom_errors
```

## Architecture

- `extract.rs` - `Livr` (JSON body), `LivrQuery`, `LivrForm`, `schema` (a route that serves a schema); generic over `livr::Schema`, which `schema!` implements
- `middleware.rs` - `validate_json`, `validate_json_with::<E>` and `LivrValidated`, for schemas known at run time (`livr::Validator`)
- `read.rs` - `read_body`, `read_json`, `read_form`, `parse_json`: the reading of requests, public for other extractors and frameworks; the extractors and the middleware use them
- `rejection.rs` - `LivrRejection`: 415, 400, 413, 422 and 500 with the errors of the fields
- `urlencoded.rs` - `parse_urlencoded` (query strings and forms to JSON: repeated keys and `key[]` become lists; with `Schema::FIELDS`, list fields are lists and other keys are dropped) and the `Content-Type` checks

axum without default features: no `json` feature, responses are built by hand.

## Code style

`cargo fmt`, clippy without warnings, edition 2024, rust-version 1.88, like rust-livr. Comments explain why, in plain sentences. Every behavior change comes with a test.
