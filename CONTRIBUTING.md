# Contributing to Hush

Thanks for helping. Hush is early, so an issue that describes a real problem is as useful as code.

## Before you start

- Read `AGENTS.md` for the crate layout and the rules that keep Hush fast and out of the user's
  way, and `CONTEXT.md` for the domain words.
- For anything bigger than a fix, open an issue first so we can agree on the shape.

## Build and check

You need the Rust toolchain pinned in `rust-toolchain.toml` (rustup installs it). Platform code
needs that platform to build and test.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same commands on every pull request.

## Pull requests

- One change per PR, small enough to review in one sitting.
- Say what changed for the user and how you checked it. Include timings or token counts when the
  change touches speed or output size.
- Tests should fail on a real regression. Don't add tests that only restate the code.
