# Urban Sprawl (working title)

A Lakeside Games city game where the city builds itself. Private repository. Proprietary.

## Crates

- `us-gen`: town generation (road skeleton, additions, street patterns, lots). Deterministic from (preset, seed).
- `us-sim`: headless deterministic simulation (clock, growth reveal, construction, tax, abandonment, money).
- `us-render`: wgpu cel renderer, offscreen or into a window.
- `us-app`: the desktop game, binary `urban-sprawl`.
- `us-gen-cli`: plan-view PNG and stats from a preset and seed.

## Run

`cargo run --release -p us-app`. Releases are built by `.github/workflows/release.yml`
on tags `v1.0.0-preview.*` and are published as a GitHub pre-release.
