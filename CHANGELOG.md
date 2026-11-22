# Changelog

## Unreleased

- Mesa sandstone has broader mineral layers, chipped cliff faces and close-up sand ripples. Hills have more exposed soil, and fine terrain and snow detail fades smoothly as you zoom out. Shadow-map edges blend into sunlight, and sandstone no longer picks up the roadworks stripe pattern.
- Shallow road merges share one paved apron where the lanes overlap. Kerbs, snowbanks and competing lane lines stay outside the shared asphalt. New towns ease short collector hooks into arterials with a curve.
- Desert towns get dry winters and occasional rain. Old desert saves reopen without winter snowpack or road snow.
- Downtown roadside shops read as part of the street wall. Vacant central retail sites and parking plots between developed lots can fill with shops, while outer shopping areas keep their large-format stores.
- Coverage and building-level maps show standing structures. Other overlays keep vacant sites flat. Analysis views use simpler lighting and reuse overlay meshes while map colours stay the same.
- The People graph shows estimated workers and workplace construction by sector. Empty lot details explain whether new development is waiting on service capacity or arrivals.
- Building and road catalogs collapse to placement controls while you work on the map. Legal sites stay where you aim. A conflicting planned street is highlighted, with a message that explains the reservation.
- Balanced graphics reduce the city's render resolution during sustained slow frames. Interface text and controls stay at native resolution. The game also skips repeat street, woodland and lot updates between simulation changes.
- Winter terrain keeps its sun shadows and wind-shaped surface detail with fewer screen-space shading artifacts. Hills have more varied grass, exposed soil and rock.
- Big-box sites include larger stores, two-anchor shopping malls and warehouse retailers with pickup bays, with shallower parking lots.

## 1.6.0 Preview 1 (2026-11-21)

<!-- Release coverage starts at 92cbdff. -->

### For players

- Select a junction on the map to inspect its incoming streets. The inspector shows each approach's signal and the time until green. When a peak-hour estimate is available for that junction, it compares arrivals with capacity and flags likely queues.
- Switch between the traffic map and the city view without closing the inspector.

### For developers

- Read live signal phases from the same clock and junction geometry used to animate vehicles. Show peak-hour estimates only when their recorded stop position matches the selected junction.
- Desktop preview installers cover Windows x64, Linux x64 and universal macOS. The preview uses the existing save format.

## 1.5.2 (2026-11-21)

<!-- Release coverage: 0e9e82b..6af80f8. -->

### For players

- Roads snap to nearby open streets and offer an optional 45° angle guide.
- Junctions keep their paving, curb cuts and lane markings where streets meet. At major intersections with room for signals, cars wait at red and take turns crossing or turning. Signal delays count toward congestion.
- On iPhone and iPad, the shorter Build picker gives way to a compact toolbar after you select a road. Check the cost, undo a point or start construction with the map in view.
- Automatic growth fills blocks and connects local streets between main roads.
- Clouds are visible from farther out. Mist clears for close inspection, and cloud shadows drift gently across the map.
- The app icon has a new hand-plotted 32 × 32 pixel skyline.

### For developers

