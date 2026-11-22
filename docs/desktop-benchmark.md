# Desktop frame pacing benchmark

Launch the desktop executable with these environment variables to grow a repeatable sandbox and measure it in the main window.

| Variable | Value |
| --- | --- |
| `URBAN_SPRAWL_DESKTOP_BENCH` | `20` measured seconds after setup |
| `URBAN_SPRAWL_WINDOW` | `1360x820` |
| `URBAN_SPRAWL_SMOKE_PRESET` | `winding` |
| `URBAN_SPRAWL_SMOKE_SEED` | `3` |
| `URBAN_SPRAWL_SMOKE_DAYS` | `7500` |
| `URBAN_SPRAWL_SMOKE_TEND` | `1` |
| `URBAN_SPRAWL_SMOKE_UI` | `clean,whole,speed:1` |

Run those settings at speeds `1` and `8`. Repeat each run with `,view:7` appended to show health coverage. Keep the window size, graphics options, adapter, and viewport fixed. The large-town setup runs before frame timing starts. Set `URBAN_SPRAWL_BENCH_SAVE` to a save file to reuse the exact same city across runs. The benchmark leaves autosaves alone. Compare the reported population and day as well as FPS and 95th-percentile frame time.

Move the cursor during the measured interval to sample the time from receipt of a cursor event to submission of the following drawn frame. Use the same input pattern between runs. This excludes input-device and display latency. With no cursor movements, the benchmark reports that there are no input samples.

Run a release build for comparable timings. An offscreen software-renderer run can catch rendering regressions and exercise the harness. Report its adapter separately from desktop GPU measurements.
