# Changelog

## Unreleased

- Traffic shapes the roads (phase 1): `us-metrics` measures how real a road network is (block sizes, junction mix, dead ends, orientation spread, circuity, class shares) and scores generated towns against statistics from real towns; `us-traffic` routes peak hour trips on a town with user equilibrium assignment; `us-grow` upgrades streets over capacity and adds connectors where the drive is long, behind a proposer trait and a scorer trait. `us-grow-cli` draws traffic before and after growth.
- Town generator connection fixes: country roads cut short by the river now end on a riverside drive (both presets, both banks), a ring road is no longer split at its start point, and a street group cut off from the network gets an entrance road under 150 m or goes back to farmland. New tests check the whole road network is one piece and no arterial ends in open land.
- Workspace skeleton: town generation crate and its command line tool.
- Town generator: `us-gen` builds a 4 km town from a preset (grid or winding) and a seed, in the approved pass 5 style (skeleton, additions with their own street patterns, pods, collectors, lots). Long addition-edge streets become collectors, pods always get an entrance and at least two cul-de-sacs, small downtowns get at most five towers. `us-gen-cli` renders a plan-view PNG and a stats file.
