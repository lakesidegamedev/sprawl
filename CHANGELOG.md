# Changelog

## Unreleased

- Workspace skeleton: town generation crate and its command line tool.
- Town generator: `us-gen` builds a 4 km town from a preset (grid or winding) and a seed, in the approved pass 5 style (skeleton, additions with their own street patterns, pods, collectors, lots). Long addition-edge streets become collectors, pods always get an entrance and at least two cul-de-sacs, small downtowns get at most five towers. `us-gen-cli` renders a plan-view PNG and a stats file.
