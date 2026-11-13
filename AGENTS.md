# Contributing to Urban Sprawl

Instructions for anyone, human or AI agent, changing this repository. This repository is public:
everything committed is public and stays in the history for good.

## Never commit

- secrets, tokens, keys or signing material;
- names, handles or contact details of players or staff;
- internal hostnames, addresses, server paths or operations notes;
- anything you would not show the public. Design and planning notes are kept outside this repo.

## Build and test

- Toolchain: pinned in `rust-toolchain.toml` (Rust edition 2024).
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` must pass; CI runs the same (`.github/workflows/verify.yml`).
- GPU tests skip when no graphics adapter is available.
- Linux release builds are made on Ubuntu 22.04 so they run on current distributions; CI starts the
  Linux build the way the Lakeside launcher does and fails if it does not stay up.

## Rules for code

- Town generation and the simulation are deterministic from their inputs on every platform: seeded
  RNG only, the `libm` crate for transcendental functions, no hash-map iteration order in output.
- The game must always work offline. The Lakeside sign-in is optional; nothing in `us-lakeside`
  may block the game or be needed to play.
- Never close silently: a start failure shows the player a message saying why.

## Changes

- Branch from `main`, open a pull request, squash-merge. Never push to `main` directly.
- Conventional commit messages (`feat:`, `fix:`, `ci:`, `docs:` and so on).
- Every pull request adds a line under `## Unreleased` in `CHANGELOG.md`.
- Before adding a workflow that uses repository secrets (signing, uploads), make sure no secret is
  exposed to pull requests from forks.
- Builds of the game are never attached to GitHub releases; the game is sold through the Lakeside
  launcher.
