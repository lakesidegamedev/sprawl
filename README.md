# Urban Sprawl

A city game from Lakeside Games where the city builds itself. You found a town on an empty map,
then run it as the mayor: taxes, the budget, schools, services, the big roads and bridges. Streets,
neighbourhoods and shops grow on their own, in a cel-shaded 3D look.

**This is an unreleased preview.** It is incomplete and will change a lot. Urban Sprawl is a paid
game: the source is shown here so people can follow its development, but it is not free software
and it is not for redistribution. See [LICENSE.md](LICENSE.md). Players get the game through the
Lakeside launcher.

## Building from source

You need the Rust toolchain pinned in `rust-toolchain.toml` (rustup installs it automatically).

```
cargo run --release -p us-app        # the game
cargo test --workspace               # tests (GPU tests skip when no graphics adapter is found)
cargo run -p us-gen-cli -- --preset grid --seed 1 --png town.png --stats town.json
```

On Linux the game needs a Vulkan driver (on Ubuntu, `mesa-vulkan-drivers`) and the usual X11 or
Wayland development packages to build.

## Crates

- `us-gen`: town generation (road skeleton, additions, street patterns, lots). Deterministic from
  a preset and a seed.
- `us-sim`: headless, deterministic simulation (game clock, growth, construction, tax, money).
- `us-render`: the cel renderer (wgpu), offscreen or into a window.
- `us-app`: the desktop game, binary `urban-sprawl`.
- `us-lakeside`: optional Lakeside launcher sign-in and updates. The game never needs it to play.
- `us-metrics`, `us-traffic`, `us-grow`: road realism measures, traffic routing and
  traffic-driven road growth.
- `us-gen-cli`, `us-grow-cli`: command line tools that draw towns and traffic as images.