- Use the same junction geometry for paving, markings, stop lines and protected approaches. Keep grade-separated roads apart and assign traffic to each road run in a contracted street.
- Account for signal timing in directional capacity, route costs and road widening estimates.
- Prefer connected local blocks when selecting the next street for automatic growth. Existing saves retain their roads and terrain.
- Rebuild the packaged RGB icons, SVG marks and window RGBA icon from the 32 × 32 artwork.
- Regressions cover edge snapping, routing across a new connector, a shared-vertex crossing, a red-to-green turn, a grade-separated junction and close and regional cloud rendering. The iOS simulator workflow captures the phone road controls. (#69)

## 1.5.1 (2026-11-20)

<!-- Release coverage: f895924..ea2e222. -->

### For players

- Buildings outside the camera view cast shadows into town. Junction paving follows roads as they bend.
- Newly generated coastal towns have gentler waterfront roads through headlands, with room around the lighthouse. Existing saves keep their original terrain and roads.
- Phones and tablets can enable Detailed atmosphere and surfaces. Balanced resolution now works with Detailed while keeping the interface sharp.

### For developers

- Extend shadow-map depth for offscreen casters without lowering ground resolution. Limit rounded junction pads to straight approach lengths.
- Generator version 4 grades coastal roads and preserves version 3 maps for existing saves. Lighthouse placement clears the road and verge.
- Exercise Detailed storm rendering at town scale on iPhone and iPad simulators, plus a close phone view. Physical-device frame rate, thermal and battery costs remain unmeasured. (#67)

## 1.5.0 (2026-11-19)

<!-- Release coverage starts after 1.4.3 (1fd1e63) and includes the unreleased building work in d6fa8f0. -->

### For players

- The countryside is alive. Forests, pine woods, copses and lone trees stand on the open land around every town, in a mix that suits each landscape. The town fells them where it builds, and land opened for building keeps a few old trees between the houses.
- Downtown reads as a city. Buildings stand on the lot line and join into street walls with shopfronts, awnings and signs. Towers come in glass, stone, brick and concrete with varied setbacks, podiums and rooftops. Height follows the size of the town: a small town has a modest centre, and landmark towers arrive as it grows, tallest in the middle and stepping down to the edge.
- The middle of town fills in first. Streets of the old core near the town hall are built before the town reaches outward, so the centre is no longer a ring round empty blocks.
- Lighting has depth. Shadows have soft edges, sunlit faces are warm and shade is cool, and buildings, kerbs and trees darken the ground where they meet it. With a low sun the air glows where the light reaches and dims behind towers and hills.
- Winter looks like winter. Broadleaf trees lose their leaves over late autumn and stand bare with thin shadows, conifers stay green under snow, the sun sits low, roads are ploughed with banks along the kerb, and snow lies on every roof and ledge.
- Night has shape. Fewer windows are lit, in warm and cool colours and more in homes than offices, roofs fall back into the dark, street lamps light the roads and red beacons mark tall towers.
- Roads sit better in the land. Main junctions have crossings and stop lines at their mouths, planted medians end before the junction with a rounded nose, two roads that part at a narrow angle share one paved area instead of being drawn over each other, and main roads have a gravel shoulder. Lamps, benches and people keep off the carriageway.
- Harbours make sense. Piers start at the water's edge, boats are moored along the piers and breakwaters, breakwaters grow out of the shore and the beach varies in width. The land no longer ends in a visible edge: it fades into haze at a flat horizon.
- Glass, roofs and walls have materials. Windows are dark glass with sky light, a glare and the odd drawn blind, flat roofs show gravel and seams, and walls weather.
- Open plots stay open. Land the town has not reached is left as it is, and only some plots in the true centre get an interim use such as a small square, a fenced building site or a car park.

### For developers

- Plant woodland from the town seed in `woods.rs`, fell it for visible road runs, laid-out lots, the placed hall and stations, thin it in opened building land, and redraw a few chunks a frame. A reopened town has the same woods as before it was saved.
- Replace the three wall tones with a continuous tone and warm and cool tints, filter sun shadows (one comparison on the light preset), add two bands of screen-space contact shade behind the Shadows setting, average foliage normals, and march the shadow map along the view ray for sun shafts in Detailed.
- Draw bare branches under every broadleaf crown and collapse leaf vertices by season in the main and shadow vertex stages. Snow uses a world-up mask.
- Build downtown from street-wall blocks and tower profiles with a silhouette repeat cap, and take height from population, lot level and distance from the centre.
- `Sim::pick_street` builds planned streets of revealed core neighbourhoods within 320 m of the hall before anything else. Save format and generator version are unchanged.
- Junction pads reach to where narrow-angle kerbs part, within the straight part of each arm, and markings are placed only on that straight part. Street furniture and pedestrians are filtered off carriageways.
- The fixed mesh fingerprint in `tests/render.rs` changed with the foliage normals. Frame rate on phones was not measured for this release.

## 1.4.3 (2026-11-18)

<!-- Release coverage starts after TestFlight 1.4.2 (8042afe) and includes #62 (023b018). -->

### For players

- Clouds drift across the city and change shape as they move. Light passes through their thinner edges, and ground fog stays visible when you zoom into the streets.
- Water rolls in irregular swells. Small ripples soften into the surface as you zoom out, reducing the stripes and shimmer on distant water.
- Storms use local lightning and softer shade. Fog and lighting cost less to render, and Balanced world resolution is now the default. Graphics settings include Native and, on desktop, Detailed.
- Tap a semicircle gauge to build or upgrade water, power, schools or health services. Each mobile sheet remembers its own scroll position and fits above the dock with a fixed close button.

### For developers

- Advect a tiled 3D cellular density field with integrated wind and separate flow for wisps. Remove the duplicate sky projection, weight light at the visible density, and filter fine detail and sample spacing for shallow rays.
- Reuse adjacent density samples, reconstruct the half-resolution atmosphere against scene depth, and cap the cost of fallback rays along building edges. Reduce terrain history in thick weather and keep lightning local to the strike.
- Cull instances outside the camera and shadow bounds, select simpler meshes at wide standard-quality zoom, and skip negligible sun-shadow work. Filter distant water normals by their screen footprint and upscale the Balanced world beneath the native UI.
- Measure rendered frames against wall time, keep weather animation responsive during slow frames, and cover cloud travel at constant cover in a renderer regression test. This release uses the existing save format.

## 1.4.2 (2026-11-18)

<!-- Release coverage starts after v1.4.1 (a577645). Atmosphere improvements are in #61. -->

### For players

- Zoom into the streets to clear the overhead cloud cover. Ground fog stays visible in foggy weather, and clouds still shade the city.
- Clouds have rounded billows, brighter sunlit edges and cooler shadowed interiors. The rounded shapes have fewer visible bands and cleaner edges at shallow angles and on narrow screens.
- Breaks in stronger storms keep streets and buildings easier to see at planning zoom.

### For developers

- Shape cloud density with overlapping 3D billows and rotated erosion noise. Integrate paired ray samples with depth-aware reconstruction and light the density-weighted sample position to reduce banding in dense cloud edges.
- Fade only the overhead cloud volume on ground-facing rays as the camera approaches street zoom. Keep the ground fog, weather state and cloud shadows independent of that fade.
- Capture a seeded coastal storm through the normal game surface for simulator verification.

## 1.4.1 (2026-11-18)

<!-- Release coverage starts after v1.4.0 (60bd560). Includes the city and renderer overhaul in #60. -->

### For players

- Watch storms roll in. Low clouds build before rain, gusts push heavier rain across parts of town, and streets stay wet while the sky clears. Nearby surfaces and cloud banks light up around a strike, and distant thunder takes longer to arrive. Reduce Motion turns off visible lightning flashes.
- Ground fog fills hollows and moves between buildings. Sun breaks light up parts of the fog, clouds cast moving shadows, and distant terrain fades naturally. Water, wet roads and glass pick up their surroundings.
- Detailed desktop graphics reduce shimmer while moving the camera and add reflections, lighting and shadows around small objects. Headlights and lit signs have a soft glow. An optional mild tilt shift softens the far edges while keeping the center of the view sharp.
- Seasonal changes spread across the map over nine game weeks. Snow melts and accumulates gradually. Meadows have more varied vegetation and soil, and cliff faces, sea stacks and arches have more natural stone and silhouettes.
- There are 132 seeded building variants, from manufactured homes and warehouse lofts to Art Deco towers, 28-story offices and landmark spires. Tall buildings grow in established neighborhoods with strong services and high land value.
- Upgrade civic buildings in place through three tiers. The new capacity bar brings up water, power, school or health as demand approaches its limit.
- Start a town in three short steps with an actual in-game preview for each map. Larger mobile menus give the controls room. The mayor campaign now runs through three chapters, from founding a town to serving a regional city.

### For developers

- Integrate visual winds across frames and vary storm cover and precipitation spatially. Smooth persistent surface wetness separately from active rain, and use the forecast to bring cloud cover in ahead of a storm. Seed lightning position and schedule the matching stereo thunder by distance. Keep visual weather outside the deterministic simulation and save data.
- Integrate terrain-relative fog and world-space clouds against the scene depth, with directional light and shadow samples in both volumes. Add sky lighting, distance scattering and projected road weathering.
- Pack material and surface normal data into the existing outline attachment. Trace screen-space reflections against scene depth and blend directional local radiance probes as a fallback. Use depth and surface normals for nearby ambient occlusion and contact shadows.
- Resolve the world into a floating-point render target with highlight compression and selective bloom before drawing the UI. Detailed mode uses subpixel camera jitter, previous-frame reprojection and depth rejection, with resolution and scene changes invalidating history.
- Extend civic levels and campaign goal bits through existing save fields. Store custom town names in an optional save trailer. Existing towns keep their geometry and use the new upgrades and growth rules as they play.

## 1.4.0 (2026-11-18)

<!-- Release coverage starts after the 1.3.2 TestFlight source 5561e3c. -->

### For players

- Choose Town Growth to run a town that builds its own streets, or City Planning to draw local roads, collectors and arterials yourself and zone roadside land.
- Mist rolls along the ground on damp mornings. Rain and snow move through the world and pass behind buildings and hills. Seasonal colours change over six game weeks, with nearby areas turning at different times.
- Windows light up gradually across neighborhoods at dusk and keep a steady pattern. Streetlights and headlights cast light on nearby roads and buildings.
- Wind ripples the water, with shallows and foam along the shore. Grass blends into exposed ground on slopes, and dense downtowns get taller office and apartment towers with distinct silhouettes.
- Hear traffic, wind, rain and building cues, with original day and evening music. Adjust music, ambience and effects separately.
- Roads stay visible at a distance and on bridge approaches. Traffic on Gorge follows one-way ramps and keeps flyovers separate from the roads below.
- Unlimited Money removes spending limits. Existing sandbox towns can toggle it from the pause menu.
- Compact layouts give the map more space. Balanced world resolution reduces 3D rendering work while keeping the interface sharp.

### For developers

- Add terrain-relative, depth-aware fog and world-space precipitation with scene occlusion. Use stable anchors for window lighting and local lights for streetlights and headlights.
- Preserve newer detail on water, slopes and dense buildings while retaining distance-based rendering for far geometry. Skip readbacks on the native window path.
- Store manual road and zoning state in the optional PLAN save section. Preserve prior save defaults and keep road direction and grade consistent across assignment, traffic animation and service routing.
- Use the existing TestFlight distribution identity and app provisioning profile. Windows packages use the matching workspace version.

## 1.3.2 (2026-11-18)

<!-- Release coverage starts after the 1.3.1 TestFlight source ca827a4. Includes civic placement and opening balance from 2937362. -->

### For players

- Traffic builds in the morning and reverses for the evening commute. Cars slow, queue and move through busy junctions in separate lanes, then spread back out after rush hour. Closed, flooded and unfinished roads stay clear of traffic.
- Reopening a saved town restores its local streets, junctions and neighborhood ground.
- Streets have clearer arterial markings, textured asphalt and paving, larger tree canopies, more roadside planting and more detailed grass, roofs and brick. Rail crossings show sleepers and brighter railheads.
- Season colours change over four game weeks. Rain follows the wind and ripples in puddles. Roads stay wet as the sky clears.
- Civic buildings snap beside open streets. The placement preview shows where the building will go before you pay.
- Towns start with more water, power, school seats and clinic capacity. Water towers, power plants, schools and clinics cost less to build. Buildings reopening or rebuilding reserve capacity just like new construction.

### For developers

- Build revealed neighborhood meshes from saved state, preserve the saved generator output on load, and track visibility separately from construction progress. Keep the Hard opening migration wave within its existing growth bound.
- Carry directional vehicle flow, capacity and the downstream junction from the peak-hour solver to the live renderer. Sample a daily demand curve against road storage and discharge capacity. The traffic map continues to show peak-hour load.
- Preserve directional approach data for the current and pending traffic assignment in the optional TRAF save section. Older saves use their stored aggregate traffic until their next assignment. Town generation is unchanged.
- Use world-space, distance-filtered materials and the existing weather state to drive the rain pass. Preserve the renderer's existing outline attachment, chunk culling and shadow paths.

## 1.3.1 (2026-11-17)

<!-- Release coverage starts after v1.2.0 (386b2cb). Includes the 1.3.0 candidate (2995c92). -->

### For players

- Smoother shadow outlines while panning and zooming. The renderer skips town geometry entirely outside the camera and shadow views, and uses less graphics memory for outlines on supported devices.
- Phones and tablets keep Build, Budget, Map and News within one tap, with other tools under More. The shorter dock leaves more room for the town and sheets stay above it. Budget stacks department controls vertically on small screens.
- Goals and town updates sit in separate columns so alerts no longer cover them. Open Taxes when needed to keep more of the map visible.
- Town hall placement works only on visible crossroads markers. Tapping elsewhere leaves the town unstarted.
- Fix the camera on steep maps so the view follows the terrain while moving, lifts over ridges that block the town, and lowers again over open ground.
- On compact screens, choose a site and review its cost or placement problem before committing to a service building. Panels keep touch input from reaching the map behind them.
- Budget and Build now show the same committed capacity as the growth warning. A warning shows the funding shortfall and opens Bonds when the town cannot afford the fix.
- First-year goals open the Budget and Ordinances panels directly.
- Bridge sides and parapets follow bends in the road without leaving gaps.
- Water ripples blend between close and distant views, and stronger winds brighten the crests.

### For developers

- Use a two-channel outline attachment where the adapter supports it at the selected sample count. Keep full-precision distance and face IDs and retain the wider attachment as a fallback.
- Cull chunks against all six clip planes with wind-sway bounds. Filter shadows at close zoom, stabilize the shadow grid during camera movement, and skip redundant shadow and contact-shade reads. Terrain, simulation and save formats carry forward from 1.3.0.
- Includes the steep-terrain camera and mobile interaction changes in #45 and #46.

## 1.2.0 (2026-11-16)

- Version 1.2.0. Shown version is 1.2.0.
- Four new town styles: River gorge (a winding gorge with layered rock walls, high bridges at rim height and a switchback road down to a riverside flat), Red rock (sheer banded mesas, buttes and spires with the town wrapped round them, distant mesas on the horizon), Mountain valley (a town on the valley floor between sharp peaks with forest, broken granite and snow caps; the camera lifts so a ridge never hides the town) and Coastal cliffs (high cliffs, a natural harbour bay with breakwaters, piers, moored boats and a lighthouse, eroded sea stacks and an arch, a sea that runs to the horizon). Each has its own weather, sea and rock views raise land value, low harbour ground and desert washes flood in storms.
- A livelier start: builders start on day one, settlers fill a new town through its first year, and a town left alone reaches about 650 to 1,000 people in a year instead of about 300.
- Growth walls you can see: when water, power, school seats or clinics run out, one banner under the top bar says what stopped and what to build, with the price and a button that opens Build with it picked; a slimmer warning shows before it is full. Growth now stops exactly when a capacity is full, counting buildings under way.
- First goals: a card of six steps for the first year (300 people, a water tower, a school, a tax or budget change, 1,000 people, an ordinance or district) with small grants or extra settlers as rewards, shown in the Budget window. Saves keep it in section `GOAL`; older towns open with the goals they already met.
- Fix: one thunderstorm could start dozens of fires and burn a third of a town without a fire station. A storm now starts at most a few fires, fire spreads less, and the first fire in a town with no station says so. A town left blocked for months loses a little approval.
- Ground and water: grass tufts, bushes, boulders, flower clumps, reeds, fallen logs and hay bales; farm fields with crop rows along each field, several crops, hedges, stone walls, fences and tree lines; broader meadow colour; animated water with ripples, shallows and moving shore foam; cooler shade and a darker step where things meet the ground.
- Buildings and streets: floor trim, side windows, roof units, vents, water tanks and roof gardens, ridge caps, shutters and lit windows that vary per building and thin out late at night; street trees along opened streets, house yards with driveways, paths, shrubs and hedges, downtown paving, shop forecourts with benches, planters and cafe tables, depot aprons with trucks.
- No more grey slabs: downtown, industry and mall land stays natural ground until something is built, each built lot gets its own paving, building sites show bare earth, a paved town square with a fountain surrounds the town hall, and downtown lots away from a street get a footpath.
- Saved towns keep their buildings and generator; the five older town styles are unchanged.

## 1.1.0 (2026-11-15)

- Version 1.1.0, a big update. Shown version is 1.1.0.
- Interface 2.0 in the Slate look: tool rail, inspector, icons, sentence case text, smooth motion and a Reduce motion setting.
- Roads look like roads: junctions with rounded kerbs, sidewalks, zebra crossings, lane markings and planted medians; new towns also get roundabouts, highway ramps, fewer dead ends and streets that bend with the hills.
- More models: about 25 new buildings (supermarket, bank, hotels, cinema, office park, car dealership, diner, petrol stations, corner store, pharmacy, townhouse rows, mid-rise, glass towers, duplex, mobile homes, factories, tank farm, freight yard, port cranes, recycling centre and civic look-alikes), city buses, delivery and garbage trucks, pickups and vans that run in numbers set by the town's size, traffic lights, bus shelters, billboards, benches and bins, and poplar, oak and pine street trees. New calls: `kit_more::variant`, `fleet::{Vehicle, Prop}`, `Scene::set_population`; the `modelsheet` example.
- Fires grow through smoke, flames and full fire, spread with wind and dry weather, and are answered by fire engines, police cars and ambulances that drive the real roads; response time decides how much burns.
- Construction you can watch: new buildings and rebuilds go up in 12 stages (fenced plot, foundation, frame inside scaffolding, walls floor by floor, tidy-up) with tower or mobile cranes, rebuilds start with the old building coming down, trees grow back after the job, day and night change smoothly (windows, street lamps and car lights come on gradually, cars gain tail lights), and tall jobs make news naming the place. New: `site` module, `building_site_for`, `Calendar::{darkness, lamps}`, `Sim::place_phrase`.
- Weather: seeded rain, snow, thunderstorms, heat waves and blizzards with a day-ahead warning; floods, lightning fires, power cuts, plowing and storm repairs; storm drains, building codes and snow plowing ordinances.
- Tram lines: link two to six transit stops into a line in the new Trams window; the line follows open arterials and collectors, costs $520 a metre and upkeep every month, is built in stages (track, then wires) and then runs two and three section trams both ways at a pace set by population and the roads budget; open lines take traffic off the streets beside them (and lift approval), raise land value near stops, bring a few more people and a little shop demand, with news when a line opens. Saves keep lines in section `TRAM`. New calls: `Sim::tram_stops`, `check_tram`, `build_tram`, `remove_tram`, `tram_lines`, `tram_rate`, `trams_open`, `trams_version`, `TramPlan`, `TramLineView`, `TramState`, `TramError`; `trams` in us-render and the `tramsheet` example.
- Slip lanes: highway ramps are one-way single lanes that leave or join the highway in a long tapered merge, with white edge lines and arrows, and the highway's edge line breaks along the merge. All roads use one asphalt colour.
- Ground texture: grass, fields, sand, dirt and snow have stepped patches and tufts up close instead of flat colour; snow settles in rounded drifts.
- New towns use generator version 3 (roundabouts, ramps, through collectors) and always have at least two downtown towers. Saved towns keep the generator they were made with.
- Saves keep fires in progress (`FIRE`), the weather (`WTHR`) and tram lines (`TRAM`) in their own sections; older saves open with all three at their defaults.
- Fix: on phones, the Districts and Ordinances sheets could close as soon as they opened.
- Balance note: with the new road layout towns run their services a little cheaper, so the default-tax money check now allows a surplus up to 45 percent of income (was 40).

## 1.0.1 (2026-11-15)

- Fix: on a wide window with the signed-in name and frame rate showing, the top bar drew the election date under the speed buttons. The buttons on the right are now laid out first and the town's figures get the room that is left: the small trend charts are left out, then the election date moves into the approval meter's tip, and nothing is ever drawn under a button. Phones get the same rule in both rows, the approval meter sizes its words to the room it has, and the date shows month and year.
- Fix (iOS): the top bar and the bottom dock sat under the Dynamic Island and the home bar. The interface now lays out inside the safe area the phone reports, and the strips above and below it are filled in the bar colour so the bars still reach the screen edge.
- Shown version is 1.0.1.

## 1.0.0 (2026-11-15)

- Fix: map views (land value, use, cover, demand, traffic, districts) showed no colours on hilly ground since preview 5, because the coloured blocks were drawn under the hills. They now stand on the ground like the buildings. The Build window lists the road tools first, one line per supply, and tall windows use more of the screen before scrolling.
- Version 1.0.0, the first full release. Shown version is 1.0.0; the help strip and the download's README list every panel key (I districts, H ordinances, R rival towns, Y roads).
- Districts and ordinances: group neighbourhoods into up to 12 named, coloured districts (Districts window, paint tool, Districts map view with each district's people, jobs, buildings, land value, building level, tax, fees and biggest problem) and switch on 10 ordinances for the town or a district (height limit, downtown upzoning, heritage protection, smoke alarms, neighbourhood watch, new business tax break, tourism campaign, clean industry rules, traffic calming, recycling). Effects show in the why panel and voter breakdown, fees show in the budget, and challengers can promise a missing ordinance. Saves keep it in section `DIST` (no format change). New calls: `Ordinance`, `OrdScope`, `District`, `DistrictStats`, `Sim::districts`, `create_district`, `toggle_addition`, `set_ordinance_town`, `set_ordinance_district`, `ordinance_costs`, `ordinance_estimate`, `ordinance_approval`, `district_stats`, `LedgerNow::ordinances`, `Why::ordinances`, `OverlayKind::District`.
- Milestone unlocks: six civic buildings open as the town grows (library at 1,500 people, high school 3,000, hospital 5,000, transit stop 6,000, stadium 9,000, university 14,000), shown greyed with "Unlocks at N people" in the Build window until then, each with its own price, upkeep and effect on land value, traffic, demand, arrivals, capacity, approval and leavers, with a news line when one unlocks and a goal card hint for the next. New models for them and for the power plant. Saves keep the unlocks in a `MILE` section (older saves open with what their size earned). New calls: `CivicKind::{Library, HighSchool, Hospital, Transit, Stadium, University}`, `unlock_at`, `covers`, `reach_at`, `Sim::is_unlocked`, `next_unlock`, `unlock_all`, `civic_notes`, `PlaceError::Locked`.
- Three rival towns (simulation and app): a commuter suburb, a port and factory town and a shopping and office city, each with an invented name, its own taxes (they undercut a tax rise and raise taxes as they grow), a population that grows on what it takes from the town and shrinks when the town wins people back, news when one overtakes the town, a Rivals window (R), rivals named on the voters panel and by challengers, and a rival line in the why panel. Saves keep the first rival in the body and the rest in a `RIVL` section. New calls: `Personality`, `RivalView`, `Sim::rival_towns`, `rival_name_of`, `rival_tax_of`, `rival_taking_most`.
- Avenues and bridges: the Build window has a Roads row (key Y) to draw an avenue or collector road from an open road by clicking points, with a ghost showing length, cost, bridges and why a spot is refused; crossing water builds a bridge at four times the cost per metre, standing buildings block the road, empty lots under it are given up. It is built in stages like other streets, then joins traffic, which re-routes within days; land value rises nearby and traffic noise lowers it. A second mode widens a street one class. New calls: `Sim::check_road`, `build_road`, `check_upgrade`, `upgrade_street`, `RoadPlan`, `UpgradePlan`, `RoadError`. Saves keep it in section AVEN.
- Trains run on the railways: passenger trains stop at the depot, freight slows by the grain elevator, and both run more often in bigger towns; lots near an open depot and industry beside the line gain land value and a little demand, shown in the why panel and the land value legend. New calls: `rail::rates_for`, `Sim::depot_open`, `open_depots`, `train_rates`, `rail_bonus`, `trains::Trains`, `Scene::set_trains`; the `trainsheet` example.
- Saves are format 5: each feature added from now on keeps its state in its own tagged section after the body, so new features no longer need a new save format. Format 3 (preview 6 and 7) and format 4 saves open with every new feature at its defaults (an older town gets the buildings its size has earned, and rival towns made from its seed).
- Towns with real centres (generator version 2): every lot now has a density from the town's centres (downtown round the hall, the blocks round the railway depot, crossings of big roads, malls). Lots near a centre lean to shops, flats and offices, up to 14 office towers can stand where downtown is densest, and crossroads out in the suburbs get small shopping corners. In play, builders fill a street from the buildings already standing instead of scattering, central lots of a town over 2,500 people start bigger, buildings beside taller neighbours rebuild sooner, and towers at the very middle rise higher (offices to about 24 floors), while outer suburbs never go past row houses. Every railway through town gets a brick depot with a platform and canopy where a street crosses the track nearest downtown, and prairie towns a grain elevator across the line; the mayor cannot build over them. Fixes: buildings no longer stand in parks (the zone is checked where the building really ends up), and streets no longer end on the railway (they cross it to the road beyond or stop short in a turning circle). Saves are format 4, which records the generator version; format 3 saves from preview 6 and 7 open on the exact town they were made on. New calls: `GEN_VERSION`, `generate_version`, `Station`, `StationKind`, `Centre`, `CentreKind`, `centrality`, `Lot::density`, `Town::stations`, `Town::centres`, `Town::gen_version`, `Sim::lot_density`, `max_level_at`, `build_lot_level_dense`, `rail`; `us-gen-cli --version` and `--density`; the `skyline` example.
- New app icon and logo: a cel-shaded town where tall blocks at the crossroads thin out to houses. Used for the iPhone and iPad app, the Linux AppImage and the desktop window and task bar icon. The drawing script is `packaging/icon/draw-logo.py`.
- The load screen says plainly that towns saved before 1.0.0 preview 6 cannot be opened and why. From save format 3 on, a format change must come with a converter so towns survive updates.
- Version label 1.0.0 preview 7.
- Growth has limits (simulation and a few hooks in the app): people and firms now arrive from outside at a pace set by game time, not by the speed setting, so a well run town reaches about 10,000 people in 10 to 15 game years instead of 3. How many come depends on jobs, taxes against the rival town, approval and how well the town is run; shops and firms follow the people, and families leave when there are no jobs. Water, power, school seats and clinics now have a capacity: when one is full the kinds of building that need it stop being started and the news says what to build ("Growth has stalled: the water supply is full. Build a water tower."). New: the power plant civic building (250,000 dollars; drawn with a borrowed look until it gets a model). New streets and utility connections are paid from the treasury when they start, long before their taxes. Easy, Normal and Hard presets (a row on the new town screen) scale arrivals, capacity per building and growth costs. The town now starts with only the streets near the town hall and 900,000 dollars; the Build window shows how full each capacity is. New calls: `Difficulty`, `Rules::difficulty`, `Rules::with_difficulty`, `Capacity`, `CapacityStatus`, `Sim::capacity`, `capacities`, `growth_stalls`, `job_ratio`, `growth_spending`, `CivicKind::Power`, `EventKind::Limit`. Saves are format 3.
- Growth by streets (simulation): a new neighbourhood no longer appears whole. Each street is built one at a time, starting from roads that already stand and branching outward (a side street waits for the street it leaves), only when the town is running short of building lots. A street takes two to seven weeks and passes through graded, paved and open; its lots can be built on only once it is open, builders favour lots on arterials and collectors, and alleys follow the street they run off. A street costs upkeep only once it is open. New calls: `road_visible`, `road_open`, `road_build_state` (stage and progress for the renderer), `streets_building`, `streets_opened`. Saves are format 2 (older saves can no longer be opened). The town view draws only started streets, with roadworks at the simulation's own progress.
- Growth by streets (simulation): a new neighbourhood no longer appears whole. Each street is built one at a time, starting from roads that already stand and branching outward (a side street waits for the street it leaves), only when the town is running short of building lots. A street takes two to seven weeks and passes through graded, paved and open; its lots can be built on only once it is open, builders favour lots on arterials and collectors, and alleys follow the street they run off. A street costs upkeep only once it is open. New calls: `road_visible`, `road_open`, `road_build_state` (stage and progress for the renderer), `streets_building`, `streets_opened`. Saves are format 2 (older saves can no longer be opened). The town view draws only started streets, with roadworks at the simulation's own progress.
- Fix (phones): an open panel sheet could cover the bottom bar of panel buttons, so most menus seemed missing; sheets now sit above the bar's real position. The placement hint says tap on touch screens.
- Fix (iOS): taps landed below where the player touched. The game drew at the safe-area size and iOS stretched the picture to the whole screen; it now draws at the full screen size and keeps the notch and home bar clear with insets.
- Graphics fidelity and visible road building: smooth edges (4x multisampling of the scene, with the ink lines resolved per sample so they stay crisp; adapters that cannot do it, such as OpenGL ES, and Metal for now keep drawing as before), ink thins out and then vanishes as a pixel covers more of the world and far roofs are lighter, so a whole-town view shows coloured roofs instead of dark specks. Buildings gain window frames, sills and mullions, door frames and steps, a darker base course, a cornice on tall walls, a fascia board along pitched eaves, and shop signs with stripe lettering; the far form drops all of it. A neighbourhood that opens now builds its streets in front of you (graded dirt, then gravel, then fresh asphalt, then painted lines, with a truck or roller on site) instead of appearing finished; widened streets use the same stages. Park neighbourhoods get a looping path with spokes to the edge, a pond, benches, a playground, a pavilion and trees in groups. Small additive changes in `us-app` (start roadworks when a neighbourhood opens, 8 redraw steps) and `us-render` (`park`, `detail`, `Ground::place_prop`, the `fidelity` example).
- Shown version is 1.0.0 preview 5.
- Hills in the game (preview 5): the town you play on now stands on the hills the generator made. Ground, roads, fields, shores, buildings, trees and the town hall follow the slope, roads that cross water show a bridge, regional building looks show in play, clicks and the placement ghost land on the real ground (the reach rings sit on the hillside), and the camera follows the ground height. The ground is cut into 256 m tiles with a coarse form for tiles far from the camera, so a whole-town view draws about a sixth of the ground faces of the full form. Game over and election night screens now dim only the town, not their own text. Phone: the bottom bar uses two rows of four buttons on a narrow screen so every label is 11 points or larger, and on iOS the safe area is read from both the view and its window each frame, with a safe default for phones when UIKit reports none.
- Fix: the iOS app lists all four orientations, which App Store Connect requires for an iPad app with multitasking (the first TestFlight upload was refused for it).
- Shown version is 1.0.0 preview 4.
- Stakes and saves in the interface (preview 4): a new town now asks how you play (Standard, Sandbox, or Sandbox with unlimited money) and lists all five town types with a line on each; the top bar shows an approval meter and the next election date, and a Voters panel (V) breaks approval down into taxes, services, jobs, traffic, debt and fires and names the worst issue in plain words; election night shows your share against the challenger; a game over screen for bankruptcy or a lost election shows your career and offers a new town, a save to load, or the menu. Saves: Save, Save as and Load in the pause menu, Continue and Load game on the main menu (with town, date, population and age of each save, and plain messages for a damaged, too old or too new save), and an autosave every game year, kept in the standard per-user data folder (the Documents folder on iOS) so updates never touch them. Phone layout (narrow or short screens, iPhone and iPad): one bottom bar of 44 point panel buttons, panels as sheets over the lower half of the screen, a two row top bar, stacked and shortened goal and news cards, everything inside the safe area, and touch gestures in the help strip on iOS. Street lamp light patches at night are smaller and rounder (a small additive change in the renderer).
- iOS app (iPhone and iPad), simulator first: the same game on Metal, with touch (one finger pans, pinch zooms, two-finger twist rotates, two-finger vertical drag tilts, tap selects or places the hall, long press explains), safe areas, 44 point controls, a two row top bar on phones, and pause and resume when the app goes to the background. No Lakeside sign-in, updater, quit button or window settings on iOS. The Xcode project is generated from `packaging/ios/project.yml`; CI builds it for the Simulator without signing and keeps screenshots for a day. A protected manual TestFlight upload workflow is added and is untested until the Apple agreement is accepted and the app record exists.
- The town on screen now shows the living town: buildings are drawn at their level (upgrades show a building site with a crane that grows with progress, burned lots show the burned look), widened and new streets show as roadworks for a while and then take their new width and markings, cars keep off roadworks, police stations, clinics and water towers have their own models, seasons follow the game date, and a calm day and night cycle (one day every 4 minutes at 1x, scaled by speed, frozen when paused) lights street lamps and windows. New Settings option "Day and night": cycle or always day. Only the changed lot's 256 m cell is rebuilt.
- Renderer: the hills are drawn. The ground is built from the town's heightfield and everything sits on it: fields, sand and roads follow the slope, buildings stand on the highest ground under them with their feet following the hill, trees, cars, lamps and chimney smoke ride the surface, and each river or lake is a flat sheet at its water level. Roads that cross the water climb onto a bridge deck with approach ramps, slab edges, parapets and piers. Slopes are shaded in stepped tones by how they face the sun, steep park land gets dense woods, ground picking and the shadow fit follow the hills. Dense blocks: every model is fitted inside its lot so neighbours never touch (garages and fences included). Roofs: shops, flats and offices lose the dark slate lids, flat roofs get pale decks, and more shops and flats get pitched, hipped or mansard roofs. New helpers for the game view: `Land`, `live::*_on`, `Obj::emit_on`, `Scene::pick`, `kit::build_lot_for` and `build_lot_level_for` (which pass the town style so regional looks show in play); the `hills` example draws the pictures.
- Building variety: a parametric kit replaces the fixed models, with roofs (gable, cross gable, hip, flat with parapet, mansard, shed, sawtooth), dormers, chimneys, solar panels, porches, bay windows, balconies, garages, driveways, fences, yard props (sheds, pools, trampolines, gardens, boats), shopfronts with awnings, signs and cut corner entrances, stepped and twin towers, and brick, painted wood, stucco, concrete and glass curtain wall families, all seeded per lot. Each town style leans its own way (prairie brick and gables, river painted wood and porches, Sunbelt stucco, flat roofs and pools, lake cottages and boats). Distinct looks measured by the new `variety` example rose from 13,153 to over 319,000 (the sample caps most kinds). `kitsheet` and `townviews` examples draw the pictures.
- Twice the density: towns now hold about twice as many buildings per square kilometre in every style. Houses, shops, rows and apartments sit closer together (old town lots about 9 to 15 m wide, suburbs 15 to 20 m), shops, rows and apartments in the core and inner ring are built shoulder to shoulder, corner lots are used, and blocks get back lots (up to three rows downtown, two in the inner ring, an occasional back house in outer suburbs, small courtyards between apartment blocks). Builders start twice as many buildings a day and keep twice as many lots ready, so a town fills at the same pace; population and jobs follow, and traffic stays light. `shots` now renders every town style.
- Town generation: a seeded heightfield (flat ground round the hall and highway, a flat river valley, hills further out) with old roads and curving suburbs bending along the contours, steep ground left wooded and unbuilt, and heights and slopes exposed for the renderer; three new town styles (Sunbelt, Crossroads, Lake town) in the new-town menu; grid and winding layouts tuned against road statistics of real towns (bigger blocks, far fewer alleys, fewer arterials, more collectors), with the tuner (`us-metrics tune`) included.
- Shown version is 1.0.0 preview 3.
- Fix: placing a school, police station, fire station, clinic, park or water tower closed the game, because the town view did not know about the new lot. The view now follows lots the mayor adds.
- Preview 3 controls: separate tax sliders for homes, shops and industry with the money each brings in; a Budget window (funding per department with cost per month and coverage); a Bonds window (borrow, repay, interest, credit limit); a Build window to place school, police, fire, clinic, park and water tower with a ghost showing the reach and why a spot is refused; a Graphs window with line and stacked area charts of the town's history (people and jobs, money, income, spending, tax rates, demand, buildings, land value, traffic) over 1 year, 5 years or all, with hover values and series toggles; small trend lines for population, treasury and monthly net in the top bar; a News list that flies the camera to where something happened; map views (land value, building level, use, each service's coverage, demand, traffic) with a legend and keyboard shortcuts, tinted by the renderer; cars and chimney smoke now run in the town; the explain panel now shows land value, building level and coverage.
- Renderer polish and reach: the game now draws through OpenGL when Vulkan is missing (the ink pass uses an integer target every back end can render to; checked on the software OpenGL driver and matches Vulkan to under 0.1 percent of pixels). Far chunks switch to a lighter form (big faces only, no ink) and street lamps are skipped at a distance, roughly halving the frame time of a whole 4 km town on the software renderer. New models for a police station, clinic, standpipe water tower, library and stadium. Street lamps along the roads with a crisp lit patch on the street at night; autumn ground tones and fallen leaves, spring blossom on some trees, a deeper summer green, and snow banked along road edges in winter. `bench` and `polish` examples render the timings and pictures.
- Saves: `Sim::save` and `Sim::load` write and read the whole game (town seed and preset, hall, every lot and road, levers, bonds, history, events, rival, elections, clock and caches) in a compact versioned binary format with a checksum; older or newer formats, damaged and cut-short files are refused with a plain message. Loading then stepping gives exactly the same game as never saving. `Sim::autosave` rotates three files in a folder the caller chooses.
- Simulation pressure: a rebalanced economy (running cost per building grows with the size of the town and with service cover, utilities follow building value, road upkeep doubled, bond interest 8 percent rising to 12 as the credit limit fills) so default taxes hover near break-even over 30 game years, very low taxes run a deep deficit and very high taxes empty the town; bankruptcy (overdrawn with no credit left for six months ends the game, after emergency bonds); voter approval from taxes, service cover, jobs, traffic, debt and fires; an election every four years against an invented challenger who runs on your worst issue, and losing ends the game; a game over state with a career summary; sandbox rules (no bankruptcy, no elections, optional unlimited money). Approval and days to the next election are in the monthly history.
- Renderer: a living world on screen. Six map views (land value, building level, use, service coverage, demand, traffic) in flat stepped colours; building models for levels 1 to 5 of every lot kind, with building-site, burned-out and rebuilt looks; roads re-meshed in place when their class changes, with a roadworks state; instanced moving cars that follow per-road traffic volumes; a stepped day and night cycle with lit windows, autumn leaves and winter snow from the game date; factory smoke and downtown pedestrians. `shots` renders all of it.
- CI: pull request runs build the installers and zips without publishing them; the Linux copy used by the launcher-style start check is deleted right after the check.
- Simulation depth (a living town): buildings rebuild bigger through five levels when land value, demand and services stay high, merge empty lots for big buildings and run down on poor land; every quarter a background traffic run widens streets over capacity and adds connecting roads, and feeds congestion into land value and the history; fires start where fire cover is weak and the lot is rebuilt; a rival town sets its own taxes and draws households, shops and firms from a town that taxes more; seasonal demand swings.
- Simulation depth (levers, history, land value): separate tax rates for homes, shops and industry; department budgets from 0 to 150 percent (police, fire, schools, health, parks, road upkeep); bonds with interest and a credit limit from the tax base; placeable school, police station, fire station, clinic, park and water tower that cover lots by road travel distance; land value per lot recomputed monthly and used by growth and abandonment; one history sample per game month; a dated event log; map layer data for land value, building level, use, service coverage and demand.
- Internal: the app is now a library with a thin desktop binary, and platform specific code sits in `platform/` modules, so an iOS front end can share the game. No change for players.
- Preview 2: a visible goal and first-run tips (population goals, three short hints you can dismiss), event messages (new neighbourhood, first shop, school, church and fire station, treasury running low or below zero), a readable controls strip, clearer hall placement text, the signed-in name in the top bar and pause menu, signed-out and launcher sign-in lines on the main menu. Fix: Back in Settings opened from the pause menu threw away the running town; it now returns to the pause menu. Shown version is 1.0.0 preview 2 and the release title follows the tag.
- Fix a flaky updater test: parallel download tests shared one scratch folder.
- The repository is public: README says what the game is and that it is an unreleased, paid preview; LICENSE.md (all rights reserved, Silkscreen under the OFL); contributor guide without internal notes; builds are no longer attached to GitHub releases, and build artifacts expire after one day.
- Fix: the Linux build is compiled on Ubuntu 22.04 so it runs on 22.04, 24.04, Debian 12 and other current distributions (preview 1 needed glibc 2.43 and closed at once on them).
- Fix: when the graphics cannot start (for example Linux with no Vulkan driver), the game shows a message saying why and what to install, instead of closing silently; a panic on the main thread is shown too.
- Linux AppImage carries the keyboard libraries the window needs, and CI now starts every Linux build the way the Lakeside launcher does (launcher environment, made-up sign-in code, stock desktop) and fails if it does not stay up.
- Interface uses the studio's dark navy panels with gold for the main action, no cream panels.
- Fix: text and texture updates the UI produces in a frame the window skips are kept and uploaded on the next frame instead of being lost (the debug build stopped on this).
- Lakeside account in the app: "Signed in as" on the main menu when the launcher hands over a sign-in, and Sign in, Sign out and Check for updates in Settings (updates say the launcher keeps the copy current when it started the game).
- Playable 1.0.0 preview 1: `urban-sprawl` desktop app (winit, wgpu, egui in the studio pixel style) with a main menu, new town (grid or winding, seed), town hall placement at an old crossroads, a town that grows outward by itself, speed controls, a property tax lever, a demand bar, a click-to-explain panel, and a smooth orbit camera. New `us-sim` crate holds the deterministic growth, construction, tax and abandonment rules. `us-render` can now update a scene chunk by chunk and draw into a window. Release workflow builds Windows, macOS (ad-hoc signed) and Linux packages.
- 3D renderer: new `us-render` crate draws a generated town offscreen with wgpu in the cel look (three stepped tones, ink outlines, hard shadows from one sun), with the full building kit ported from the mockups, an orbit camera in metres, ground picking, and a `shots` example. Depends on the connection fixes PR.
- Lakeside launcher and accounts: new `us-lakeside` crate (no UI) reads the launcher environment, redeems the handoff code once off the main thread (5 second limit, no redirects, never retried), offers optional in-game sign-in with the token in OS secure storage, and checks and downloads updates through Lakeside with size and SHA-256 verification. Updates are off when the launcher starts the game.
- Installers workflow: Windows Inno Setup, Linux AppImage and macOS dmg installers plus a launcher manifest, built on tags and on demand.
- 3D renderer: new `us-render` crate draws a generated town offscreen with wgpu in the cel look (three stepped tones, ink outlines, hard shadows from one sun), with the full building kit ported from the mockups, an orbit camera in metres, ground picking, and a `shots` example.
- Traffic shapes the roads (phase 1): `us-metrics` measures how real a road network is (block sizes, junction mix, dead ends, orientation spread, circuity, class shares) and scores generated towns against statistics from real towns; `us-traffic` routes peak hour trips on a town with user equilibrium assignment; `us-grow` upgrades streets over capacity and adds connectors where the drive is long, behind a proposer trait and a scorer trait. `us-grow-cli` draws traffic before and after growth.
- Town generator connection fixes: country roads cut short by the river now end on a riverside drive (both presets, both banks), a ring road is no longer split at its start point, and a street group cut off from the network gets an entrance road under 150 m or goes back to farmland. New tests check the whole road network is one piece and no arterial ends in open land.
- Workspace skeleton: town generation crate and its command line tool.
- Town generator: `us-gen` builds a 4 km town from a preset (grid or winding) and a seed, in the approved pass 5 style (skeleton, additions with their own street patterns, pods, collectors, lots). Long addition-edge streets become collectors, pods always get an entrance and at least two cul-de-sacs, small downtowns get at most five towers. `us-gen-cli` renders a plan-view PNG and a stats file.
