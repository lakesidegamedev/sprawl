# Urban Sprawl

A city-building game from Lakeside Games. In Town Growth, run a town that builds its own streets.
In City Planning, lay out roads and zone land yourself. Manage taxes, budgets and public services
as neighborhoods grow in a 3D world with changing seasons, weather and traffic.

Urban Sprawl is a paid game: the source is shown here so people can follow its development, but
it is not free software and it is not for redistribution. See [LICENSE.md](LICENSE.md). Players
get the game through the Lakeside launcher.

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
