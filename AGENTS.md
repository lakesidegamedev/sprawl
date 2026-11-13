# Urban Sprawl agent instructions

Read `/root/AGENTS.md` and `/root/.claude/CLAUDE.md` on this host first.

## Authority

- Design: ops-knowledge doc `sprawl-plan` (collection docs). Design docs live there, never in this repo.
- Owner decisions override the doc. Record substantive changes on the doc, not here.
- Kanban: card 255320 (ops-knowledge `work_report`, actor `claude:sprawl` or your worker name).
- MetroForge is deprecated: do not copy from it or reference it.
- Never commit credentials, signing keys, personal data or real people's names or likenesses.

## Road and town generation

The rules are proven in a Python mockup that is the reference implementation:
`/root/misc/archive/2026-11-13-sprawl-plan/` (`sprawl5.py` is current; it imports `sprawl3.py`
and `sprawl4.py`; `osm/` holds the real-town study). The owner approved pass 5 on 2026-11-13.
Port behaviour, not code style. Known gaps the owner was told would be fixed in the port:
long addition-edge streets become collectors; pods get more internal cul-de-sacs; small towns
get few towers downtown.

Generation must be deterministic from (preset, seed) on every platform: no HashMap iteration
order in output, use the `libm` crate for transcendental functions, seed-based RNG only.

## Development

- Edition 2024, toolchain pinned in `rust-toolchain.toml`.
- Every PR adds a line under `## Unreleased` in `CHANGELOG.md`.
- Feature branches from `main`, squash-merge PRs. Never push to `main` directly.
- Worktrees at `worktrees/<branch>`. Stage explicit files. Finish clean.
- This host runs production services. Local checks: `cargo test -p us-gen -j 4` and
  `cargo clippy -p <crate> -j 4` are fine. GPU crates (wgpu) build on hosted CI, not here,
  unless the owner says otherwise. Run commands in the foreground; never background a waiter.
