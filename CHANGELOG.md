# Changelog

## Unreleased

- Lakeside launcher and accounts: new `us-lakeside` crate (no UI) reads the launcher environment, redeems the handoff code once off the main thread (5 second limit, no redirects, never retried), offers optional in-game sign-in with the token in OS secure storage, and checks and downloads updates through Lakeside with size and SHA-256 verification. Updates are off when the launcher starts the game.
- Installers workflow: Windows Inno Setup, Linux AppImage and macOS dmg installers plus a launcher manifest, built on tags and on demand.
- 3D renderer: new `us-render` crate draws a generated town offscreen with wgpu in the cel look (three stepped tones, ink outlines, hard shadows from one sun), with the full building kit ported from the mockups, an orbit camera in metres, ground picking, and a `shots` example.
- Town generator connection fixes: country roads cut short by the river now end on a riverside drive (both presets, both banks), a ring road is no longer split at its start point, and a street group cut off from the network gets an entrance road under 150 m or goes back to farmland. New tests check the whole road network is one piece and no arterial ends in open land.
- Workspace skeleton: town generation crate and its command line tool.
- Town generator: `us-gen` builds a 4 km town from a preset (grid or winding) and a seed, in the approved pass 5 style (skeleton, additions with their own street patterns, pods, collectors, lots). Long addition-edge streets become collectors, pods always get an entrance and at least two cul-de-sacs, small downtowns get at most five towers. `us-gen-cli` renders a plan-view PNG and a stats file.
