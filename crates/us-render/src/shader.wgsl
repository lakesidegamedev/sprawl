// Cel shader: three stepped tones per face, hard shadows from one sun, and an ink edge pass.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    previous_view_proj: mat4x4<f32>,
    eye: vec4<f32>,     // xyz camera position, w near plane
    fog: vec4<f32>,     // rgb atmospheric light, w mist density
    terrain: vec4<f32>, // elevation base and range, inverse map size and texture size
    local_lights: vec4<f32>, // world tile origin, streetlight and headlight strength, contact shade on
    resolve: vec4<f32>, // xy native to world pixel conversion
    sun: vec4<f32>,    // xyz toward the sun, w shadow texel size in metres
    ink: vec4<f32>,    // rgb ink colour, a ink width in pixels (1 or 2)
    shade: vec4<f32>,  // x multiplier for a shadowed face, y shadow depth bias, z seasonal blend, w overcast
    screen: vec4<f32>, // width, height of the main targets
    tone: vec4<f32>,   // rgb flat tint of the whole picture, a share of lit windows
    look: vec4<f32>,   // x season (1 autumn, 2 spring, 3 summer), y snow, z map view (desaturate), w share of lamps and vehicle lights on (0 to 1)
    wx: vec4<f32>,     // x snow cover 0 to 1, y wet ground, zw wind (east, north) in m/s
    precipitation: vec4<f32>, // rain, snow and the lower corner of the weather grid
    weather_flow: vec4<f32>,   // wind displacement, storm strength and tilt shift
    lightning: vec4<f32>,     // strike position and intensity, w ink softness
    temporal: vec4<f32>,     // history weight, bloom intensity, exposure, tone mapping
    seasons: vec4<f32>,      // x 1 when a calendar is running, so season 0 is a real winter, y 1 for the wide occlusion band, z 1 for all occlusion (picture switches)
    rock: array<vec4<f32>, 8>, // rows 0 to 3 strata colours, 4 talus, 5 (snow line, tree line, band height, on), 6 (snow rgb, winter drop)
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;
@group(0) @binding(5) var lights_tex: texture_2d<f32>;
@group(0) @binding(6) var reflection_probes: texture_2d<f32>;
@group(0) @binding(7) var cloud_tex: texture_3d<f32>;
@group(0) @binding(8) var cloud_sampler: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) id: u32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) id: u32,
    @location(4) dist: f32,
    @location(5) @interpolate(flat) tint: u32,
    @location(6) @interpolate(flat) room_anchor: vec3<f32>,
};

// Tree crowns lean downwind and sway with the wind's strength. Only leaf-coloured vertices move.
fn sway(p: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let w = g.wx.zw;
    if (abs(w.x) + abs(w.y) < 0.5) {
        return p;
    }
    if (leaf_step(c, 88.0, 150.0, 76.0) < 0 && leaf_step(c, 70.0, 128.0, 66.0) < 0
        && leaf_step(c, 100.0, 168.0, 58.0) < 0) {
        return p;
    }
    let k = 0.045 * (1.0 + 0.5 * sin(g.screen.w * 2.3 + p.x * 0.21 + p.y * 0.17));
    return vec3<f32>(p.x + w.x * k, p.y + w.y * k, p.z);
}

// Where a broadleaf crown has lost its leaves the vertex is thrown outside the view, so every
// triangle of the crown collapses to nothing and casts no shadow. The bark skeleton inside it
// is left standing. All of a crown's vertices share a colour family, so a face never mixes.
fn crown_colour(c: vec3<f32>) -> bool {
    return leaf_step(c, 88.0, 150.0, 76.0) >= 0 || leaf_step(c, 70.0, 128.0, 66.0) >= 0;
}

fn haze_step(c: vec3<f32>, r: f32, gr: f32, b: f32) -> i32 {
    let d = c * 255.0 - vec3<f32>(r, gr, b);
    if (abs(d.x) < 0.6 && abs(d.y) < 0.6 && d.z > 7.4 && d.z < 11.6) {
        return i32(round(d.z)) - 8;
    }
    return -1;
}

// 1 while the leaves are on, 0 when they are down. Neighbourhoods turn at different moments
// (the same landscape field the palettes use), so leaves come and go patch by patch.
fn leaf_presence(world: vec3<f32>) -> f32 {
    let season = i32(g.look.x + 0.5);
    if (g.seasons.x < 0.5) { return 1.0; }
    // leaves drop early in the winter weeks and return late in the spring ones
    if (season == 0) { return 1.0 - smoothstep(0.05, 0.65, season_phase(world)); }
    if (season == 2) { return smoothstep(0.35, 0.95, season_phase(world)); }
    return 1.0;
}

// `limit` is the leaf cover below which the crown is dropped: a thinning crown stops casting its
// solid shadow earlier than it stops being drawn.
fn crown_gone(c: vec3<f32>, world: vec3<f32>, limit: f32) -> bool {
    if (g.look.z > 0.5 || (g.look.x > 0.5 && g.look.x < 1.5) || (g.look.x > 2.5)) {
        return false;
    }
    return crown_colour(c) && leaf_presence(world) < limit;
}

const FOLDED: vec4<f32> = vec4<f32>(2.0, 2.0, 2.0, 1.0);

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let p = sway(v.pos, v.color.rgb * 255.0);
    o.clip = g.view_proj * vec4<f32>(p, 1.0);
    if (crown_gone(v.color.rgb, p, 0.02)) {
        o.clip = FOLDED;
    }
    o.world = p;
    o.normal = v.normal;
    o.color = v.color;
    o.id = v.id;
    o.dist = o.clip.w;
    o.tint = 0u;
    o.room_anchor = v.pos;
    return o;
}

struct Inst {
    @location(4) pos_theta: vec4<f32>,
    @location(5) scale: f32,
    @location(6) tint: u32,
    @location(7) id: u32,
};

fn inst_world(v: VIn, i: Inst) -> vec3<f32> {
    let s = sin(i.pos_theta.w);
    let c = cos(i.pos_theta.w);
    let p = v.pos * i.scale;
    return vec3<f32>(i.pos_theta.x + c * p.x - s * p.y, i.pos_theta.y + s * p.x + c * p.y, i.pos_theta.z + p.z);
}

@vertex
fn vs_inst(v: VIn, i: Inst) -> VOut {
    var o: VOut;
    let w = inst_world(v, i);
    let s = sin(i.pos_theta.w);
    let c = cos(i.pos_theta.w);
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    if (crown_gone(v.color.rgb, w, 0.02)) {
        o.clip = FOLDED;
    }
    o.world = w;
    o.normal = vec3<f32>(c * v.normal.x - s * v.normal.y, s * v.normal.x + c * v.normal.y, v.normal.z);
    o.color = v.color;
    if (v.id == 16777215u) {
        o.id = v.id;
    } else {
        o.id = v.id + i.id;
    }
    o.dist = o.clip.w;
    o.tint = i.tint;
    o.room_anchor = w;
    return o;
}

@vertex
fn vs_inst_shadow(v: VIn, i: Inst) -> @builtin(position) vec4<f32> {
    let w = inst_world(v, i);
    if (crown_gone(v.color.rgb, w, 0.6)) {
        return FOLDED;
    }
    return g.light_vp * vec4<f32>(w, 1.0);
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    let p = sway(v.pos, v.color.rgb * 255.0);
    if (crown_gone(v.color.rgb, p, 0.6)) {
        return FOLDED;
    }
    return g.light_vp * vec4<f32>(p, 1.0);
}

// Fraction of the comparison footprint that the sun reaches.
fn lit(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.sun.z <= 0.03 || g.shade.x >= 0.985) {
        return 1.0;
    }
    let p = world + n * g.sun.w * 1.5;
    let c = g.light_vp * vec4<f32>(p, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || c.z > 1.0) {
        return 1.0;
    }
    // A soft edge: each tap is already a filtered comparison, and the ring widens the
    // penumbra to about three texels so shadow edges stop stepping along the texel grid.
    let texel = 1.0 / f32(textureDimensions(shadow_tex).x);
    let depth = c.z - g.shade.y;
    let centre = textureSampleCompareLevel(shadow_tex, shadow_cmp, uv, depth);
    if (g.resolve.w < 0.5 && g.local_lights.w < 0.5) {
        // the light preset (phones by default) keeps the single filtered comparison
        return centre;
    }
    var sum = centre * 2.0;
    let r = texel * 1.4;
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(r, r * 0.4), depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(-r * 0.4, r), depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(-r, -r * 0.4), depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(r * 0.4, -r), depth);
    if (g.resolve.w < 0.5) {
        return sum / 6.0;
    }
    // the wide ring reaches further across a sloping face, so it compares with more slack
    let far_depth = depth - g.shade.y * 1.6;
    let q = texel * 2.6;
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(q, -q * 0.3), far_depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(q * 0.3, q), far_depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(-q, q * 0.3), far_depth);
    sum += textureSampleCompareLevel(shadow_tex, shadow_cmp, uv + vec2<f32>(-q * 0.3, -q), far_depth);
    return sum / 10.0;
}

fn packed_surface(n: vec3<f32>, material: u32) -> u32 {
    let sum = abs(n.x) + abs(n.y) + abs(n.z);
    let normal = n / max(sum, 0.001);
    let face = select(normal.xy, (1.0 - abs(normal.yx)) * sign(normal.xy + 0.0001), normal.z < 0.0);
    let uv = vec2<u32>(round((face * 0.5 + 0.5) * 7.0));
    return ((material & 3u) << 30u) | (uv.y << 27u) | (uv.x << 24u);
}

fn surface_normal(value: u32) -> vec3<f32> {
    let uv = vec2<f32>(f32((value >> 24u) & 7u), f32((value >> 27u) & 7u)) * (2.0 / 7.0) - 1.0;
    let z = 1.0 - abs(uv.x) - abs(uv.y);
    let fold = max(0.0, -z);
    return normalize(vec3<f32>(uv - sign(uv) * fold, z));
}

struct FOut {
    @location(0) colour: vec4<f32>,
    @location(1) aux: vec4<u32>,
};

fn hash_u(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

fn near(c: vec3<f32>, r: f32, gr: f32, b: f32) -> bool {
    let d = abs(c * 255.0 - vec3<f32>(r, gr, b));
    return d.x < 1.6 && d.y < 1.6 && d.z < 1.6;
}

// Tree leaf colours differ by 0 to 3 in blue; returns that step, or -1 for not a leaf.
fn leaf_step(c: vec3<f32>, r: f32, gr: f32, b: f32) -> i32 {
    let d = c * 255.0 - vec3<f32>(r, gr, b);
    if (abs(d.x) < 0.6 && abs(d.y) < 0.6 && d.z > -0.6 && d.z < 3.6) {
        return i32(round(d.z));
    }
    return -1;
}

fn autumn_leaf(step: i32, dark: bool) -> vec3<f32> {
    var c = vec3<f32>(214.0, 120.0, 36.0);
    if (step == 1) { c = vec3<f32>(204.0, 164.0, 44.0); }
    if (step == 2) { c = vec3<f32>(172.0, 66.0, 40.0); }
    if (step == 3) { c = vec3<f32>(150.0, 150.0, 52.0); }
    if (dark) { c = c * 0.85; }
    return c / 255.0;
}

// Marker colours that only show in their season: fallen leaves (autumn), a lamp's pool of light
// (night), snow banked along the road edge (winter). The blue channel of a leaf patch carries
// the tree's colour step.
fn patch_step(c: vec3<f32>) -> i32 {
    return leaf_step(c, 250.0, 126.0, 16.0);
}

// ---- ground grain: stepped patches and tufts on natural ground, so fields, grass, sand and
// snow are not flat colour. Stepped like the rest of the cel look: no blur, no haze.

fn hash2(p: vec2<i32>) -> f32 {
    let h = hash_u(bitcast<u32>(p.x) * 73856093u ^ bitcast<u32>(p.y) * 19349663u);
    return f32(h % 10000u) / 10000.0;
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<i32>(1, 0));
    let c = hash2(i + vec2<i32>(0, 1));
    let d = hash2(i + vec2<i32>(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn cloud_base() -> f32 {
    return max(0.0, g.terrain.x) + 107.0;
}

fn cloud_thickness() -> f32 {
    return 174.0;
}

// Cover opens and closes gaps inside a deck at a fixed elevation. The wind
// carries the entire form while height-dependent shear pulls the crowns ahead.
fn cloud_drift() -> vec2<f32> {
    return g.screen.w * vec2<f32>(7.8, 2.8) + g.weather_flow.xy;
}

fn cloud_field(xy: vec2<f32>, layer: f32) -> f32 {
    let shear = clamp(g.wx.zw, vec2<f32>(-9.0), vec2<f32>(9.0)) * (3.0 + 3.0 * layer);
    let p = xy - cloud_drift() - shear;
    return vnoise(p * 0.0038) * 0.71 + vnoise(p * 0.0106 + vec2<f32>(8.7, 11.3)) * 0.29;
}

fn cloud_threshold() -> f32 {
    return mix(0.95, 0.42, g.shade.w);
}

fn cloud_density(point: vec3<f32>, detail: f32) -> f32 {
    let height = point.z - cloud_base();
    let layer = height / cloud_thickness();
    if (layer <= 0.0 || layer >= 1.0) {
        return 0.0;
    }

    let field = cloud_field(point.xy, layer);
    // Wider gaps between the local storm cells preserve breaks in a heavy deck.
    let threshold = cloud_threshold() + (1.0 - field) * (1.0 - field) * 0.21 * g.weather_flow.z;
    if (field < threshold - 0.17) {
        return 0.0;
    }

    let time = g.screen.w;
    let wind = clamp(g.wx.zw, vec2<f32>(-9.0), vec2<f32>(9.0));
    let p = point.xy - cloud_drift() - wind * (3.0 + 4.0 * layer);
    let billow_pos = vec3<f32>(p + height * vec2<f32>(0.48, -0.34),
        height - time * 0.84 + 92.0);
    let billow = textureSampleLevel(cloud_tex, cloud_sampler, billow_pos / 510.0, 0.0).r;
    let ceiling = mix(0.52, 1.0, smoothstep(0.30, 0.76, billow * 0.63 + field * 0.43));
    let vertical = smoothstep(0.0, 0.17, layer)
        * (1.0 - smoothstep(ceiling - 0.32, ceiling, layer));
    let base = field + (billow - 0.5) * 0.51;
    var shape = base;
    if (detail > 0.0 && base < threshold + 0.27) {
        // Faster flow at this scale continually erodes and replaces the wisps
        // on a growing billow. Neither octave changes its opacity with time.
        let fine = billow_pos + time * vec3<f32>(1.9, -0.8, -0.43);
        let folds = textureSampleLevel(cloud_tex, cloud_sampler, fine / 132.0, 0.0).rg;
        let edge = 1.0 - smoothstep(threshold + 0.05, threshold + 0.27, base);
        shape += ((folds.x - 0.5) * 0.17 - folds.y * 0.048) * edge * detail;
    }
    let mass = smoothstep(threshold - 0.06, threshold + 0.19, shape);
    // A thin fringe transmits light from town. The denser part of the same
    // volume still has enough extinction to shade its interior.
    let extinction = mass * mix(0.21, 1.0, mass);
    return extinction * vertical * smoothstep(threshold - 0.17, threshold - 0.085, field);
}

fn cloud_sunlight(world: vec3<f32>) -> f32 {
    if (g.shade.w <= 0.12 || g.sun.z <= 0.035 || g.look.z > 0.5) {
        return 1.0;
    }
    let level = cloud_base() + cloud_thickness() * 0.43;
    let distance = max(0.0, level - world.z) / max(0.19, g.sun.z);
    let field = cloud_field(world.xy + g.sun.xy * distance, 0.43);
    let threshold = cloud_threshold();
    let shadow = smoothstep(threshold - 0.08, threshold + 0.23, field);
    let below = 1.0 - smoothstep(level - 10.0, level + 32.0, world.z);
    let cover = smoothstep(0.12, 0.36, g.shade.w);
    return 1.0 - shadow * below * cover * smoothstep(0.035, 0.21, g.sun.z) * 0.18;
}

// Natural ground (grass, fields, sand, dirt, snow), not roads, pavements, markings or water.
fn natural(c: vec3<f32>) -> bool {
    let hi = max(c.r, max(c.g, c.b));
    let lo = min(c.r, min(c.g, c.b));
    let snowy = lo > 0.88 && c.b >= c.r;
    let watery = c.b > c.g + 0.04 && c.b > c.r + 0.04;
    return (hi - lo > 0.055 || snowy) && !watery;
}

// Ground kinds, told from the base colour before the season changes it.
// 0 other, 1 grass, 2 field, 3 sand, 4 park lawn, 5 bare earth.
fn ground_kind(c: vec3<f32>) -> i32 {
    if (near(c, 96.0, 176.0, 84.0)) { return 4; }
    if (near(c, 176.0, 198.0, 106.0) || near(c, 198.0, 186.0, 124.0) || near(c, 140.0, 172.0, 96.0)
        || near(c, 168.0, 190.0, 110.0) || near(c, 206.0, 194.0, 140.0) || near(c, 128.0, 164.0, 88.0) || near(c, 222.0, 194.0, 100.0) || near(c, 170.0, 134.0, 94.0)) {
        return 2;
    }
    if (near(c, 214.0, 200.0, 150.0) || near(c, 226.0, 208.0, 150.0) || near(c, 206.0, 196.0, 150.0)) { return 3; }
    if (is_green(c)) { return 1; }
    if (natural(c)) { return 5; }
    return 0;
}

// A small tuft in a jittered cell of size `cell`: a short upright stroke with a leaning twin.
// Returns 1 inside a blade, 0 outside.
// Terrain detail is filtered by the world-space footprint of a screen pixel.
fn ground_detail(col: vec3<f32>, base: vec3<f32>, kind: i32, w: vec2<f32>, dist: f32, snowy: bool) -> vec3<f32> {
    var c = col;
    let near_d = dist < 140.0;
    let mid_d = dist < 600.0;
    // broad patches at three scales, in three tone bands
    let big = vnoise(w / 38.0);
    let mid = vnoise(w / 11.0);
    var n = big * 0.6 + mid * 0.4;
    if (mid_d) {
        n += (vnoise(w / 3.6 + vec2<f32>(5.0, 9.0)) - big) * 0.2;
    }
    var k = 1.0;
    if (n > 0.66) {
        k = 1.05;
    } else if (n < 0.34) {
        k = 0.955;
    }
    if (snowy) {
        // a faint cool shade on the lee of a drift only: the drift height is the same noise the
        // cover uses, and the shade follows its slope away from the sun, so it never sits
        // in blotches of its own
        let away = -normalize(g.sun.xy + vec2<f32>(0.0001, 0.0)) * 2.4;
        let here = vnoise(w / 16.0) * 0.7 + vnoise(w / 5.0) * 0.3;
        let past = vnoise((w + away) / 16.0) * 0.7 + vnoise((w + away) / 5.0) * 0.3;
        let lee = clamp((here - past) * 5.0, 0.0, 1.0);
        let s = mix(vec3<f32>(1.0), vec3<f32>(0.93, 0.955, 1.0), lee * 0.8);
        return c * s;
    }
    if (kind == 1 || kind == 4) {
        // Grassland: a continuous spectrum of variation instead of two glade sizes, so the
        // ground reads as pasture with lusher hollows and drier rises, not as painted blots.
        let footprint = g.screen.z * dist;
        let rich = g.resolve.w > 0.5 || g.local_lights.w > 0.5;
        var warp = vec2<f32>(0.0);
        if (rich) {
            warp = vec2<f32>(vnoise(w / 97.0 + vec2<f32>(13.0, 5.0)), vnoise(w / 97.0 + vec2<f32>(-7.0, 31.0))) - 0.5;
        }
        let broad = vnoise(w / 260.0 + vec2<f32>(4.1, 22.8));
        let field = vnoise((w + warp * 70.0) / 74.0);
        let glade = vnoise((w + warp * 24.0) / 21.0 + vec2<f32>(9.0, 2.0));
        var clump = 0.5;
        if (rich) {
            clump = vnoise(w / 6.1 + vec2<f32>(3.0, 17.0));
        }
        let meadow = broad * 0.40 + field * 0.33 + glade * 0.18 + clump * 0.09;
        let lush_green = vec3<f32>(0.25, 0.39, 0.21);
        let dry_green = vec3<f32>(0.46, 0.48, 0.28);
        let seasonal = mix(lush_green, dry_green, smoothstep(0.24, 0.78, meadow));
        var grass = mix(c, seasonal * (c.g / 0.66), select(0.70, 0.45, kind == 4));
        let detail = 1.0 - smoothstep(0.06, 0.32, footprint);
        let tufts = 1.0 - smoothstep(0.18, select(0.68, 0.95, rich), footprint);
        var grain = 0.0;
        if (tufts > 0.0) {
            // tussocks about a metre and a half across, combed a little by the wind
            let comb = w + vec2<f32>(w.y * 0.28, 0.0);
            let tussock = vnoise(comb / vec2<f32>(1.9, 1.3) + vec2<f32>(2.9, -8.1));
            grain += (tussock - 0.5) * 0.17 * tufts;
        }
        if (detail > 0.0) {
            grain += (vnoise(w * vec2<f32>(5.6, 2.8) + vec2<f32>(0.5, 13.0)) - 0.5) * 0.24 * detail;
            // wild flowers gather in the lusher ground
            let speck = hash2(vec2<i32>(floor(w * 3.1)));
            let bloom = smoothstep(0.52, 0.66, glade) * (1.0 - smoothstep(0.50, 0.70, field)) * detail;
            if (kind == 1 && speck > 0.965 && i32(g.look.x + 0.5) >= 2) {
                let petal = mix(vec3<f32>(0.96, 0.90, 0.46), vec3<f32>(0.95, 0.95, 0.90), step(0.985, speck));
                grass = mix(grass, petal, bloom * 0.8);
            }
        }
        let cut = 1.0 + select(0.0, 0.05 * sign(sin((w.x + w.y * 0.22) * 0.9)) , kind == 4);
        return grass * cut * (0.90 + 0.17 * meadow + grain);
    }
    if (kind == 2) {
        // crop rows: rows run one way per field colour, a broad band of tone per row
        let alongx = (u32(base.r * 255.0 + 0.5) & 1u) == 0u;
        let u = select(w.x, w.y, alongx);
        let row = fract(u / 1.6);
        if (dist < 260.0) {
            // close up the rows are real ridges (cover.rs); only a faint tone remains here
            if (row < 0.4) {
                k = k * 0.97;
            }
        } else {
            // far: wider bands, still readable
            let rowf = fract(u / 6.4);
            if (rowf < 0.4) {
                k = k * 0.95;
            }
        }
        let base_crop = mix(vec3<f32>(0.48, 0.49, 0.32), vec3<f32>(0.60, 0.53, 0.33),
            smoothstep(0.28, 0.78, vnoise(w / 65.0)));
        return mix(c, base_crop, 0.38) * k;
    }
    if (kind == 3) {
        // sand ripples: wavy lines
        let rp = sin((w.x * 0.9 + w.y * 0.5) * 1.3 + vnoise(w / 4.0) * 6.0);
        if (rp > 0.72) {
            k = k * 1.05;
        } else if (rp < -0.8) {
            k = k * 0.95;
        }
        c = c * k;
        if (near_d && hash2(vec2<i32>(floor(w / 0.8))) > 0.97) {
            c = c * 0.9;
        }
        return c;
    }
    // bare earth, gravel: pebbles
    c = c * k;
    if (near_d) {
        let ph = hash2(vec2<i32>(floor(w / 0.6)));
        if (ph > 0.9) {
            c = c * 0.88;
        } else if (ph < 0.06) {
            c = c * 1.1;
        }
    }
    return c;
}

// Water kinds from the marker colours: 1 open water, 2 pond, 3 far shallows, 4 near shallows, 5 foam.
fn water_kind(c: vec3<f32>) -> i32 {
    if (near(c, 66.0, 132.0, 200.0)) { return 1; }
    if (near(c, 84.0, 150.0, 212.0)) { return 2; }
    if (near(c, 60.0, 120.0, 188.0)) { return 3; }
    if (near(c, 54.0, 108.0, 176.0)) { return 4; }
    if (near(c, 244.0, 250.0, 254.0)) { return 5; }
    return 0;
}

// Two wave bands follow the wind in world space. Filter the shorter band as it
// falls below a pixel so moving water stays steady at town-scale zoom.
fn storm_cell(xy: vec2<f32>) -> f32 {
    let p = xy - cloud_drift();
    let front = vnoise(p * 0.006);
    let squall = vnoise(p * 0.021 + vec2<f32>(17.0, 4.0));
    return smoothstep(0.21, 0.83, front * 0.76 + squall * 0.24);
}

fn flash_at(world: vec3<f32>) -> f32 {
    let d = (world.xy - g.lightning.xy) * 0.004;
    return g.lightning.z / (1.0 + dot(d, d));
}

// The derivative of the same smoothly interpolated noise used for terrain.
fn wave_gradient(p: vec2<f32>) -> vec2<f32> {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<i32>(1, 0));
    let c = hash2(i + vec2<i32>(0, 1));
    let d = hash2(i + vec2<i32>(1, 1));
    return 6.0 * f * (1.0 - f) * vec2<f32>(mix(b - a, d - c, u.y), mix(c - a, d - b, u.x));
}

fn wave_normal(w: vec2<f32>, px: f32, pond: bool) -> vec3<f32> {
    let velocity = g.wx.zw;
    let gust = min(length(velocity) / 13.0, 1.0);
    let drift = velocity * 0.14 + vec2<f32>(0.88, 0.44);
    let along = normalize(select(drift, vec2<f32>(0.88, 0.44), dot(drift, drift) < 0.001));
    let side = vec2<f32>(-along.y, along.x);
    let t = g.screen.w;
    let local = vec2<f32>(dot(w, along), dot(w, side));
    let swell = wave_gradient((local - vec2<f32>(t * (1.5 + gust * 2.3), t * 0.4))
        * vec2<f32>(0.105, 0.042));
    let broad = 1.0 - smoothstep(2.0, 7.5, px);
    let fine = 1.0 - smoothstep(0.35, 2.0, px);
    var slope = vec2<f32>(0.18, 0.10) * swell * broad;
    if (fine > 0.0) {
        let cross = vec2<f32>(local.x * 0.62 + local.y * 0.38, local.y * 0.73 - local.x * 0.27);
        let ripples = wave_gradient(cross * 0.65 - vec2<f32>(t * 0.91, t * 0.57));
        slope += ripples * 0.10 * fine;
    }
    let scale = mix(0.44, 1.0, gust) * select(1.0, 0.57, pond);
    return normalize(vec3<f32>(-(along * slope.x + side * slope.y) * scale, 1.0));
}

fn sky_radiance(ray: vec3<f32>) -> vec3<f32> {
    let horizon = 1.0 - clamp(ray.z, 0.0, 1.0);
    let day = smoothstep(-0.1, 0.38, g.sun.z);
    let sunset = (1.0 - smoothstep(0.03, 0.35, g.sun.z)) * day;
    let toward_sun = pow(max(0.0, dot(ray, g.sun.xyz)), 9.0);
    let blue = mix(vec3<f32>(0.29, 0.51, 0.73), vec3<f32>(0.63, 0.75, 0.83), horizon);
    // The sky fill responds to humidity and cover without drawing another
    // projected cloud sheet. Visible clouds come from the world-space volume.
    let clear = mix(vec3<f32>(0.10, 0.16, 0.27), blue, day)
        + vec3<f32>(0.32, 0.13, 0.04) * sunset * (0.3 + toward_sun);
    let grey = mix(vec3<f32>(0.11, 0.15, 0.23), vec3<f32>(0.58, 0.66, 0.73), day);
    return mix(clear, grey, g.shade.w * 0.60);
}

fn probe_face(origin: vec3<f32>, face: i32) -> vec4<f32> {
    let cell = clamp(origin.xy * g.terrain.z * 16.0 - 0.5, vec2<f32>(0.0), vec2<f32>(15.0));
    let base = vec2<i32>(floor(cell));
    let f = fract(cell);
    let next = min(base + vec2<i32>(1), vec2<i32>(15));
    let north = mix(textureLoad(reflection_probes, vec2<i32>(base.x * 6 + face, base.y), 0),
        textureLoad(reflection_probes, vec2<i32>(next.x * 6 + face, base.y), 0), f.x);
    let south = mix(textureLoad(reflection_probes, vec2<i32>(base.x * 6 + face, next.y), 0),
        textureLoad(reflection_probes, vec2<i32>(next.x * 6 + face, next.y), 0), f.x);
    return mix(north, south, f.y);
}

fn nearby_radiance(origin: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
    let sky = sky_radiance(ray);
    if (g.resolve.w < 0.5) {
        return sky;
    }
    let weight = ray * ray;
    let x = probe_face(origin, select(1, 0, ray.x >= 0.0));
    let y = probe_face(origin, select(3, 2, ray.y >= 0.0));
    let z = probe_face(origin, select(5, 4, ray.z >= 0.0));
    let local_tone = mix(g.tone.rgb, vec3<f32>(1.0), 0.18);
    let reflected = x.rgb * (weight.x * x.a) + y.rgb * (weight.y * y.a) + z.rgb * (weight.z * z.a);
    let coverage = dot(weight, vec3<f32>(x.a, y.a, z.a));
    return sky * (1.0 - coverage) + reflected * local_tone;
}

fn water_colour(kind: i32, world: vec3<f32>, px: f32, normal: vec3<f32>) -> vec3<f32> {
    let water_depth = max(0.0, world.z - terrain_height(world.xy));
    let shore = kind >= 3;
    let pond = kind == 2;
    let bottom = vec3<f32>(0.30, 0.60, 0.54);
    let deep = select(vec3<f32>(0.055, 0.23, 0.34), vec3<f32>(0.09, 0.31, 0.33), pond);
    let depth = max(water_depth, select(6.0, 0.3, shore));
    let transmitted = exp(-depth * select(0.32, 0.53, pond));
    let base = mix(deep, bottom, transmitted);
    let eye = normalize(g.eye.xyz - world);
    let ray = reflect(-eye, normal);
    let sky = nearby_radiance(world, ray);
    let facing = max(0.0, dot(normal, eye));
    let fresnel = 0.055 + 0.945 * pow(1.0 - facing, 5.0);
    let half_ray = normalize(eye + g.sun.xyz);
    let spec = pow(max(0.0, dot(half_ray, normal)), mix(117.0, 54.0, g.shade.w));
    let sparkle = spec * (0.27 + 0.55 * (1.0 - g.shade.w)) * lit(world, normal);
    var result = mix(base, sky, fresnel) + vec3<f32>(1.0, 0.86, 0.65) * sparkle;
    if (shore) {
        let t = g.screen.w;
        let fine = 1.0 - smoothstep(0.5, 2.0, px);
        var foam = vnoise(world.xy * 0.066 + vec2<f32>(t * 0.11, -t * 0.07));
        if (fine > 0.0) {
            foam = mix(foam, vnoise(world.xy * 0.43 + vec2<f32>(t * 0.22, -t * 0.14)), fine);
        }
        let bank = select((1.0 - smoothstep(0.28, 1.4, water_depth)) * 0.35, 0.88, kind == 5);
        result = mix(result, vec3<f32>(0.79, 0.89, 0.90),
            bank * smoothstep(0.38, 0.72, foam) * (0.60 + 0.40 * fine));
    }
    return min(result, vec3<f32>(1.2));
}

// Layer vegetation cover and patches of exposed soil across a real slope.
fn grass_soil(colour: vec3<f32>, world: vec3<f32>, normal: vec3<f32>, px: f32) -> vec3<f32> {
    let hill = smoothstep(0.03, 0.34, 1.0 - normal.z);
    let meadow_patch = vnoise(world.xy * 0.016);
    let sward = vnoise(world.xy * 0.18);
    let dry = smoothstep(0.43, 0.81, meadow_patch * 0.76 + sward * 0.24 + hill * 0.22);
    let bare = dry * (0.07 + 0.50 * hill) * (1.0 - g.wx.x);
    let detail = 1.0 - smoothstep(0.15, 0.65, px);
    var litter = 0.0;
    if (detail > 0.0) {
        litter = (vnoise(world.xy * 1.35) - 0.5) * detail;
    }
    let soil = vec3<f32>(0.44, 0.36, 0.26) * (1.0 - g.wx.y * 0.26);
    let meadow = colour * (0.90 + 0.20 * sward + litter * 0.15);
    return mix(meadow, soil, bare);
}

fn is_green(c: vec3<f32>) -> bool {
    return c.g > c.b + 0.08 && c.g > c.r - 0.06;
}

fn season_ground(c: vec3<f32>, season: i32) -> vec3<f32> {
    if (season == 1) { return vec3<f32>(c.r * 1.25 + 0.04, c.g * 0.82, c.b * 0.7); }
    if (season == 2) { return vec3<f32>(c.r * 1.06 + 0.02, c.g * 1.04 + 0.02, c.b * 1.05 + 0.03); }
    if (season == 3) { return vec3<f32>(c.r * 0.9, c.g * 0.96, c.b * 0.88); }
    return c;
}

// Leaf colour for spring (some trees in blossom) and summer (deeper green).
fn season_leaf(dark: bool, season: i32) -> vec3<f32> {
    if (season == 2) {

        if (dark) { return vec3<f32>(92.0, 158.0, 74.0) / 255.0; }
        return vec3<f32>(116.0, 180.0, 86.0) / 255.0;
    }
    if (dark) { return vec3<f32>(50.0, 112.0, 52.0) / 255.0; }
    return vec3<f32>(64.0, 134.0, 58.0) / 255.0;
}


// Natural ground of a land with a rock look: grass on gentle slopes, forest on the lower
// mountainsides, stone below faces, rock on steep slopes (stepped strata, or fractured granite),
// bare ground above the tree line and a clean snow cap above the snow line.
fn terrain_look(world: vec3<f32>, n: vec3<f32>, facet: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    // `base` is the gentle ground: grass for the mountain looks, the vertex colour for looks whose
    // flats, washes and beaches are coloured on the CPU
    let grass = base;
    if (g.rock[5].w < 0.5 || g.look.z > 0.5) {
        return grass;
    }
    let z = world.z;
    let s = sqrt(max(1.0 - n.z * n.z, 0.0)) / max(n.z, 0.01);
    let big = vnoise(world.xy / 11.0);
    let fine = vnoise(world.xy / 3.5);
    let wide = vnoise(world.xy / 90.0);
    let forest_s = g.rock[7].y;
    let tree_line = g.rock[5].y + (wide - 0.5) * 70.0;
    let rock_s = 0.7 + (big - 0.5) * 0.14;
    let talus_s = 0.45 + (big - 0.5) * 0.1;
    // the snow edge wanders slowly, so the cap has a clean outline
    let snow_line = g.rock[5].x - g.wx.x * g.rock[6].w + (wide - 0.5) * 50.0;
    if (z > snow_line && s < rock_s + 0.4) {
        return g.rock[6].rgb;
    }
    if (forest_s > 0.0 && z < tree_line && s > 0.14 && s < forest_s) {
        // forest canopy: two greens in clumps
        let c = vnoise(world.xy / 7.0);
        if (c > 0.55) {
            return vec3<f32>(52.0, 112.0, 62.0) / 255.0;
        }
        return vec3<f32>(66.0, 130.0, 70.0) / 255.0;
    }
    if (s > talus_s) {
        let tilt = world.x * 0.061 + world.y * 0.035;
        // Joints, cracks and fragments are drawn triplanar: each is projected along the dominant
        // axis of the true face normal, so a face looking along any axis keeps the scale of its
        // strata and cracks instead of smearing into streaks down the wall.
        let raw_w = pow(abs(facet), vec3<f32>(6.0)) + vec3<f32>(0.0001);
        let tw = raw_w / (raw_w.x + raw_w.y + raw_w.z);
        var cleavage = 0.0;
        var fragments = 0.0;
        if (tw.x > 0.02) {
            // a face looking along x: the wall runs along y
            cleavage += tw.x * vnoise(vec2<f32>(world.y / 14.0 + 3.1, z / 55.0));
            fragments += tw.x * vnoise(vec2<f32>(world.y / 3.6 + 7.7, z / 7.8));
        }
        if (tw.y > 0.02) {
            cleavage += tw.y * vnoise(vec2<f32>(world.x / 14.0, z / 55.0));
            fragments += tw.y * vnoise(vec2<f32>(world.x / 3.6, z / 7.8));
        }
        if (tw.z > 0.02) {
            // a top or a gentle slope: seen from above, with cells as wide as they are tall
            cleavage += tw.z * vnoise(world.xy / 14.0 + vec2<f32>(11.3, 5.9));
            fragments += tw.z * vnoise(world.xy / 3.6 + vec2<f32>(2.2, 9.4));
        }
        let lithology = vnoise(world.xy / 74.0 + vec2<f32>(z / 40.0, -z / 71.0));
        var colour = mix(g.rock[0].rgb, g.rock[3].rgb, smoothstep(0.12, 0.91, lithology));
        if (g.rock[7].x < 0.5) {
            let warp = (vnoise(world.xy / 60.0 + vec2<f32>(8.0, -1.0)) - 0.5) * 18.0;
            let layer = (z + tilt + warp + (fragments - 0.5) * 2.1) / max(2.0, g.rock[5].z);
            let seam = smoothstep(0.82, 1.0, fract(layer)) * smoothstep(0.30, 0.78, cleavage);
            let bed = 0.5 + 0.5 * sin(layer * 1.9 + lithology * 2.8);
            colour = mix(colour, g.rock[1].rgb, bed * 0.41) * (1.0 - seam * g.rock[7].w);
        }
        let fracture = 1.0 - smoothstep(0.055, 0.22, abs(cleavage - 0.48));
        colour = colour * (0.88 + fragments * 0.20 - fracture * 0.09);
        let talus = g.rock[4].rgb * (0.90 + fine * 0.16);
        let exposure = smoothstep(talus_s - 0.04, rock_s + 0.15, s);
        let gravel = smoothstep(talus_s, rock_s - 0.06, s);
        return mix(mix(grass, talus, gravel), colour, exposure);
    }
    if (forest_s <= 0.0 && s > 0.16) {
        // a gorge slope too gentle to be rock: dry scrub with bare stone showing through, not a
        // smooth green slide
        let blotch = vnoise(world.xy / 8.0);
        let grain = vnoise(world.xy / 2.6);
        let bare = 0.42 + (s - 0.16) * 0.6;
        if (blotch + (grain - 0.5) * 0.3 > 1.0 - bare) {
            var tc = g.rock[4].rgb;
            if (grain > 0.6) {
                tc = tc * 0.88;
            }
            return tc;
        }
        let dry = vec3<f32>(112.0, 128.0, 70.0) / 255.0;
        return mix(grass, dry, 0.72);
    }
    if (z > tree_line) {
        // alpine meadow thinning to scree with height
        let up = clamp((z - tree_line) / 180.0, 0.0, 1.0);
        if (big < up * 0.9) {
            return g.rock[4].rgb;
        }
        return mix(grass, vec3<f32>(128.0, 160.0, 96.0) / 255.0, 0.5);
    }
    return grass;
}

// World-space phase offsets spread activity across blocks. Each polygon also
// has a stable room anchor, independent of outline IDs assigned during rebuilds.
// A room's own roll, 0 to 1, from where it is: picks whether its light is on and what colour.
fn room_roll(anchor: vec3<f32>) -> f32 {
    let quantized = vec3<i32>(floor(anchor * 16.0 + 0.5));
    let room = hash_u(bitcast<u32>(quantized.x) ^ (bitcast<u32>(quantized.y) * 165557u)
        ^ (bitcast<u32>(quantized.z) * 224171u));
    return f32(room % 1000u) / 1000.0;
}

// The colour of a lit room: tungsten, warm white, cool white and now and then a screen's blue.
fn window_colour(roll: f32) -> vec3<f32> {
    let r = fract(roll * 7.31 + 0.37);
    var c = vec3<f32>(1.0, 0.70, 0.36);
    c = mix(c, vec3<f32>(1.0, 0.86, 0.58), smoothstep(0.28, 0.40, r));
    c = mix(c, vec3<f32>(0.90, 0.94, 1.0), smoothstep(0.72, 0.80, r));
    c = mix(c, vec3<f32>(0.62, 0.78, 1.0), smoothstep(0.93, 0.97, r));
    return c;
}

fn window_glow(anchor: vec3<f32>, shop: bool, office: bool) -> f32 {
    let block = vnoise(anchor.xy / 145.0);
    let choice = room_roll(anchor);
    // a third to two thirds of the windows are lit once the evening is in, block by block
    // offices keep 15 to 35 percent of their windows lit, homes 30 to 60, shopfronts a little more
    let share = select(mix(0.30, 0.60, block), mix(0.15, 0.35, block), office) + select(0.0, 0.08, shop);
    let schedule = clamp(g.tone.w / 0.75, 0.0, 1.0);
    // Individual rooms fade over several simulation minutes rather than crossing a binary cutoff.
    let awake = smoothstep(choice - 0.035, choice + 0.035, share * schedule);
    let brightness = 0.50 + 0.50 * fract(choice * 13.7);
    return awake * brightness * smoothstep(0.015, 0.18, g.tone.w);
}

fn night_illumination(world: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    let cell = vec2<i32>(floor(world.xy / 24.0) - g.local_lights.xy);
    if (any(cell < vec2<i32>(0)) || any(cell >= vec2<i32>(32))) {
        return light;
    }
    for (var i = 0; i < 6; i++) {
        let x = (cell.x * 6 + i) * 2;
        let source = textureLoad(lights_tex, vec2<i32>(x, cell.y), 0);
        if (source.w <= 0.0) {
            continue;
        }
        let beam = textureLoad(lights_tex, vec2<i32>(x + 1, cell.y), 0).xyz;
        let d = source.xyz - world;
        let distance2 = dot(d, d);
        if (distance2 >= beam.z * beam.z) {
            continue;
        }
        let to_light = d * inverseSqrt(max(distance2, 0.05));
        let aimed = dot(beam.xy, beam.xy) > 0.5;
        let axis = select(vec3<f32>(0.0, 0.0, -1.0), normalize(vec3<f32>(beam.xy, -0.16)), aimed);
        let cosine = dot(axis, -to_light);
        let cone = select(smoothstep(0.1, 0.62, cosine), smoothstep(0.68, 0.96, cosine), aimed);
        let angle = max(0.0, dot(normal, to_light));
        let reach = 1.0 - smoothstep(0.36, 1.0, sqrt(distance2) / beam.z);
        let intensity = source.w * cone * angle * reach / (distance2 + 7.0);
        let tint = select(vec3<f32>(1.0, 0.75, 0.43), vec3<f32>(0.74, 0.83, 1.0), aimed);
        light += tint * intensity;
    }
    return light * g.local_lights.z * 2.1;
}

fn road_weather_decals(w: vec2<f32>, footprint: f32) -> f32 {
    let grid = w / 11.0;
    let cell = vec2<i32>(floor(grid));
    let seed = hash2(cell);
    let offset = vec2<f32>(hash2(cell + vec2<i32>(3, 12)), hash2(cell + vec2<i32>(30, 5))) * 0.56 + 0.22;
    let p = (fract(grid) - offset) * 11.0;
    let spun = vec2<f32>(p.x + 0.41 * p.y, p.y - 0.41 * p.x);
    let repair = 1.0 - smoothstep(0.79, 1.05, length(spun * vec2<f32>(0.70, 1.3)));
    let wet = smoothstep(0.18, 0.85, g.wx.y);
    let stain = (1.0 - smoothstep(0.41, 0.82, length(spun * vec2<f32>(0.54, 0.92)))) * wet;
    let distance_fade = 1.0 - smoothstep(0.34, 0.81, footprint);
    return 1.0 - (repair * select(0.0, 0.13, seed > 0.72) + stain * 0.12) * distance_fade;
}

// Surface studies in world metres. Filter the grain and seams against the pixel footprint.
fn textured_surface(col: vec3<f32>, base: vec3<f32>, world: vec3<f32>, normal: vec3<f32>, px: f32, cel: bool, object: bool) -> vec3<f32> {
    let w = world.xy;
    let aggregate = 1.0 - smoothstep(0.09, 0.28, px);
    if (near(base, 108.0, 108.0, 120.0)) {
        let repair = vnoise(w * 0.34);
        let dust = vnoise(w * 0.95 + vec2<f32>(19.0, 7.0));
        let specks = hash2(vec2<i32>(floor(w * 7.0))) - 0.5;
        // old repairs: now and then a squared-off patch of newer, darker asphalt
        let slab = w / vec2<f32>(11.0, 7.0);
        let slab_id = vec2<i32>(floor(slab));
        let slab_roll = hash2(slab_id + vec2<i32>(91, 13));
        let inset = abs(fract(slab) - 0.5);
        let mend = step(0.83, slab_roll) * (1.0 - smoothstep(0.26, 0.29, max(inset.x, inset.y)))
            * (1.0 - smoothstep(0.3, 1.2, px));
        var paved = col * vec3<f32>(0.80, 0.80, 0.76)
            * (0.95 + 0.05 * repair + 0.06 * dust + 0.20 * specks * aggregate - 0.06 * mend);
        if (g.wx.y > 0.05) {
            let pool = vnoise(w * 0.54) * 0.7 + vnoise(w * 1.32) * 0.3;
            let puddle = smoothstep(0.77 - 0.15 * g.wx.y, 0.84 - 0.15 * g.wx.y, pool) * g.wx.y;
            let eye = normalize(g.eye.xyz - world);
            let reflected = reflect(-eye, normal);
            let sheen = 0.18 + 0.68 * pow(1.0 - max(0.0, dot(eye, normal)), 3.0);
            let broken_sky = nearby_radiance(world, reflected) * sheen;
            let fine = w * 1.9;
            let tile = vec2<i32>(floor(fine));
            let offset = vec2<f32>(hash2(tile), hash2(tile + vec2<i32>(17, 31))) * 0.5 + 0.25;
            let life = fract(g.screen.w * 1.65 + hash2(tile + vec2<i32>(31, 7)));
            let radius = length(fract(fine) - offset);
            let ring = 1.0 - smoothstep(0.025, 0.105, abs(radius - (0.06 + 0.38 * life)));
            let ripple = ring * (1.0 - life) * g.precipitation.x;
            paved = mix(paved, broken_sky * (1.0 + ripple * 0.47), puddle * (0.50 + 0.16 * dust));
        }
        return paved * road_weather_decals(w, px);
    }
    if (near(base, 194.0, 190.0, 182.0) || near(base, 206.0, 202.0, 194.0)) {
        let cell = w / vec2<f32>(1.8, 0.95);
        let in_tile = fract(cell);
        let seam = 1.0 - smoothstep(0.01, 0.10,
            min(min(in_tile.x, 1.0 - in_tile.x), min(in_tile.y, 1.0 - in_tile.y)));
        let variation = hash2(vec2<i32>(floor(cell))) - 0.5;
        let fade = 1.0 - smoothstep(0.2, 0.62, px);
        let stone = vec3<f32>(0.76, 0.73, 0.68) * (1.0 - 0.22 * g.wx.y);
        var paving = stone * (0.97 + 0.08 * vnoise(w * 0.32) + fade * (0.11 * variation - 0.22 * seam));
        return paving;
    }
    if (near(base, 128.0, 104.0, 84.0) || near(base, 198.0, 194.0, 186.0) || is_sidewalk(base)) {
        let grains = hash2(vec2<i32>(floor(w * 6.0)));
        return col * (0.92 + 0.14 * vnoise(w * 1.2) + (grains - 0.5) * 0.24 * aggregate);
    }
    if (near(base, 83.0, 69.0, 58.0)) {
        return col * (0.89 + 0.22 * vnoise(w * vec2<f32>(5.0, 1.4)));
    }
    if (!object || !cel || px > 0.49) {
        return col;
    }
    var col_weather = 1.0;
    let fade = 1.0 - smoothstep(0.15, 0.49, px);
    if (normal.z > 0.95 && !is_green(base) && base.r + base.g + base.b < 2.5) {
        // a flat roof: gravel, the seams of the roofing sheets and old water stains
        let sheet = w / 3.4;
        let edge = min(min(fract(sheet.x), 1.0 - fract(sheet.x)), min(fract(sheet.y), 1.0 - fract(sheet.y)));
        let seam = 1.0 - smoothstep(0.0, 0.035, edge);
        let stain = vnoise(w * 0.21 + vec2<f32>(7.0, 3.0));
        let gravel = hash2(vec2<i32>(floor(w * 5.0))) - 0.5;
        return col * (1.0 + fade * (0.10 * (stain - 0.5) + 0.12 * gravel * aggregate - 0.13 * seam));
    }
    if (abs(normal.z) < 0.12) {
        // walls weather: faint streaks run down them and the foot of the wall is darker
        let run = dot(w, vec2<f32>(-normal.y, normal.x));
        let streak = vnoise(vec2<f32>(run * 1.3, world.z * 0.11));
        col_weather = 1.0 + fade * (0.05 * (streak - 0.5));
    }
    if (abs(normal.z) < 0.08 && base.r - base.g > 0.11 && base.g - base.b > 0.03
        && base.r < 0.83 && base.g < 0.65) {
        let along = dot(w, vec2<f32>(-normal.y, normal.x));
        let course = world.z / 0.42;
        let block = along / 0.86 + 0.5 * (floor(course) % 2.0);
        let edge = min(min(fract(course), 1.0 - fract(course)), min(fract(block), 1.0 - fract(block)));
        let mortar = 1.0 - smoothstep(0.02, 0.13, edge);
        let kiln = hash2(vec2<i32>(i32(floor(block)), i32(floor(course))));
        return mix(col * (1.0 + (kiln - 0.5) * 0.23 * fade), vec3<f32>(0.64, 0.59, 0.52), mortar * fade * 0.4) * col_weather;
    }
    if (normal.z > 0.42 && normal.z < 0.94 && !is_green(base)) {
        let run = normalize(normal.xy);
        let row = dot(w, run) / 0.52;
        let seam = 1.0 - smoothstep(0.0, 0.16, fract(row));
        let tile = vec2<i32>(i32(floor(dot(w, vec2<f32>(-run.y, run.x)) * 1.8 + 0.5 * floor(row))), i32(floor(row)));
        return col * (1.0 + fade * ((hash2(tile) - 0.5) * 0.22 - seam * 0.24));
    }
    return col * col_weather;
}

// Window glass: a dark room behind, a band of sky light across the upper pane, a diagonal
// glare and a shadow under the head of the opening. `pane` is 0 to 1 across and up the window.
fn glass_look(base: vec3<f32>, pane: vec2<f32>, seed: f32, world: vec3<f32>, n: vec3<f32>, px: f32, neutral: f32) -> vec3<f32> {
    let day = smoothstep(-0.05, 0.3, g.sun.z);
    let eye = normalize(g.eye.xyz - world);
    let grazing = pow(1.0 - max(0.0, dot(eye, n)), 2.0);
    var sky = sky_radiance(normalize(vec3<f32>(n.xy, 0.55)));
    sky = mix(sky, vec3<f32>(dot(sky, vec3<f32>(0.3, 0.59, 0.11))) * vec3<f32>(1.0, 0.98, 0.95), 0.6 * neutral);
    let room = base * mix(vec3<f32>(0.34, 0.40, 0.50), vec3<f32>(0.40, 0.40, 0.41), neutral);
    // some panes have blinds drawn part of the way down
    let blind = step(0.72, seed) * (1.0 - smoothstep(0.45 + 0.4 * fract(seed * 7.3), 0.5 + 0.4 * fract(seed * 7.3), 1.0 - pane.y));
    let sharp = 1.0 - smoothstep(0.25, 0.9, px);
    var col = mix(room, sky * (0.72 + 0.3 * seed), (0.30 + 0.45 * grazing) * smoothstep(0.15, 0.95, pane.y));
    col = mix(col, vec3<f32>(0.86, 0.84, 0.78) * (0.55 + 0.35 * day), blind * 0.8 * sharp);
    let band = pane.x * 0.9 + pane.y - 0.55 - 0.5 * seed;
    let glare = (1.0 - smoothstep(0.0, 0.13, abs(band))) + 0.5 * (1.0 - smoothstep(0.0, 0.05, abs(band - 0.26)));
    col += vec3<f32>(0.20, 0.23, 0.25) * 0.5 * glare * day * sharp * (0.5 + 0.5 * max(0.0, dot(n.xy, g.sun.xy)));
    // the head of the opening shades the top of the glass
    col *= 1.0 - 0.30 * smoothstep(0.84, 0.98, pane.y) * sharp;
    return mix(base * mix(vec3<f32>(0.62, 0.70, 0.80), vec3<f32>(0.70, 0.71, 0.72), neutral), col, 0.35 + 0.65 * sharp);
}

// Finish codes come from Obj::Finish. These materials are projected in metres
// so masonry and windows keep their scale across camera movement.
fn dressed_surface(col: vec3<f32>, finish: u32, world: vec3<f32>, anchor: vec3<f32>,
    normal: vec3<f32>, px: f32) -> vec3<f32> {
    if (px >= 0.5) { return col; }
    let roof = finish >= 247u && finish <= 249u;
    let offset = world - anchor;
    let tangent = normalize(vec2<f32>(-normal.y, normal.x) + vec2<f32>(0.00001, 0.0));
    let wall = vec2<f32>(dot(offset.xy, tangent), offset.z);
    let uv = select(wall, offset.xy, roof);
    let fade = 1.0 - smoothstep(0.10, 0.5, px);
    let mottling = vnoise(uv * 0.76 + anchor.xy * 0.17);
    var pattern = 0.91 + 0.15 * mottling;
    if (finish == 240u || finish == 247u) {
        let batten = select(uv.y / 0.22, uv.x / 0.58, roof);
        let seam = 1.0 - smoothstep(0.02, 0.12, fract(batten));
        pattern -= seam * 0.20 * (1.0 - smoothstep(0.05, 0.26, px));
    } else if (finish == 241u || finish == 248u) {
        let scale = select(vec2<f32>(0.50, 0.25), vec2<f32>(0.37, 0.24), roof);
        let row = uv.y / scale.y;
        let block = vec2<f32>(uv.x / scale.x + 0.5 * (floor(row) % 2.0), row);
        let f = fract(block);
        let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
        let mortar = (1.0 - smoothstep(0.013, 0.10, edge)) * (1.0 - smoothstep(0.05, 0.26, px));
        let piece = hash2(vec2<i32>(floor(block))) - 0.5;
        pattern = pattern * (1.0 + piece * 0.22) - mortar * 0.19;
    } else if (finish == 249u) {
        let seam = 1.0 - smoothstep(0.01, 0.075, fract(uv.y / 2.1));
        pattern -= seam * 0.16;
    }
    return col * mix(1.0, pattern, fade);
}

struct FacadeLook {
    colour: vec3<f32>,
    glass: f32,
    glow: f32,
    roll: f32,
};

fn facing_facade(col: vec3<f32>, finish: u32, world: vec3<f32>, anchor: vec3<f32>,
    normal: vec3<f32>, px: f32) -> FacadeLook {
    let warm = finish == 244u || finish == 246u;
    let dark = finish == 251u;
    let glass_base = mix(select(select(vec3<f32>(0.22, 0.35, 0.45), vec3<f32>(0.20, 0.22, 0.24), warm), vec3<f32>(0.13, 0.15, 0.18), dark), col, 0.42);
    let detail = 1.0 - smoothstep(0.34, 0.9, px);
    let average = mix(col, glass_base, select(0.45, 0.26, warm));
    if (detail <= 0.0) {
        return FacadeLook(average, 0.0, 0.0, 0.0);
    }
    let tangent = vec2<f32>(-normal.y, normal.x);
    let offset = world - anchor;
    let storey = select(3.5, 3.9, finish == 244u);
    let ribbon = finish == 250u;
    let balcony = finish == 245u;
    let spacing = select(select(2.55, 2.05, ribbon || finish == 244u), 3.15, finish == 246u || balcony);
    let uv = vec2<f32>(dot(offset.xy, tangent) / spacing, offset.z / storey);
    let cell = floor(uv);
    let part = fract(uv);
    // the frames and piers between windows thin out at night so the material still shows
    let night = clamp(g.look.w * 1.25, 0.0, 1.0);
    let reveal = select(select(0.10, 0.22, warm || balcony), 0.048, ribbon) * (1.0 - 0.75 * night);
    let border = max(0.014, px / spacing * 0.36);
    let jamb = min(part.x, 1.0 - part.x);
    let sill = select(select(0.15, 0.31, ribbon), 0.37, balcony) * (1.0 - 0.45 * night);
    let head = mix(select(0.93, 0.83, ribbon || balcony), 0.975, 0.7 * night);
    let glazing = smoothstep(reveal - border, reveal + border, jamb)
        * smoothstep(sill - border, sill + border, part.y)
        * (1.0 - smoothstep(head - border, head + border, part.y));
    var frame = 1.0;
    if (finish == 244u || finish == 246u) {
        frame -= (1.0 - smoothstep(0.012, 0.04 + border, abs(part.x - 0.5))) * 0.50;
    }
    let glass = glazing * frame * (1.0 - 0.0 * night);
    let seed_cell = cell + floor(anchor.xy * 1.7 + anchor.z * vec2<f32>(0.53, 1.07));
    let pane_seed = hash2(vec2<i32>(seed_cell));
    let inner = vec2<f32>(
        clamp((part.x - reveal) / max(0.05, 1.0 - 2.0 * reveal), 0.0, 1.0),
        clamp((part.y - sill) / max(0.05, head - sill), 0.0, 1.0));
    let tint = glass_look(glass_base * 1.5, inner, pane_seed, world, normal, px, select(0.0, 1.0, warm));
    let ledge = 1.0 - smoothstep(0.018, 0.077 + border * 0.3, min(part.y, 1.0 - part.y));
    let ribs = select(0.08, 0.20, finish == 244u || balcony);
    let rail = (1.0 - smoothstep(0.028, 0.078, abs(part.y - 0.22))) * select(0.0, 0.20, balcony);
    let fin = (1.0 - smoothstep(0.06, 0.16, jamb)) * select(0.0, 0.18, finish == 244u);
    let cladding = col * (0.94 + 0.09 * pane_seed + ledge * ribs * (1.0 - 0.6 * night) + rail + fin);
    var glow = 0.0;
    var roll = 0.0;
    if (!dark && g.tone.w > 0.0 && glass > 0.01) {
        let room_offset = tangent * ((cell.x + 0.5) * spacing);
        let room = anchor + vec3<f32>(room_offset, (cell.y + 0.5) * storey);
        glow = window_glow(room, finish == 246u, finish == 250u || finish == 251u || finish == 243u) * glass * detail;
        roll = room_roll(room);
        // blinds drawn down from the head of some lit windows, with slats, and a flickering
        // screen light in a few
        let blinds = step(0.62, fract(roll * 3.7 + 0.13));
        let drawn = 0.25 + 0.55 * fract(roll * 9.1);
        let covered = blinds * smoothstep(1.0 - drawn - 0.03, 1.0 - drawn + 0.03, inner.y);
        let slats = 0.70 + 0.30 * step(0.5, fract(inner.y * 9.0));
        glow = glow * mix(1.0, 0.30 * slats, covered);
    }
    // dark windows read as a pattern of black squares at night: unlit glass settles most of the
    // way to the wall colour so only the lit rooms stand out
    let unlit_tint = mix(tint, cladding * 0.92, 0.62 * night);
    return FacadeLook(mix(average, mix(cladding, unlit_tint, glass), detail), glass * detail, glow, roll);
}

fn previous_season(season: i32) -> i32 {
    switch (season) {
        case 0: { return 1; }
        case 1: { return 3; }
        case 2: { return 0; }
        default: { return 2; }
    }
}

// A stable landscape field staggers changes across neighboring plots without
// moving palette thresholds when the camera pans.
fn season_phase(world: vec3<f32>) -> f32 {
    let progress = g.shade.z;
    if (progress <= 0.0 || progress >= 1.0) {
        return progress;
    }
    let site = vnoise(world.xy * 0.010);
    let offset = (site - 0.5) * 2.2;
    return clamp(progress + offset * progress * (1.0 - progress), 0.0, 1.0);
}

fn field_palette(step: i32, season: i32) -> vec3<f32> {
    var colour = select(vec3<f32>(214.0, 178.0, 66.0), vec3<f32>(202.0, 164.0, 54.0), step == 1);
    switch (season) {
        case 0: { colour = vec3<f32>(150.0, 134.0, 100.0); }
        case 1: { colour = vec3<f32>(178.0, 148.0, 88.0); }
        case 2: { colour = vec3<f32>(130.0, 176.0, 84.0); }
        default: {}
    }
    return colour / 255.0;
}

fn groundcover_palette(step: i32, season: i32) -> vec3<f32> {
    let summer = array<vec3<f32>, 4>(
        vec3<f32>(92.0, 158.0, 62.0), vec3<f32>(106.0, 172.0, 72.0),
        vec3<f32>(80.0, 146.0, 58.0), vec3<f32>(120.0, 176.0, 76.0));
    let spring = array<vec3<f32>, 4>(
        vec3<f32>(118.0, 184.0, 84.0), vec3<f32>(132.0, 194.0, 96.0),
        vec3<f32>(104.0, 176.0, 76.0), vec3<f32>(146.0, 196.0, 100.0));
    let autumn = array<vec3<f32>, 4>(
        vec3<f32>(200.0, 172.0, 92.0), vec3<f32>(182.0, 146.0, 78.0),
        vec3<f32>(212.0, 188.0, 112.0), vec3<f32>(166.0, 140.0, 76.0));
    let winter = array<vec3<f32>, 4>(
        vec3<f32>(160.0, 146.0, 106.0), vec3<f32>(148.0, 134.0, 98.0),
        vec3<f32>(172.0, 158.0, 118.0), vec3<f32>(138.0, 128.0, 96.0));
    let i = step & 3;
    switch (season) {
        case 0: { return winter[i]; }
        case 1: { return autumn[i]; }
        case 2: { return spring[i]; }
        default: { return summer[i]; }
    }
}

fn leaf_palette(base: vec3<f32>, step: i32, dark: bool, season: i32) -> vec3<f32> {
    if (season == 1) { return autumn_leaf(step, dark); }
    if (season == 0) { return base; }
    return season_leaf(dark, season);
}


// ---- winter dressing ----

fn is_bark(c: vec3<f32>) -> bool {
    return near(c, 134.0, 114.0, 98.0);
}

// Bushes, hedges and yard shrubs: greens of their own, not the broadleaf colours, so they stay
// in leaf all winter under a heavy coat of snow.
fn is_shrub(c: vec3<f32>) -> bool {
    return near(c, 66.0, 122.0, 64.0) || near(c, 84.0, 140.0, 70.0)
        || leaf_step(c, 58.0, 116.0, 62.0) >= 0 || leaf_step(c, 48.0, 100.0, 56.0) >= 0;
}

// The sidewalk strip is drawn in the sidewalk colour with its blue running 181 to 191 across it
// (outer edge high), so winter can wear a footpath into it. Plain 186 is a sidewalk with no path.
fn is_sidewalk(c: vec3<f32>) -> bool {
    let d = c * 255.0;
    return abs(d.x - 198.0) < 0.6 && abs(d.y - 194.0) < 0.6 && d.z > 180.0 && d.z < 191.6;
}

// 0 not paving, 1 sidewalk, 2 plaza and forecourt paving, 3 kerb and barrier stone, 4 gravel shoulder.
fn paving_kind(c: vec3<f32>) -> i32 {
    if (is_sidewalk(c)) { return 1; }
    if (near(c, 206.0, 202.0, 194.0) || near(c, 194.0, 192.0, 202.0) || near(c, 208.0, 200.0, 198.0)
        // the darker squares of the downtown checker (0.93 of each paving colour) are paving too,
        // or they would take the snow differently and show a checkerboard under it
        || near(c, 194.0, 190.0, 182.0) || near(c, 191.0, 188.0, 180.0) || near(c, 180.0, 178.0, 188.0)
        || near(c, 193.0, 186.0, 184.0)
        // the town square: sand, light and dark flags and a pale edge
        || near(c, 216.0, 198.0, 162.0) || near(c, 204.0, 200.0, 192.0) || near(c, 188.0, 184.0, 178.0)
        || near(c, 226.0, 222.0, 214.0)) { return 2; }
    if (near(c, 166.0, 162.0, 158.0) || near(c, 170.0, 168.0, 164.0)) { return 3; }
    if (near(c, 150.0, 141.0, 118.0)) { return 4; }
    return 0;
}

// Snow on paving: sidewalks lie under snow with a trodden footpath worn along them, plazas keep
// winding desire lines through the snow, kerb stone and shoulders are mostly buried. Cleared and
// trodden snow shows wet stone, never grass.
fn paving_snow(col: vec3<f32>, kind: i32, base: vec3<f32>, world: vec3<f32>, px: f32) -> vec3<f32> {
    let cover = smoothstep(0.06, 0.7, g.wx.x);
    if (cover <= 0.0 || kind == 0) { return col; }
    let w = world.xy;
    let lumps = vnoise(w / 2.4 + vec2<f32>(5.0, 1.0)) * 0.6 + vnoise(w / 0.8) * 0.4;
    var amount = 0.0;
    var trodden = 0.0;
    if (kind == 1) {
        amount = 0.86;
        // the footpath: a band along the strip, with gaps where it is drifted over
        let u = clamp((base.b * 255.0 - 181.0) / 10.0, 0.0, 1.0);
        let off = abs(base.b * 255.0 - 186.0) < 0.5;
        let band = 1.0 - smoothstep(0.10, 0.19, abs(u - 0.70));
        trodden = select(band, 0.0, off) * smoothstep(0.18, 0.40, lumps + 0.35);
        amount = amount * (0.88 + 0.12 * lumps);
    } else if (kind == 2) {
        // an even blanket with a little texture, so no tile reads as bare while its neighbour is white
        amount = 0.90 * (0.92 + 0.08 * lumps);
        // wide cleared paths across the open paving, and two wheel tracks along the lot
        let line = abs(vnoise(w / 17.0 + vec2<f32>(3.0, 8.0)) - 0.5);
        let line2 = abs(vnoise(w / 23.0 + vec2<f32>(9.0, 2.0)) - 0.5);
        let paths = max(1.0 - smoothstep(0.008, 0.024, line), 1.0 - smoothstep(0.007, 0.02, line2));
        let lane = fract(w.y / 11.0 + 0.3 * vnoise(vec2<f32>(w.x / 30.0, 1.0)));
        let wheels = max(1.0 - smoothstep(0.02, 0.05, abs(lane - 0.40)), 1.0 - smoothstep(0.02, 0.05, abs(lane - 0.56)))
            * smoothstep(0.35, 0.6, vnoise(vec2<f32>(w.x / 14.0, w.y / 40.0) + vec2<f32>(2.0, 5.0)));
        trodden = max(paths * 0.85, wheels * 0.6);
    } else if (kind == 3) {
        amount = 0.62 * smoothstep(0.1, 0.5, lumps);
    } else {
        amount = 0.80 * smoothstep(0.05, 0.4, lumps + 0.25);
    }
    let wet = col * vec3<f32>(0.80, 0.82, 0.86);
    var c = mix(col, vec3<f32>(0.945, 0.962, 0.984), amount * cover * (1.0 - trodden));
    c = mix(c, wet, trodden * cover * 0.8);
    // the packed edge of a footpath: a slightly bluer rim of heaped snow
    return c;
}

// Snow on any surface that faces up, by the world-up direction of the face rather than by asset:
// roofs, ledges, tower tops, setbacks, lawns, car roofs and the top of every branch. Paving is
// only dusted and pines keep their dark green underneath a white cap.
fn snow_on_surface(col: vec3<f32>, base: vec3<f32>, world: vec3<f32>, n: vec3<f32>, exempt: bool) -> vec3<f32> {
    if (crown_colour(base)) {
        return col;
    }
    let reach = g.wx.x * 1.3 - 0.15;
    let pine = near(base, 46.0, 104.0, 72.0);
    let bark = is_bark(base);
    let shrub = is_shrub(base);
    // neighbourhoods fill at slightly different depths
    let h = vnoise(world.xy / 21.0) * 0.5 + 0.12;
    let cover = smoothstep(h - 0.12, h + 0.12, reach);
    var up = smoothstep(0.30, 0.75, n.z);
    if (pine) {
        // heavy snow on the upper part of each tier (the normal swings up towards the tip of a
        // smooth cone), and a patchy dusting on the flat faces of the street pines
        let patchy = 0.55 * smoothstep(0.3, 0.5, n.z)
            * smoothstep(0.30, 0.55, vnoise(world.xy * 1.4 + vec2<f32>(world.z * 1.1, 2.0)));
        up = max(smoothstep(0.64, 0.84, n.z), patchy);
    }
    if (bark) {
        // snow rides the top of a branch, and a thin line of it on the upper flank
        up = smoothstep(-0.15, 0.40, n.z);
    }
    if (shrub) {
        // a bush under snow: the top is white and the sides are plastered in clumps, with a
        // little dark green showing through
        let clumps = smoothstep(0.25, 0.55, vnoise(world.xy * 2.1 + vec2<f32>(world.z * 3.0, 1.0)));
        up = max(smoothstep(0.35, 0.8, n.z), 0.80 * clumps);
    }
    var amount = up * cover;
    if (bark) {
        amount = amount * (0.72 + 0.28 * vnoise(world.xy * 1.7 + vec2<f32>(world.z * 2.0, 0.0)));
    }
    let sat = max(base.r, max(base.g, base.b)) - min(base.r, min(base.g, base.b));
    let pk = paving_kind(base);
    if (pk != 0 && n.z > 0.8) {
        return paving_snow(col, pk, base, world, 0.1);
    }
    if (exempt && n.z > 0.9 && sat < 0.12 && !is_green(base) && !bark) {
        // paving: a dusting with cleared patches, not a blanket
        amount = amount * 0.7 * smoothstep(0.28, 0.58, vnoise(world.xy / 2.3 + vec2<f32>(4.0, 9.0)));
    }
    return mix(col, vec3<f32>(0.945, 0.962, 0.984), amount);
}

// Surface of open snow: long soft drifts laid by the wind, lighter on their crests and a little
// cooler in the hollows, fading out as a pixel covers more ground.
fn snow_relief(col: vec3<f32>, w: vec2<f32>, amount: f32) -> vec3<f32> {
    let wind = g.wx.zw;
    let wl = length(wind);
    let dir = select(vec2<f32>(0.8, 0.6), wind / max(wl, 0.01), wl > 0.5);
    let along = dot(w, dir);
    let across = dot(w, vec2<f32>(-dir.y, dir.x));
    let ridge = vnoise(vec2<f32>(along / 13.0, across / 3.4)) * 0.55
        + vnoise(vec2<f32>(along / 4.6, across / 1.25)) * 0.30
        + vnoise(w / 0.7) * 0.15;
    let hollow = 1.0 - smoothstep(0.32, 0.55, ridge);
    let broad = vnoise(w / 46.0 + vec2<f32>(11.0, 4.0)) * 0.65 + vnoise(w / 19.0 + vec2<f32>(2.0, 7.0)) * 0.35;
    var c = col * (0.93 + 0.14 * ridge) * (0.90 + 0.16 * broad);
    c = mix(c, c * vec3<f32>(0.88, 0.93, 1.04), hollow * 0.75);
    c = mix(c, c * vec3<f32>(0.96, 0.97, 0.94), smoothstep(0.62, 0.8, broad) * 0.5);
    return mix(col, c, amount);
}

// The ploughed banks along a kerb (kind 1) and the worn lanes of a cleared road (kind 2). The
// geometry carries a position across the strip in its blue channel.
fn road_dress(kind: i32, kb: vec3<f32>, world: vec3<f32>, n: vec3<f32>, px: f32, gradb: vec2<f32>) -> vec3<f32> {
    let w = world.xy;
    let cover = smoothstep(0.06, 0.7, g.wx.x) * select(0.0, 1.0, g.look.y > 0.5);
    if (kind == 3) {
        // junction paving: the same wet asphalt with slush lying in patches
        let asphalt = vec3<f32>(108.0, 108.0, 120.0) / 255.0;
        let paved = textured_surface(asphalt, asphalt, world, n, px, true, false);
        let patch_ = vnoise(w / 3.0) * 0.6 + vnoise(w / 1.0) * 0.4;
        let slush = cover * 0.30 * smoothstep(0.42, 0.74, patch_);
        let wet_patch = cover * smoothstep(0.30, 0.58, vnoise(w / 4.5 + vec2<f32>(6.0, 2.0)));
        let wetter = mix(paved, paved * vec3<f32>(0.62, 0.64, 0.70), wet_patch * 0.8);
        return mix(wetter, vec3<f32>(0.70, 0.72, 0.76), slush);
    }
    if (kind == 1) {
        let t = clamp((kb.b - 140.0) / 113.0, 0.0, 1.0);
        let wob = vnoise(w / 3.3) * 0.6 + vnoise(w / 1.1) * 0.4;
        // the outer edge of the bank wanders, and the bank grows with the cover
        let width = (0.55 + 0.45 * wob) * smoothstep(0.02, 0.45, cover);
        if (t > width + 0.03 || cover < 0.03) {
            discard;
        }
        // a heaped ridge: it rises from the road side, crests, and slopes off to the outer side.
        // The fake slope is lit by the sun so the bank reads as relief, not a flat band.
        let tt = t / max(width, 0.1);
        let slope = select(-1.0, 1.0, tt < 0.42) * (1.0 - smoothstep(0.0, 0.5, abs(tt - 0.42)));
        let dir = normalize(gradb + vec2<f32>(0.0001, 0.0));
        let nn = normalize(vec3<f32>(-dir * slope * 1.3, 1.0));
        let relief = clamp(dot(nn, g.sun.xyz) - g.sun.z, -0.5, 0.5);
        // dirty grey at the road side, clean white on the crest
        var c = mix(vec3<f32>(0.58, 0.57, 0.58), vec3<f32>(0.82, 0.86, 0.93), smoothstep(0.02, 0.18, tt));
        c = mix(c, vec3<f32>(0.955, 0.968, 0.99), smoothstep(0.16, 0.40, tt));
        c *= 1.0 + relief * 0.8 - 0.14 * (1.0 - smoothstep(0.06, 0.5, tt)) * vnoise(w * 2.6);
        return c;
    }
    let hw = (kb.r - 200.0) / 4.0;
    let u = clamp((kb.b - 20.0) / 230.0, 0.0, 1.0);
    let asphalt = vec3<f32>(108.0, 108.0, 120.0) / 255.0;
    let paved = textured_surface(asphalt, asphalt, world, n, px, true, false);
    let x = abs(u * 2.0 - 1.0) * hw;
    let lanes = max(1.0, round(hw / 3.6));
    let lw = hw / lanes;
    let c = x - (floor(x / lw) + 0.5) * lw;
    let detail = 1.0 - smoothstep(0.10, 0.45, px);
    // two wheel paths in every lane, bare and wet, with snow left between them and at the kerb
    let track = (1.0 - smoothstep(0.10, 0.36, abs(abs(c) - 0.88))) * detail;
    let kerb = smoothstep(hw - 1.6, hw - 0.5, x);
    let crown = mix(0.45, 1.0 - smoothstep(0.3, 0.9, abs(c)), detail);
    let broken = 0.85 + 0.3 * vnoise(w / 2.6 + vec2<f32>(3.0, 1.0));
    let slush = cover * clamp((0.07 + 0.24 * crown + 0.72 * kerb) * (1.0 - 0.8 * track) * broken, 0.0, 0.9);
    let wet = mix(paved, paved * 0.80, track * cover);
    // a pale salt haze dries out along the edges in broken streaks
    let edge_band = smoothstep(hw - 4.2, hw - 1.4, x) * (1.0 - smoothstep(hw - 1.0, hw - 0.3, x));
    let streak = smoothstep(0.35, 0.70, vnoise(vec2<f32>(w.x / 2.4, w.y / 2.4) + vec2<f32>(4.0, 9.0)));
    let haze = cover * edge_band * (0.20 + 0.20 * streak);
    let salted = mix(wet, vec3<f32>(0.66, 0.67, 0.69), haze);
    // grey slush where the road meets the bank
    let gutter = cover * smoothstep(hw - 0.9, hw - 0.1, x);
    let dirty = mix(salted, vec3<f32>(0.50, 0.50, 0.52), gutter * 0.55);
    return mix(dirty, vec3<f32>(0.70, 0.71, 0.73), slush * (1.0 - gutter * 0.5));
}

@fragment
fn fs_main(v: VOut) -> FOut {
    var n = normalize(v.normal);
    var l = dot(n, g.sun.xyz);
    // the true normal of the triangle: steep rock is shaded in flat facets, not in the smooth
    // gradients that interpolated vertex normals give on a coarse grid
    let world_dx = dpdx(v.world);
    let world_dy = dpdy(v.world);
    var gradb = vec2<f32>(1.0, 0.0);
    {
        // how the blue channel (a marker for the position across a strip) grows over the ground
        let db = vec2<f32>(dpdx(v.color.b), dpdy(v.color.b)) * 255.0;
        let det = world_dx.x * world_dy.y - world_dx.y * world_dy.x;
        if (abs(det) > 1e-9) {
            gradb = vec2<f32>(db.x * world_dy.y - db.y * world_dx.y,
                -db.x * world_dy.x + db.y * world_dx.x) / det;
        }
    }
    var facet = normalize(cross(world_dx, world_dy));
    if (dot(facet, n) < 0.0) {
        facet = -facet;
    }
    var rc = v.color.rgb;
    if (v.id == 0u && v.color.a > 0.5 && rc.r > 0.72 && abs(rc.g - 0.5) < 0.06 && rc.b < 0.34) {
        // the road works edge: a dull striped barrier, never a bright orange line
        let stripe = step(0.5, fract((v.world.x + v.world.y) / 1.4));
        rc = mix(vec3<f32>(0.30, 0.28, 0.27), vec3<f32>(0.55, 0.41, 0.31), stripe);
    }
    var haze = false;
    let hz = max(haze_step(rc, 88.0, 150.0, 76.0), haze_step(rc, 70.0, 128.0, 66.0));
    if (hz >= 0) {
        // a far crown: the same colour as the near one, eight steps up in blue
        haze = true;
        rc.b = rc.b - 8.0 / 255.0;
    }
    var col = rc;
    if (is_bark(rc)) {
        // bark runs from warm brown to silver grey from tree to tree, and a little in lightness
        let cell = vec2<i32>(floor(v.world.xy / 4.5));
        col = mix(vec3<f32>(0.55, 0.45, 0.36), vec3<f32>(0.52, 0.50, 0.48), hash2(cell))
            * (0.88 + 0.22 * hash2(cell + vec2<i32>(7, 3)));
    }
    let a = v.color.a;
    let code = u32(round(a * 255.0));
    let finish = select(0u, code, code >= 240u && code <= 251u);
    let cel = a > 0.5;
    let keep = finish == 0u && a > 0.001 && a < 0.999;
    let ground = v.id == 0u;
    let exempt = v.id == 16777215u;
    var dress = 0;
    if (ground && cel) {
        let kb = rc * 255.0;
        if (abs(kb.r - 247.0) < 0.6 && abs(kb.g - 248.0) < 0.6 && kb.b > 135.0) {
            dress = 1;
        } else if (abs(kb.g - 246.0) < 0.6 && kb.r > 199.0 && kb.r < 253.0 && kb.b > 12.0) {
            dress = 2;
        } else if (abs(kb.r - 108.0) < 0.3 && abs(kb.g - 108.0) < 0.3 && abs(kb.b - 121.0) < 0.3) {
            // junction paving: asphalt with a blue of 121, told apart only for winter
            dress = 3;
        }
    }
    let snowy_view = g.look.y > 0.5 && g.look.z < 0.5;
    if (dress == 1 && !snowy_view) {
        discard;
    }
    if (dress == 3 && !snowy_view) {
        dress = 0;
    }
    let coastal_rock = !ground && near(col, 143.0, 138.0, 129.0);
    if (v.tint != 0u) {
        if (near(col, 255.0, 0.0, 255.0)) {
            col = vec3<f32>(f32((v.tint >> 16u) & 255u), f32((v.tint >> 8u) & 255u), f32(v.tint & 255u)) / 255.0;
        }
    }
    if (ground && abs(col.r * 255.0 - 116.0) < 0.6 && abs(col.g * 255.0 - 172.0) < 0.6 && abs(col.b * 255.0 - 95.0) < 0.6) {
        col = terrain_look(v.world, n, facet, vec3<f32>(116.0, 172.0, 92.0) / 255.0);
        if (g.rock[5].w > 0.5 && g.look.z < 0.5 && n.z < 0.9) {
            l = dot(facet, g.sun.xyz);
        }
    } else if (ground && g.rock[7].z > 0.5 && g.look.z < 0.5 && (n.z < 0.92 || facet.z < 0.9)) {
        // looks with CPU ground colours: only steep ground is repainted here. A wall is judged by
        // the true normal of its triangle, and lit as if upright: the coarse grid makes tilted
        // triangles at the foot of a cliff whose sun facing left tall pale spikes on the rock.
        if (facet.z < 0.9) {
            n = facet;
            if (facet.z < 0.86) {
                let h = normalize(v.normal.xy + vec2<f32>(0.0001, 0.0));
                n = normalize(vec3<f32>(h * 0.97, 0.24));
            }
            l = dot(n, g.sun.xyz);
        }
        col = terrain_look(v.world, n, facet, col);
    }
    if (coastal_rock && g.look.z < 0.5) {
        let wall_normal = normalize(vec3<f32>(n.xy, 0.28));
        col = terrain_look(v.world, wall_normal, facet, col);
    }
    var emissive = false;
    var haze_twigs = false;
    let season = i32(g.look.x + 0.5);
    let previous = previous_season(season);
    var gkind = 0;
    var wkind = 0;
    if (ground && cel && g.look.z < 0.5) {
        wkind = water_kind(col);
        if (wkind == 0) {
            gkind = ground_kind(col);
        }
    }
    var dressed = false;
    if (dress != 0) {
        gkind = 0;
        dressed = true;
        col = road_dress(dress, rc * 255.0, v.world, n, g.screen.z * v.dist, gradb);
        if (g.look.z > 0.5 && dress >= 2) {
            // map views take the plain road colour
            col = vec3<f32>(108.0, 108.0, 120.0) / 255.0;
        }
    }
    let pstep = patch_step(col);
    if (pstep >= 0) {
        let phase = season_phase(v.world);
        let litter = select(select(0.0, 1.0 - phase, season == 0), phase, season == 1);
        let tile = v.world.xy / 0.22;
        let cell = vec2<i32>(floor(tile));
        let chance = hash2(cell);
        let tilt = hash2(cell + vec2<i32>(17, -3)) - 0.5;
        let d = fract(tile) - vec2<f32>(0.5);
        let leaf = vec2<f32>(d.x + tilt * d.y, d.y - tilt * d.x);
        if (g.look.z > 0.5 || g.look.y > 0.5 || v.dist * g.screen.z > 0.45
            || chance > 0.70 * litter || abs(leaf.x) * 1.3 + abs(leaf.y) > 0.45) {
            discard;
        }
        col = autumn_leaf(pstep, pstep >= 2) * (0.70 + 0.21 * chance);
    }
    var is_cover = false;
    if (cel && !ground) {
        let gs = leaf_step(col, 100.0, 168.0, 58.0);
        let fs = leaf_step(col, 246.0, 206.0, 30.0);
        let ws = leaf_step(col, 212.0, 176.0, 60.0);
        let bs = leaf_step(col, 232.0, 188.0, 70.0);
        if (gs >= 0) {
            is_cover = true;
            col = mix(groundcover_palette(gs, previous), groundcover_palette(gs, season),
                season_phase(v.world)) / 255.0;
        }
        if (ws >= 0) {
            is_cover = true;
            col = mix(field_palette(ws, previous), field_palette(ws, season), season_phase(v.world));
        }
        if (fs >= 0) {
            var fc = vec3<f32>(250.0, 214.0, 70.0);
            if (fs == 1) { fc = vec3<f32>(250.0, 248.0, 238.0); }
            if (fs == 2) { fc = vec3<f32>(240.0, 128.0, 166.0); }
            if (fs == 3) { fc = vec3<f32>(156.0, 116.0, 208.0); }
            let dormant = vec3<f32>(153.0, 132.0, 84.0);
            let winter = mix(f32(previous == 0), f32(season == 0), season_phase(v.world));
            let autumn = mix(f32(previous == 1), f32(season == 1), season_phase(v.world));
            col = mix(fc, dormant, max(winter, autumn * 0.86)) / 255.0;
        }
        if (bs >= 0) {
            let dormant = mix(f32(previous == 2 || previous == 0),
                f32(season == 2 || season == 0), season_phase(v.world));
            col = mix(vec3<f32>(226.0, 192.0, 92.0), vec3<f32>(139.0, 135.0, 88.0), dormant) / 255.0;
        }
    }
    if (ground && near(col, 247.0, 248.0, 253.0) && (g.look.y < 0.5 || g.look.z > 0.5)) {
        discard;
    }
    var glow = vec3<f32>(0.0);
    var glow_amount = 0.0;
    if (g.look.z < 0.5) {
        // Seasonal palettes ease from the previous season.
        if (ground && is_green(col)) {
            col = mix(season_ground(col, previous), season_ground(col, season), season_phase(v.world));
        }
        if (cel) {
            let s1 = leaf_step(col, 88.0, 150.0, 76.0);
            let s2 = leaf_step(col, 70.0, 128.0, 66.0);
            if (s1 >= 0 || s2 >= 0) {
                let presence = leaf_presence(v.world);
                if (presence < 0.999) {
                    // leaves go in clumps: the crown thins into holes, then nothing
                    let clump = hash2(vec2<i32>(floor(v.world.xy * 1.7 + v.world.z * vec2<f32>(0.9, 1.3))));
                    if (clump >= presence) {
                        if (!haze) {
                            discard;
                        }
                        // far away the bare crown is a soft haze of twigs: a sparse, steady
                        // screen door of grey brown, not single pixel lines
                        let door = hash2(vec2<i32>(floor(v.clip.xy * 0.5)));
                        if (door > 0.44 * (1.0 - presence)) {
                            discard;
                        }
                        haze_twigs = true;
                    }
                }
                let step = max(s1, s2);
                let dark = s2 >= 0;
                col = mix(leaf_palette(col, step, dark, previous), leaf_palette(col, step, dark, season), season_phase(v.world));
                if (step == 3) {
                    let spring = mix(f32(previous == 2), f32(season == 2), season_phase(v.world));
                    let blossom = smoothstep(0.69, 0.82, vnoise(v.room_anchor.xy * 0.083));
                    col = mix(col, vec3<f32>(0.79, 0.62, 0.66), spring * blossom * 0.76);
                }
                let foliage = vnoise(v.world.xy * 0.95 + v.world.z * 0.63);
                col = col * (0.86 + foliage * 0.29);
                if (haze_twigs) {
                    col = vec3<f32>(0.50, 0.44, 0.40) * (0.88 + 0.24 * foliage);
                }
            }
        }
        // Buildings brighten in neighborhoods, followed by continuously lit
        // lamps and vehicles. A window or paired set of lights never samples an outline id.
        if (!cel && g.tone.w > 0.0 && (near(col, 98.0, 156.0, 204.0) || near(col, 64.0, 88.0, 118.0))) {
            glow = window_colour(room_roll(v.room_anchor));
            glow_amount = window_glow(v.room_anchor, near(col, 64.0, 88.0, 118.0), false);
        }
        if (!cel && near(col, 255.0, 236.0, 150.0)) {
            glow = vec3<f32>(1.0, 0.95, 0.76);
            glow_amount = g.look.w;
        }
        if (!cel && near(col, 222.0, 36.0, 44.0)) {
            glow = vec3<f32>(1.0, 0.18, 0.15);
            glow_amount = g.look.w;
        }
        // snow
        if (g.look.y > 0.5) {
            // patches go white as the cover deepens; full cover is the plain winter look
            let reach = g.wx.x * 1.3 - 0.15;
            if (ground) {
                if (col.g > col.b + 0.08 && col.g > col.r - 0.06) {
                    // drifts with rounded edges that join up as the cover deepens
                    let h = vnoise(v.world.xy / 16.0) * 0.7 + vnoise(v.world.xy / 5.0) * 0.3;
                    // planted medians and lot edges are covered first, not last
                    let whiteness = smoothstep(h - 0.11, h + 0.11, reach + 0.3);
                    col = mix(col, vec3<f32>(236.0, 241.0, 247.0) / 255.0, whiteness);
                }
            } else if (is_cover) {
                let h = vnoise(v.world.xy / 16.0) * 0.7 + vnoise(v.world.xy / 5.0) * 0.3;
                col = mix(col, vec3<f32>(228.0, 235.0, 244.0) / 255.0, smoothstep(h - 0.11, h + 0.11, reach));
            }
        }
        // rain: wet roads and ground are darker and a little bluer, wet roofs slightly so
        if (g.wx.y > 0.01 && v.tint == 0u) {
            let wet = g.wx.y;
            let is_white = col.r > 0.88 && col.g > 0.88;
            if (!cel && !is_white) {
                col = col * vec3<f32>(1.0 - 0.28 * wet, 1.0 - 0.25 * wet, 1.0 - 0.15 * wet);
            } else if (cel && n.z > 0.35 && !is_white) {
                col = col * (1.0 - 0.1 * wet);
            }
        }
    } else if (!keep) {
        let lum = dot(col, vec3<f32>(0.3, 0.59, 0.11));
        col = mix(vec3<f32>(lum), col, 0.12);
    }
    if (wkind > 0) {
        // At a grazing angle a pixel can span much more water than the
        // camera-relative footprint suggests. Filter both wave scales accordingly.
        let water_px = max(length(world_dx.xy), length(world_dy.xy));
        n = wave_normal(v.world.xy, water_px, wkind == 2);
        col = water_colour(wkind, v.world, water_px, n);
    } else if (gkind > 0) {
        let snowy = min(col.r, min(col.g, col.b)) > 0.88;
        col = ground_detail(col, rc, gkind, v.world.xy, v.dist, snowy);
        if (snowy) {
            col = snow_relief(col, v.world.xy, 1.0 - 0.8 * smoothstep(0.8, 3.0, g.screen.z * v.dist));
        }
        if ((gkind == 1 || gkind == 4) && !snowy) {
            col = grass_soil(col, v.world, n, g.screen.z * v.dist);
        }
    }
    var facade_glass = 0.0;
    let window_pane = !cel && !ground && v.tint == 0u
        && (near(rc, 98.0, 156.0, 204.0) || near(rc, 64.0, 88.0, 118.0));
    if (window_pane && g.look.z < 0.5 && abs(n.z) < 0.5) {
        // a window drawn as its own face: its first corner is the anchor, so the pane's own
        // width and height are not known; a typical window is assumed
        let along = dot(v.world.xy - v.room_anchor.xy, vec2<f32>(-n.y, n.x));
        let pane = vec2<f32>(fract(abs(along) / 1.4), clamp(abs(v.world.z - v.room_anchor.z) / 1.5, 0.0, 1.0));
        let seed = hash2(vec2<i32>(floor(v.room_anchor.xy * 3.1 + v.room_anchor.z * 1.7)));
        let footprint = sqrt(max(dot(world_dx, world_dx), dot(world_dy, world_dy)));
        col = glass_look(col, pane, seed, v.world, n, footprint, 0.0);
        // glass in sun is shaded with everything else below; glass turned away is dimmer
        if (l <= 0.02) {
            col *= 0.80;
        }
    }
    if (!emissive && g.look.z < 0.5 && v.tint == 0u && wkind == 0 && !dressed) {
        if (finish == 0u) {
            let plain = col;
            col = textured_surface(col, rc, v.world, n, g.screen.z * v.dist, cel, !ground && !exempt);
            if (cel && !ground) {
                // brick courses, tile seams and streaks are noise up close on a lit face and
                // shimmer far away: less contrast in the sun, none at a distance
                let calm = (1.0 - smoothstep(0.10, 0.36, g.screen.z * v.dist))
                    * (1.0 - 0.45 * smoothstep(0.2, 0.7, l));
                col = mix(plain, col, calm);
            }
        } else {
            let footprint = sqrt(max(dot(world_dx, world_dx), dot(world_dy, world_dy)));
            if ((finish >= 243u && finish <= 246u) || finish >= 250u) {
                let facade = facing_facade(col, finish, v.world, v.room_anchor, n, footprint);
                col = facade.colour;
                facade_glass = facade.glass;
                glow = window_colour(facade.roll);
                glow_amount = facade.glow;
            } else {
                col = dressed_surface(col, finish, v.world, v.room_anchor, n, footprint);
            }
        }
    }
    if (g.look.y > 0.5 && g.look.z < 0.5 && (cel || is_green(rc)) && !ground && !emissive
        && v.tint == 0u && wkind == 0 && !is_cover) {
        col = snow_on_surface(col, rc, v.world, n, exempt);
    }
    if (g.look.y > 0.5 && g.look.z < 0.5 && ground && cel && wkind == 0 && !dressed) {
        let pk = paving_kind(rc);
        if (pk != 0) {
            col = paving_snow(col, pk, rc, v.world, g.screen.z * v.dist);
        }
    }
    var k = 1.0;
    if (cel && (ground || coastal_rock) && wkind == 0) {
        // hillshade: stepped tones by how the slope faces the sun compared with level ground
        var d = l - g.sun.z;
        if (g.rock[7].z > 0.5) {
            // flat desert and sea flats must read flat: a faint swell of the land is not shaded
            d = d * (1.0 - 0.97 * smoothstep(0.9, 0.97, facet.z));
        }
        let rock_grade = smoothstep(0.68, 0.95, n.z);
        k = clamp(1.0 + d * mix(0.42, 1.25, rock_grade), 0.71, 1.12);
    } else if (cel && wkind == 0) {
        if (g.look.z > 0.5) {
            // map views keep the three flat steps
            if (n.z > 0.9) {
                k = 1.0;
            } else if (l > 0.45) {
                k = 0.93;
            } else if (l > 0.1) {
                k = 0.8;
            } else {
                k = 0.66;
            }
        } else {
            // a continuous tone by how the face meets the sun: roofs and sunlit walls bright,
            // walls turned away settle into a deep shade that the sky fill then colours
            let facing = smoothstep(-0.12, 0.62, l);
            let up = smoothstep(0.55, 0.95, n.z);
            k = mix(0.60, 0.97, facing) + 0.05 * up;
        }
    }
    if (cel && wkind == 0 && g.look.z < 0.5) {
        // moon and sky fill: faces turned from the moon keep their shape instead of going black
        let moon_fill = clamp(g.look.w * 1.25, 0.0, 1.0);
        k = mix(k, max(k, 0.90), moon_fill);
    }
    var sunlit = 1.0;
    if (l > 0.02 && k > g.shade.x && wkind == 0) {
        sunlit = lit(v.world, n);
        k = mix(g.shade.x, k, sunlit);
    }
    if (g.resolve.w > 0.5 && g.look.z < 0.5 && v.dist * g.screen.z < 0.23) {
        let footprint = v.dist * g.screen.z;
        let grain = vnoise(v.world.xy * 6.4 + v.world.z * vec2<f32>(4.9, 8.1));
        let detail = 1.0 - smoothstep(0.06, 0.23, footprint);
        col *= 1.0 + (grain - 0.5) * 0.22 * detail;
    }
    let material_colour = col;
    col = col * k;
    if (!emissive && g.look.z < 0.5 && wkind == 0) {
        // warm sun against cool sky: lit faces lean a little golden (more when the sun is
        // low), shade is filled by the blue of the sky instead of only getting darker
        let sh = clamp((0.97 - k) * 2.6, 0.0, 1.0);
        let low_sun = 1.0 - smoothstep(0.12, 0.62, g.sun.z);
        let warm = mix(vec3<f32>(1.035, 1.0, 0.945), vec3<f32>(1.10, 0.985, 0.83), low_sun);
        let cool = mix(vec3<f32>(0.84, 0.93, 1.13), vec3<f32>(0.80, 0.88, 1.20), low_sun);
        let clear = 1.0 - g.shade.w;
        let dusk = clamp(g.look.w * 1.25, 0.0, 1.0);
        let cool_night = mix(cool, vec3<f32>(0.95, 0.99, 1.04), 0.75 * dusk);
        col = col * mix(vec3<f32>(1.0), mix(warm, cool_night, sh), clear);
    }
    if (!emissive) {
        let sun_visibility = cloud_sunlight(v.world);
        // at night the whole picture is pulled towards neutral, so warm rooms and lamps carry
        // the colour instead of a blue wash over every wall
        let night_k = clamp(g.look.w * 1.25, 0.0, 1.0);
        let tone_lum = dot(g.tone.rgb, vec3<f32>(0.3, 0.59, 0.11));
        col *= mix(g.tone.rgb, vec3<f32>(tone_lum), 0.72 * night_k) * sun_visibility;
        if (ground && g.local_lights.z > 0.01 && wkind == 0) {
            // at night the ground is dark and the street lamps do the lighting
            col *= mix(1.0, 0.58, clamp(g.local_lights.z, 0.0, 1.0));
        }
        if (g.resolve.w > 0.5 && g.look.z < 0.5 && wkind == 0) {
            var sky_fill = sky_radiance(normalize(n + vec3<f32>(0.0, 0.0, 0.25)));
            sky_fill = mix(sky_fill, vec3<f32>(dot(sky_fill, vec3<f32>(0.3, 0.59, 0.11))), 0.5 * night_k);
            let ambient = 0.12 + 0.17 * max(0.0, 1.0 - l);
            col = mix(col, col * (0.80 + 0.39 * sky_fill), ambient);
        }
        if (g.lightning.z > 0.002) {
            let intensity = flash_at(v.world) * (0.35 + 0.65 * max(n.z, 0.0));
            col = sqrt(col * col + material_colour * material_colour * intensity * vec3<f32>(0.56, 0.79, 1.0));
        }
    }
    if (g.local_lights.z > 0.01 && g.look.z < 0.5 && !emissive) {
        let incoming = night_illumination(v.world, n);
        // Approximate direct light in linear space while the existing art palette
        // remains in display space. Materials keep their colour in the lit area.
        col = sqrt(col * col + material_colour * material_colour * incoming);
    }
    if (g.local_lights.z > 0.01 && g.look.z < 0.5 && !ground && n.z > 0.9 && v.world.z > 26.0
        && v.world.z - terrain_height(v.world.xy) > 34.0
        && hash2(vec2<i32>(floor(v.room_anchor.xy / 5.0))) < 0.55) {
        // red aviation beacon on the roof of a tall tower, blinking slowly
        // the lit spot grows with distance so it is always a few pixels across
        let d = length(v.world.xy - v.room_anchor.xy);
        let size = max(1.6, g.screen.z * v.dist * 5.0);
        let phase = fract(g.screen.w * 0.55);
        let on = smoothstep(0.0, 0.06, phase) * (1.0 - smoothstep(0.42, 0.52, phase));
        let beacon = (1.0 - smoothstep(0.35 * size, size, d)) * on;
        if (beacon > 0.0) {
            glow = mix(glow, vec3<f32>(1.0, 0.12, 0.08), beacon);
            glow_amount = max(glow_amount, beacon * clamp(g.local_lights.z * 1.5, 0.0, 1.0));
        }
    }
    if (!emissive && g.look.z < 0.5) {
        // by night the eye loses colour: unlit surfaces, autumn leaves most of all, go toward
        // a cool grey so lit windows and lamps carry the picture
        let night = smoothstep(0.08, 0.6, 1.0 - g.tone.r);
        let luma = dot(col, vec3<f32>(0.3, 0.59, 0.11));
        col = mix(col, vec3<f32>(luma) * vec3<f32>(0.88, 0.96, 1.12), night * 0.6);
        // roofs and other upward faces fall back at night: the streets and windows carry the light
        col *= 1.0 - 0.32 * night * smoothstep(0.6, 0.95, n.z);
    }
    col = mix(col, glow, glow_amount) + glow * glow_amount * 0.33;
    var o: FOut;
    let raw_colour = rc;
    let wet_road = g.wx.y > 0.10 && (near(raw_colour, 108.0, 108.0, 120.0) || dress >= 2);
    let glazing = glow_amount < 0.12 && n.z < 0.68 && !emissive
        && (facade_glass > 0.45 || (finish == 0u
            && raw_colour.b > raw_colour.r + 0.10 && raw_colour.g > raw_colour.r + 0.045));
    let material = select(select(select(0u, 3u, glazing), 2u, wet_road), 1u, wkind > 0);
    o.colour = vec4<f32>(col, 1.0);
    o.aux = vec4<u32>(bitcast<u32>(v.dist), (v.id & 0x00ffffffu) | packed_surface(n, material), 0u, 1u);
    return o;
}

// ---- edge pass ----

@group(1) @binding(0) var colour_tex: COLOUR_TEX_TYPE;
@group(1) @binding(1) var aux_tex: AUX_TEX_TYPE;

struct EOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> EOut {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var o: EOut;
    o.clip = vec4<f32>(p[i], 0.0, 1.0);
    return o;
}

@group(0) @binding(3) var terrain_tex: texture_2d<f32>;
@group(0) @binding(4) var terrain_sampler: sampler;
@group(2) @binding(0) var fog_tex: texture_2d<f32>;
@group(2) @binding(1) var previous_frame: texture_2d<f32>;
@group(1) @binding(0) var current_frame: texture_2d<f32>;

const EXEMPT: u32 = 16777215u;

fn aux_at(p: vec2<i32>, s: i32) -> vec2<f32> {
    let size = vec2<i32>(i32(g.screen.x), i32(g.screen.y));
    let q = clamp(p, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
    let t = textureLoad(aux_tex, q, s);
    return vec2<f32>(bitcast<f32>(t.x), f32(t.y & 0x00ffffffu));
}

fn ray_for_pixel(pixel: vec2<f32>) -> vec3<f32> {
    let ndc = (pixel / g.screen.xy * 2.0 - 1.0) * vec2<f32>(1.0, -1.0);
    let q = g.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    return q.xyz - g.eye.xyz * q.w;
}

fn world_at(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    return g.eye.xyz + ray_for_pixel(vec2<f32>(pixel) + 0.5) * depth;
}

fn project_pixel(world: vec3<f32>) -> vec3<f32> {
    let clip = g.view_proj * vec4<f32>(world, 1.0);
    let uv = clip.xy / max(clip.w, 0.001) * vec2<f32>(0.5, -0.5) + 0.5;
    return vec3<f32>(uv * g.screen.xy, clip.w);
}

fn terrain_fog_profile(xy: vec2<f32>) -> vec2<f32> {
    let texel = g.terrain.w;
    let uv = clamp(xy * g.terrain.z, vec2<f32>(0.0), vec2<f32>(1.0));
    let heights = textureSampleLevel(terrain_tex, terrain_sampler,
        uv * (1.0 - texel) + 0.5 * texel, 0.0).rgb;
    // Filtering both bytes is equivalent to filtering their combined heights.
    let height = g.terrain.x + g.terrain.y * dot(heights.rg, vec2<f32>(256.0 / 257.0, 1.0 / 257.0));
    return vec2<f32>(height, heights.b);
}

fn terrain_height(xy: vec2<f32>) -> f32 {
    return terrain_fog_profile(xy).x;
}

struct Precipitation {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
    @location(2) @interpolate(flat) snow: f32,
};

@vertex
fn vs_precipitation(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Precipitation {
    let column = instance / 12u;
    let cell = vec2<i32>(g.precipitation.zw) + vec2<i32>(i32(column % 36u), i32(column / 36u));
    let slot = i32(instance % 12u);
    let seed = cell * 12 + vec2<i32>(slot, slot * 17);
    let origin = (vec2<f32>(cell) + vec2<f32>(hash2(seed), hash2(seed + vec2<i32>(41, 29)))) * 12.0;
    let is_snow = slot >= 6;
    let local_weather = mix(1.0, 0.35 + 0.95 * storm_cell(origin), g.weather_flow.z);
    let strength = min(1.0, select(g.precipitation.x, g.precipitation.y, is_snow) * local_weather);
    let chance = hash2(seed + vec2<i32>(-52, 19));
    var o: Precipitation;
    o.clip = vec4<f32>(-2.0, -2.0, 0.0, 1.0);
    o.uv = vec2<f32>(0.0);
    o.opacity = 0.0;
    o.snow = f32(is_snow);
    let visible = smoothstep(chance, chance + 0.16, strength);
    if (visible <= 0.0) {
        return o;
    }

    let variation = hash2(seed + vec2<i32>(73, -13));
    let speed = select(17.0 + 13.0 * variation, 2.4 + 2.2 * variation, is_snow);
    let height = 54.0;
    let age = fract(g.screen.w * speed / height + hash2(seed + vec2<i32>(-17, 89)));
    let duration = age * height / speed;
    let gust = 1.0 + g.weather_flow.z * (storm_cell(origin + vec2<f32>(65.0, 18.0)) - 0.43) * 0.8;
    let wind = clamp(g.wx.zw, vec2<f32>(-18.0), vec2<f32>(18.0)) * select(0.62, 0.37, is_snow) * gust;
    let sway = vec2<f32>(sin(duration * 2.4 + variation * 14.0), cos(duration * 1.6)) * 0.75;
    let xy = origin + wind * duration + select(vec2<f32>(0.0), sway, is_snow);
    let ground = terrain_height(origin);
    let centre = vec3<f32>(xy, ground + (1.0 - age) * height);
    let q = g.view_proj * vec4<f32>(centre, 1.0);
    if (q.w <= g.eye.w) {
        return o;
    }

    let corner = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0)
    )[vertex];
    let metres_per_pixel = g.screen.z * q.w;
    let life = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.83, 1.0, age));
    let fade = 1.0 - smoothstep(0.45, 1.65, metres_per_pixel);
    o.clip = q;
    if (is_snow) {
        let radius = clamp((0.10 + 0.14 * variation) / metres_per_pixel, 0.48, 2.0);
        o.clip.x += corner.x * radius * q.w * 2.0 / g.screen.x;
        o.clip.y += corner.y * radius * q.w * 2.0 / g.screen.y;
        o.opacity = 0.75 * life * fade * visible;
    } else {
        let tail = centre + vec3<f32>(-wind / speed, 1.0) * (0.46 + variation * 0.54);
        let end = g.view_proj * vec4<f32>(tail, 1.0);
        let axis = (end.xy / end.w - q.xy / q.w) * g.screen.xy;
        let perp = normalize(vec2<f32>(axis.y, -axis.x) + vec2<f32>(0.0001, 0.0));
        o.clip = mix(q, end, corner.y * 0.5 + 0.5);
        let spread = clamp(0.045 / metres_per_pixel, 0.36, 1.1);
        let offset = perp * corner.x * spread * 2.0 * o.clip.w / g.screen.xy;
        o.clip.x += offset.x;
        o.clip.y += offset.y;
        o.opacity = 0.56 * life * fade * visible;
    }
    o.uv = corner;
    return o;
}

@fragment
fn fs_precipitation(p: Precipitation) -> FOut {
    let flake = 1.0 - smoothstep(0.3, 1.0, length(p.uv));
    let streak = (1.0 - smoothstep(0.06, 1.0, abs(p.uv.x))) *
        (1.0 - smoothstep(0.05, 1.0, p.uv.y));
    let colour = mix(vec3<f32>(0.65, 0.75, 0.86), vec3<f32>(0.88, 0.92, 0.98), p.snow);
    var out: FOut;
    out.colour = vec4<f32>(colour * mix(g.tone.rgb, vec3<f32>(1.0), 0.43),
        p.opacity * mix(streak, flake, p.snow));
    out.aux = vec4<u32>(0u);
    return out;
}

// Trace from the camera toward terrain. Midpoints are stable in world space as
// the camera moves, and each sample contributes only the light still visible.
fn low_fog(eye: vec3<f32>, dir: vec3<f32>, limit: f32, coarse: bool) -> vec2<f32> {
    let depth = min(limit, mix(104.0, 180.0, g.fog.w));
    let ground = eye + dir * limit;
    let drift = vec2<f32>(1.18, 0.49) * g.screen.w + g.weather_flow.xy * 0.54;
    let wind = clamp(g.wx.zw, vec2<f32>(-9.0), vec2<f32>(9.0));
    let count = select(select(8, 15, g.resolve.w > 0.5), 5, coarse);
    var start = depth;
    var remaining = 1.0;
    var centroid = vec3<f32>(0.0);
    var shafts = 0.0;
    var checked = 0.0;
    let daylight = smoothstep(0.015, 0.29, g.sun.z);
    for (var i = 0; i < count && remaining > 0.005; i++) {
        let u = 1.0 - f32(i + 1) / f32(count);
        let end = depth * (0.10 * u + 0.90 * u * u);
        let point = ground - dir * ((start + end) * 0.5);
        let ground_profile = terrain_fog_profile(point.xy);
        let above = point.z - ground_profile.x;
        if (above >= 0.0 && above < 50.0) {
            let pool = smoothstep(0.0, 1.0, ground_profile.y);
            let flow = point.xy - drift - wind * (2.0 + 0.19 * above);
            let front = vnoise(flow * 0.0105);
            let curl_pos = vec3<f32>(flow + above * vec2<f32>(0.36, -0.28),
                above * 1.45 - g.screen.w * 0.47);
            let curls = textureSampleLevel(cloud_tex, cloud_sampler, curl_pos / 161.0, 0.0).r;
            let billow = smoothstep(0.29, 0.69, front * 0.65 + curls * 0.35);
            let ceiling = 4.0 + (7.0 + 31.0 * pool) * billow * g.fog.w;
            // the top of the mist thins out gradually, so it never cuts a building in a flat sheet
            let rise = clamp(above / (ceiling * 1.9), 0.0, 1.0);
            let layer = (1.0 - rise) * (1.0 - rise);
            // and it thins as the camera gets close (never to nothing: street level keeps its mist), so
            // what the player built is always in view
            let clear = mix(0.4, 1.0, smoothstep(70.0, 460.0, distance(point, eye)));
            let density = layer * clear * billow * mix(0.48, 1.15, pool) * g.fog.w * 0.19;
            let scatter = remaining * (1.0 - exp(-density * (start - end)));
            remaining -= scatter;
            centroid += point * scatter;
            if (!coarse && g.resolve.w > 0.5 && daylight > 0.01 && (i % 5) == 0 && scatter > 0.003) {
                let normal = vec3<f32>(0.0, 0.0, 1.0);
                shafts += scatter * lit(point, normal) * cloud_sunlight(point);
                checked += scatter;
            }
        }
        start = end;
    }
    let opacity = 1.0 - remaining;
    if (opacity < 0.001) {
        return vec2<f32>(0.0);
    }
    let point = centroid / opacity;
    var direct = 0.0;
    if (daylight > 0.001) {
        let forward = pow(max(0.0, dot(dir, g.sun.xyz)), 5.0);
        let centre = lit(point, vec3<f32>(0.0, 0.0, 1.0)) * cloud_sunlight(point);
        let sunlight = mix(centre, shafts / max(0.001, checked), smoothstep(0.015, 0.08, checked) * 0.85);
        direct = sunlight * daylight * (0.39 + 0.61 * forward);
    }
    if (g.local_lights.z > 0.02) {
        let lamps = night_illumination(point, vec3<f32>(0.0, 0.0, 1.0));
        direct += min(0.74, dot(lamps, vec3<f32>(0.29, 0.57, 0.14))) * (1.0 - daylight);
    }
    return vec2<f32>(opacity, opacity * direct);
}

fn cloud_volume(eye: vec3<f32>, dir: vec3<f32>, limit: f32, pixel: vec2<f32>, coarse: bool) -> vec2<f32> {
    let base = cloud_base();
    let thickness = cloud_thickness();
    let top = base + thickness;
    var start = 0.0;
    var end = min(limit, 2550.0);
    if (abs(dir.z) > 0.001) {
        let a = (base - eye.z) / dir.z;
        let b = (top - eye.z) / dir.z;
        start = max(0.0, min(a, b));
        end = min(end, max(a, b));
    } else if (eye.z < base || eye.z > top) {
        return vec2<f32>(0.0);
    }
    let span = end - start;
    if (span <= 0.0) {
        return vec2<f32>(0.0);
    }
    let detailed = g.resolve.w > 0.5;
    let nominal = select(26, 38, detailed);
    let grazing = min(select(46, 56, detailed), i32(ceil(span / 17.0)));
    let budget = select(max(nominal, grazing), 12, coarse);
    let step_size = span / f32(budget);
    let detail = select(1.0 - smoothstep(12.0, 30.0, step_size), 0.0, coarse);
    // Stagger samples with a small per-pixel phase. Limiting the offset
    // keeps the longer intervals from turning smooth shade into grain.
    let phase = hash2(vec2<i32>(floor(pixel / g.resolve.z)));
    let jitter_range = mix(0.29, 0.17, smoothstep(10.0, 22.0, step_size));
    var previous_distance = start;
    var previous = 0.0;
    var transmission = 1.0;
    var sunlight = 0.0;
    let day = smoothstep(0.03, 0.37, g.sun.z);
    let forward = pow(max(0.0, dot(dir, g.sun.xyz)), 4.0);
    for (var i = 0; i <= budget; i++) {
        let jitter = fract(phase + f32(i) * 0.618034);
        let distance = select(start + step_size * (f32(i) + 0.5 + (jitter - 0.5) * jitter_range), end, i == budget);
        let interval = distance - previous_distance;
        let point = eye + dir * distance;
        let density = cloud_density(point, detail);
        let smooth_density = (previous + density) * 0.5;
        let light_point = point - dir * interval * (previous / max(0.001, previous + density));
        previous = density;
        previous_distance = distance;
        if (smooth_density < 0.003) {
            continue;
        }
        let optical_depth = smooth_density * interval * 0.050 * smoothstep(12.0, 95.0, distance);
        let scatter = transmission * (1.0 - exp(-optical_depth));
        transmission -= scatter;
        if (g.lightning.z > 0.002) {
            sunlight += scatter * flash_at(light_point) * 1.9;
        }
        if (day > 0.0) {
            // One sample toward the sun combines with the local density to
            // estimate extinction. The broad fill can reach a shadowed face.
            let blocker = cloud_density(light_point + g.sun.xyz * 21.0, 0.0);
            let direct = exp(-(blocker * 3.55 + smooth_density * 0.49));
            let bounced = 0.13 + 0.19 * exp(-blocker * 0.84);
            sunlight += scatter * (direct * (0.87 + forward * 0.33) + bounced) * day;
        }
        if (transmission < 0.004) {
            break;
        }
    }
    return vec2<f32>(1.0 - transmission, sunlight) * smoothstep(0.12, 0.36, g.shade.w);
}

fn atmosphere(pixel: vec2<f32>, depth: f32, coarse: bool) -> vec3<f32> {
    let ray = ray_for_pixel(pixel);
    let ray_length = length(ray);
    let dir = ray / ray_length;
    let surface = depth <= 100000.0 && depth > g.eye.w;
    let limit = select(4000.0, depth * ray_length, surface);
    var fog = vec2<f32>(0.0);
    if (g.fog.w > 0.005 && surface) {
        fog = low_fog(g.eye.xyz, dir, limit, coarse);
    }
    var cloud = vec2<f32>(0.0);
    // For ground-facing rays, zoom through the deck while leaving low fog in place.
    // Sky rays keep overhead clouds visible when the camera looks towards the horizon.
    let cloud_view = select(1.0, smoothstep(9.0, 34.0, g.eye.w), surface);
    if (g.shade.w > 0.12 && cloud_view > 0.0) {
        cloud = cloud_volume(g.eye.xyz, dir, limit, pixel, coarse);
        if (surface && g.weather_flow.z > 0.02) {
            // Preserve contrast across the playable map below heavy clouds.
            let distant = smoothstep(1350.0, 2600.0, limit);
            let thick = smoothstep(0.43, 0.91, g.shade.w);
            cloud *= 1.0 - thick * (0.18 + 0.33 * g.weather_flow.z) * (1.0 - distant);
        }
        cloud *= cloud_view;
        if (surface) {
            // cloud over the ground right below the camera thins away, so the town is never hidden
            cloud *= mix(0.2, 1.0, smoothstep(260.0, 1150.0, limit));
        }
    }
    return vec3<f32>(fog.x, fog.y * (1.0 - cloud.x) * 0.68 + cloud.y * 1.37, cloud.x);
}

@fragment
fn fs_fog(e: EOut) -> @location(0) vec4<f32> {
    let pixel = min(e.clip.xy * g.resolve.z, g.screen.xy - 0.5);
    // Match the ray to the middle of its footprint at standard resolution.
    let depth = aux_at(vec2<i32>(pixel), 0).x;
    let volume = atmosphere(pixel, depth, false);
    return vec4<f32>(volume, min(depth, 65504.0));
}

fn atmosphere_at(pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = pixel / g.resolve.z - 0.5;
    let base = vec2<i32>(floor(uv));
    let f = fract(uv);
    let bound = vec2<i32>(textureDimensions(fog_tex)) - vec2<i32>(1);
    let comparable_depth = min(depth, 65504.0);
    let tol = 1.5 + comparable_depth * 0.012;
    var result = vec3<f32>(0.0);
    var weights = 0.0;
    for (var y = 0; y <= 1; y++) {
        for (var x = 0; x <= 1; x++) {
            let offset = vec2<i32>(x, y);
            let tap = textureLoad(fog_tex, clamp(base + offset, vec2<i32>(0), bound), 0);
            let spatial = 1.0 - abs(f - vec2<f32>(offset));
            let weight = spatial.x * spatial.y
                * (1.0 - smoothstep(tol, 2.0 * tol, abs(tap.a - comparable_depth)));
            result += tap.rgb * weight;
            weights += weight;
        }
    }
    if (weights < 0.0001) {
        // Geometry edges without a matching low-resolution depth still need
        // their own occlusion, but should never run another full-quality march.
        return atmosphere(pixel, depth, true);
    }
    return result / weights;
}

// True when this pixel is on the near side of a face change: one pixel wide ink line.
fn ink_at(p: vec2<i32>, s: i32) -> bool {
    let a = aux_at(p, s);
    let ida = u32(a.y);
    if (ida == EXEMPT) {
        return false;
    }
    let tol = 0.002 * a.x + 0.05;
    var offs = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    for (var i = 0; i < 4; i = i + 1) {
        let b = aux_at(p + offs[i], s);
        let idb = u32(b.y);
        if (idb == ida || idb == EXEMPT) {
            continue;
        }
        if (a.x < b.x - tol) {
            return true;
        }
        if (abs(a.x - b.x) <= tol && ida > idb) {
            return true;
        }
    }
    return false;
}

// Contact shade: ground within about a metre of something standing on it (a wall, a trunk, a
// fence) gets one darker, cooler step. Looks at four neighbours of the first sample only.
fn contact(p: vec2<i32>, px: f32, a: vec2<f32>) -> f32 {
    if (u32(a.y) != 0u) {
        return 1.0;
    }
    let r = i32(clamp(0.9 / max(px, 0.02), 1.0, 4.0));
    var offs = array<vec2<i32>, 4>(vec2<i32>(r, 0), vec2<i32>(-r, 0), vec2<i32>(0, r), vec2<i32>(0, -r));
    let tol = 0.004 * a.x + 0.3;
    for (var i = 0; i < 4; i = i + 1) {
        let b = aux_at(p + offs[i], 0);
        let idb = u32(b.y);
        // a real object (not ground, not flat paint) that is close to the ground point in depth
        if (idb != 0u && idb != EXEMPT && abs(b.x - a.x) < 2.5 + tol) {
            return 0.80;
        }
    }
    return 1.0;
}

fn lightning_segment(pixel: vec2<f32>, depth: f32, a: vec3<f32>, b: vec3<f32>, width: f32) -> f32 {
    if (min(a.z, b.z) <= g.eye.w) {
        return 0.0;
    }
    let edge = b.xy - a.xy;
    let t = clamp(dot(pixel - a.xy, edge) / max(0.03, dot(edge, edge)), 0.0, 1.0);
    let z = 1.0 / mix(1.0 / a.z, 1.0 / b.z, t);
    let distance = length(pixel - mix(a.xy, b.xy, t));
    let visible = smoothstep(-0.2, 3.0, depth - z);
    return (1.0 - smoothstep(width * 0.28, width * 1.37, distance)) * visible;
}

fn lightning_bolt(p: vec2<i32>, depth: f32) -> f32 {
    let xy = g.lightning.xy;
    let seed = vec2<i32>(floor(xy * 0.16));
    let ground = terrain_height(xy);
    let top = max(ground + 85.0, cloud_base() + cloud_thickness() * 0.37);
    let bottom = project_pixel(vec3<f32>(xy, ground + 0.3));
    let head = project_pixel(vec3<f32>(xy, top));
    let pixel = vec2<f32>(p) + 0.5;
    if (any(pixel < min(head.xy, bottom.xy) - 0.22 * g.screen.y)
        || any(pixel > max(head.xy, bottom.xy) + 0.22 * g.screen.y)) {
        return 0.0;
    }
    let width = max(1.0, 0.0021 * g.screen.y);
    var brightness = 0.0;
    var start = bottom;
    for (var i = 1; i <= 8; i++) {
        let t = f32(i) * 0.125;
        let jitter = vec2<f32>(hash2(seed + vec2<i32>(i, 16)), hash2(seed + vec2<i32>(9, i))) - 0.5;
        let point = vec3<f32>(xy + jitter * 31.0 * sin(t * 3.14159), mix(ground, top, t));
        let end = project_pixel(point);
        brightness = max(brightness, lightning_segment(pixel, depth, start, end, width));
        if ((i == 4) || (i == 6)) {
            let direction = select(-1.0, 1.0, i == 6);
            let elbow = project_pixel(point + vec3<f32>(direction * 15.0, 8.0, -11.0));
            let fork = project_pixel(point + vec3<f32>(direction * 27.0, 13.0, -29.0));
            brightness = max(brightness, 0.60 * lightning_segment(pixel, depth, end, elbow, width * 0.66));
            brightness = max(brightness, 0.48 * lightning_segment(pixel, depth, elbow, fork, width * 0.55));
        }
        start = end;
    }
    return brightness * g.lightning.z;
}

fn near_geometry_light(p: vec2<i32>, centre: vec2<f32>) -> f32 {
    let footprint = centre.x * g.screen.z;
    let contact = 1.0 - smoothstep(1.6, 4.2, footprint);
    if (contact <= 0.0 || centre.x > 100000.0) {
        return 1.0;
    }
    let point = world_at(p, centre.x);
    let value = textureLoad(aux_tex, p, 0).y;
    let normal = surface_normal(value);
    // two rings: about a metre and a half for creases, and about four metres for the soft
    // shade that gathers at the foot of a wall and between close buildings
    let radius = clamp(1.5 / max(footprint, 0.03), 1.5, 14.0);
    let bounds = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    let directions = array<vec2<f32>, 8>(
        vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
        vec2<f32>(0.71, 0.71), vec2<f32>(-0.71, 0.71), vec2<f32>(0.71, -0.71), vec2<f32>(-0.71, -0.71));
    var blocked = 0.0;
    for (var i = 0; i < 8; i++) {
        let reach = select(1.0, 2.7, i >= 4);
        let q = clamp(p + vec2<i32>(directions[i] * radius * reach), vec2<i32>(0), bounds);
        let sample = aux_at(q, 0);
        if (sample.x > 100000.0 || abs(sample.x - centre.x) > 16.0) {
            continue;
        }
        let offset = world_at(q, sample.x) - point;
        let distance = length(offset);
        let elevation = dot(normal, offset / max(0.01, distance));
        blocked += smoothstep(0.10, 0.70, elevation) * (1.0 - smoothstep(2.0, 11.0, distance));
    }
    var shade = 0.0;
    if (g.resolve.w > 0.5 && g.sun.z > 0.12 && dot(normal, g.sun.xyz) > 0.13) {
        for (var i = 1; i <= 4; i++) {
            let ray = point + normal * 0.25 + g.sun.xyz * (0.62 * f32(i));
            let q = project_pixel(ray);
            if (any(q.xy < vec2<f32>(0.0)) || any(q.xy > vec2<f32>(bounds))) {
                break;
            }
            let hit = aux_at(vec2<i32>(q.xy), 0);
            let gap = q.z - hit.x;
            if (hit.y != centre.y && gap > 0.12 && gap < 1.2) {
                shade = 0.14 * smoothstep(0.12, 0.3, g.sun.z);
                break;
            }
        }
    }
    return 1.0 - (min(0.58, blocked * 0.125) + shade) * contact;
}

// A wide band of occlusion, 10 to 25 metres out: towers darken the streets and courtyards
// between them, and the lower walls, even from the wide town view. Looks for ground and walls
// standing well above this point in twelve directions at two reaches. Returns x the open share
// (1 open) and y a lee share for snow (1 in the shelter of something tall, downwind).
// Not run on the light preset, and the smoke picture switch `seasons.y` turns it off.
fn wide_occlusion(p: vec2<i32>, centre: vec2<f32>) -> vec2<f32> {
    if (g.seasons.y < 0.5 || g.resolve.w < 0.5 || centre.x > 100000.0) {
        return vec2<f32>(1.0, 0.0);
    }
    let footprint = max(centre.x * g.screen.z, 0.02);
    let value = textureLoad(aux_tex, p, 0).y;
    let normal = surface_normal(value);
    let point = world_at(p, centre.x);
    let bounds = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    var blocked = 0.0;
    var counted = 0.0;
    for (var ring = 0; ring < 2; ring++) {
        let metres = select(11.0, 23.0, ring == 1);
        let reach = metres / footprint;
        if (reach < 5.0 || reach > 360.0) {
            continue;
        }
        for (var i = 0; i < 6; i++) {
            let a = (f32(i) + 0.5 * f32(ring)) * 1.0472 + 0.3;
            let q = clamp(p + vec2<i32>(vec2<f32>(cos(a), -sin(a)) * reach), vec2<i32>(0), bounds);
            let sample = aux_at(q, 0);
            counted += 1.0;
            if (sample.x > 100000.0) {
                continue;
            }
            let w = world_at(q, sample.x);
            let dz = w.z - point.z;
            let run = max(length(w.xy - point.xy), 2.0);
            blocked += smoothstep(0.30, 1.5, dz / run) * smoothstep(3.0, 9.0, dz);
        }
    }
    if (counted < 1.0) {
        return vec2<f32>(1.0, 0.0);
    }
    let strength = 0.5 + 0.5 * max(normal.z, 0.0);
    let open = 1.0 - min(0.50, 1.6 * blocked / counted) * strength;
    // the lee: something tall a few metres upwind
    var lee = 0.0;
    let wind = g.wx.zw;
    let wl = length(wind);
    let upwind = select(vec2<f32>(-0.8, -0.6), -wind / max(wl, 0.01), wl > 0.5);
    for (var k = 1; k <= 3; k++) {
        let metres = 2.5 * f32(k) + 1.0;
        let dq = vec2<f32>(upwind.x, -upwind.y) * metres / footprint;
        let q = clamp(p + vec2<i32>(dq), vec2<i32>(0), bounds);
        let sample = aux_at(q, 0);
        if (sample.x > 100000.0) {
            continue;
        }
        let w = world_at(q, sample.x);
        lee = max(lee, smoothstep(1.2, 3.5, w.z - point.z) * (1.0 - smoothstep(7.0, 17.0, length(w.xy - point.xy))));
    }
    return vec2<f32>(open, lee);
}

// Light thrown onto the ground by lit windows and shopfronts beside it, taken from the lit
// pixels around a ground pixel at night.
fn window_spill(p: vec2<i32>, centre: vec2<f32>) -> vec3<f32> {
    let footprint = max(centre.x * g.screen.z, 0.03);
    if (footprint > 1.5 || centre.x > 100000.0) {
        return vec3<f32>(0.0);
    }
    let bounds = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    var spill = vec3<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 0.7854 + 0.2;
        let metres = select(3.0, 7.5, (i & 1) == 1);
        let q = clamp(p + vec2<i32>(vec2<f32>(cos(a), -sin(a)) * metres / footprint), vec2<i32>(0), bounds);
        let sample = aux_at(q, 0);
        if (sample.y < 0.5 || abs(sample.x - centre.x) > 14.0) {
            continue;
        }
        let c = textureLoad(colour_tex, q, 0).rgb;
        let peak = max(c.r, max(c.g, c.b));
        let warm = smoothstep(0.0, 0.25, c.r - c.b);
        spill += vec3<f32>(1.0, 0.76, 0.46) * smoothstep(0.55, 0.95, peak) * warm
            * (1.0 - 0.35 * f32(i & 1));
    }
    return spill * 0.11;
}

fn temporal_colour(colour: vec3<f32>, p: vec2<i32>, depth: f32, volume: f32) -> vec3<f32> {
    if (g.temporal.x < 0.01 || depth > 65000.0 || volume > 0.7) {
        return colour;
    }
    let world = world_at(p, depth);
    let q = g.previous_view_proj * vec4<f32>(world, 1.0);
    let uv = q.xy / max(0.001, q.w) * vec2<f32>(0.5, -0.5) + 0.5;
    if (q.w < g.eye.w || any(uv < vec2<f32>(0.002)) || any(uv > vec2<f32>(0.998))) {
        return colour;
    }
    let previous = textureLoad(previous_frame, vec2<i32>(uv * g.screen.xy), 0);
    if (abs(previous.a - q.w) > max(0.55, q.w * 0.008)) {
        return colour;
    }
    let limit = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    var low = colour;
    var high = colour;
    let offsets = array<vec2<i32>, 4>(vec2<i32>(-1, 0), vec2<i32>(1, 0), vec2<i32>(0, -1), vec2<i32>(0, 1));
    for (var i = 0; i < 4; i++) {
        let sample = textureLoad(colour_tex, clamp(p + offsets[i], vec2<i32>(0), limit), 0).rgb;
        low = min(low, sample);
        high = max(high, sample);
    }
    let clipped = clamp(previous.rgb, low - 0.020, high + 0.020);
    let change = length(clipped - colour);
    let weight = g.temporal.x * (1.0 - smoothstep(0.13, 0.55, change)) * (1.0 - smoothstep(0.03, 0.7, volume));
    return mix(colour, clipped, weight);
}

fn screen_reflection(colour: vec3<f32>, p: vec2<i32>, depth: f32) -> vec3<f32> {
    let value = textureLoad(aux_tex, p, 0).y;
    let material = value >> 30u;
    if (material == 0u || depth > 1700.0) {
        return colour;
    }
    let point = world_at(p, depth);
    let normal = select(surface_normal(value), vec3<f32>(0.0, 0.0, 1.0), material == 2u);
    let view = normalize(g.eye.xyz - point);
    let reflected = reflect(-view, normal);
    let surface_id = value & 0x00ffffffu;
    let shape = 0.16 + 0.84 * pow(1.0 - max(dot(view, normal), 0.0), 3.0);
    let strength = select(0.28, 0.22 * g.wx.y, material == 2u) * shape;
    let probe = nearby_radiance(point, reflected);
    let fallback = mix(colour, probe, strength * select(0.34, 0.77, material == 3u));
    var previous = -1.0;
    for (var i = 0; i < 16; i++) {
        let t = f32(i + 1);
        let q = project_pixel(point + normal * 0.42 + reflected * (t * 1.4 + t * t * 0.46));
        if (q.z <= g.eye.w || any(q.xy < vec2<f32>(1.0)) || any(q.xy >= g.screen.xy - 1.0)) {
            break;
        }
        let pos = vec2<i32>(q.xy);
        let depth_id = aux_at(pos, 0);
        let gap = q.z - depth_id.x;
        let thickness = 0.8 + t * 0.55;
        if (depth_id.x < 100000.0 && gap > 0.0 && gap < thickness && previous <= 0.0
            && u32(depth_id.y) != surface_id) {
            let uv = q.xy / g.screen.xy;
            let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
            let weight = strength * smoothstep(0.0, 0.13, edge) * (1.0 - t / 18.0);
            return mix(fallback, textureLoad(colour_tex, pos, 0).rgb, weight);
        }
        previous = gap;
    }
    return fallback;
}

fn aerial_perspective(colour: vec3<f32>, p: vec2<i32>, depth: f32) -> vec3<f32> {
    if (depth > 100000.0) {
        return colour;
    }
    let ray = normalize(ray_for_pixel(vec2<f32>(p) + 0.5));
    var drift = 0.0;
    if (g.weather_flow.z > 0.01) {
        drift = g.weather_flow.z * storm_cell(world_at(p, depth).xy);
    }
    let extinction = 0.00014 + 0.00027 * g.shade.w + 0.00039 * drift;
    let opacity = (1.0 - exp(-max(0.0, depth - 210.0) * extinction)) * 0.68;
    let sky = sky_radiance(vec3<f32>(ray.xy, abs(ray.z) * 0.3));
    return mix(colour, sky * mix(g.tone.rgb, vec3<f32>(1.0), 0.27), opacity);
}

// Whether the sun reaches a point in the air: the shadow map without any surface offset.
fn sun_reaches(world: vec3<f32>) -> f32 {
    let c = g.light_vp * vec4<f32>(world, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || c.z > 1.0) {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_tex, shadow_cmp, uv, c.z - g.shade.y);
}

// Sun shafts: the air between the eye and the surface is lit where the sun reaches it and
// dark behind buildings, trees, hills and clouds. Sunlit air glows a little and shaded air
// dims a little, so beams show without a veil over the whole picture. Strongest with a low
// sun and damp air.
fn sun_shafts(colour: vec3<f32>, p: vec2<i32>, depth: f32) -> vec3<f32> {
    let low = 1.0 - smoothstep(0.16, 0.66, g.sun.z);
    // by day only (at night the light vector is the moon's), and only while the shadow pass
    // is drawing: under heavy cloud it stops and the map would be stale
    let daylight = 1.0 - smoothstep(0.02, 0.22, g.tone.w);
    let strength = (0.16 + 0.84 * low) * smoothstep(0.03, 0.12, g.sun.z)
        * (1.0 - 0.75 * g.shade.w) * (1.0 + 1.6 * g.fog.w) * daylight;
    if (strength < 0.02 || g.shade.x >= 0.985) {
        return colour;
    }
    let ray = ray_for_pixel(vec2<f32>(p) + 0.5);
    let reach = min(depth, 2400.0);
    // interleaved gradient noise spreads the steps so eight of them read as a smooth beam
    let dither = fract(52.9829189 * fract(dot(vec2<f32>(p), vec2<f32>(0.06711056, 0.00583715))));
    var lit_air = 0.0;
    var weight = 0.0;
    for (var i = 0; i < 8; i++) {
        let t = (f32(i) + dither) / 8.0;
        let at = g.eye.xyz + ray * (reach * t);
        // the air is thicker near the ground
        let thick = exp(-max(0.0, at.z - terrain_height(at.xy)) / 48.0);
        lit_air += sun_reaches(at) * cloud_sunlight(at) * thick;
        weight += thick;
    }
    let air = weight / 8.0;
    let sunlit = lit_air / max(weight, 0.001);
    let toward = max(0.0, dot(normalize(ray), g.sun.xyz));
    let amount = (1.0 - exp(-reach * 0.0042 * air)) * strength * (0.40 + 1.2 * toward * toward);
    let beam = mix(vec3<f32>(1.0, 0.93, 0.80), vec3<f32>(1.0, 0.74, 0.46), low);
    let dimmed = colour * (1.0 - 0.62 * amount * (1.0 - sunlit));
    // open, clear air glows only faintly: the light gathers where there is mist or where
    // something stands in the beam, so ordinary play is not veiled
    let presence = 0.22 + 0.78 * min(1.0, g.fog.w * 8.0 + (1.0 - sunlit) * 3.0);
    return dimmed + beam * amount * sunlit * 0.42 * presence;
}

// The sky where no land is drawn: a gradient to a pale horizon, the sun and its glow.
fn sky_dome(p: vec2<i32>) -> vec3<f32> {
    let ray = normalize(ray_for_pixel(vec2<f32>(p) + 0.5));
    let up = vec3<f32>(ray.xy, max(ray.z, 0.0));
    var sky = sky_radiance(normalize(up + vec3<f32>(0.0, 0.0, 0.02)));
    let day = smoothstep(-0.08, 0.3, g.sun.z) * (1.0 - smoothstep(0.05, 0.45, g.tone.w));
    let low = (1.0 - smoothstep(0.05, 0.5, g.sun.z)) * day;
    let haze = mix(vec3<f32>(0.16, 0.20, 0.30), vec3<f32>(0.80, 0.85, 0.88), day)
        + vec3<f32>(0.20, 0.06, -0.06) * low;
    sky = mix(sky, haze, pow(1.0 - clamp(ray.z, 0.0, 1.0), 6.0) * 0.85);
    let toward = max(0.0, dot(ray, g.sun.xyz));
    let clear = 1.0 - 0.8 * g.shade.w;
    sky += mix(vec3<f32>(1.0, 0.95, 0.85), vec3<f32>(1.0, 0.62, 0.30), low)
        * (pow(toward, 14.0) * 0.14 + pow(toward, 180.0) * 0.35 + smoothstep(0.9993, 0.9997, toward) * 2.0)
        * day * clear;
    return sky * mix(g.tone.rgb, vec3<f32>(1.0), 0.4);
}

fn tilt_focus(colour: vec3<f32>, p: vec2<i32>) -> vec3<f32> {
    let y = (f32(p.y) + 0.5) / g.screen.y;
    let blur = smoothstep(0.24, 0.49, abs(y - 0.48));
    if (blur < 0.02) {
        return colour;
    }
    let size = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    let radius = i32(round(clamp(g.screen.y * 0.004, 1.0, 5.0) * blur));
    let step = max(radius, 1);
    var blur_colour = colour * 0.28;
    let offsets = array<vec2<i32>, 6>(vec2<i32>(-2, 0), vec2<i32>(2, 0),
        vec2<i32>(-1, -1), vec2<i32>(1, -1), vec2<i32>(-1, 1), vec2<i32>(1, 1));
    for (var i = 0; i < 6; i++) {
        let q = clamp(p + offsets[i] * step, vec2<i32>(0), size);
        blur_colour += textureLoad(colour_tex, q, 0).rgb * 0.12;
    }
    return mix(colour, blur_colour, blur * 0.74);
}

@fragment
fn fs_edge(e: EOut) -> @location(0) vec4<f32> {
    let pixel = e.clip.xy;
    let p = min(vec2<i32>(pixel), vec2<i32>(g.screen.xy) - vec2<i32>(1));
    var acc = vec3<f32>(0.0);
    // ink thins out as a pixel covers more of the world: a one pixel line from about half a
    // metre a pixel, none from a metre and a half (whole-town views keep their roof colours)
    let centre = aux_at(p, 0);
    let px = g.screen.z * centre.x;
    let inked = px < 1.5;
    var ao = 1.0;
    if (inked) {
        ao = mix(1.0, contact(p, px, centre), g.seasons.z);
    }
    for (var s = 0; s < SAMPLE_COUNT; s = s + 1) {
        var ink = false;
        if (inked) {
            ink = ink_at(p, s);
            if (!ink && g.ink.a > 1.5 && px < 0.5) {
                ink = ink_at(p + vec2<i32>(-1, 0), s) || ink_at(p + vec2<i32>(0, -1), s);
            }
        }
        if (ink) {
            // a soft line is the face's own colour gone darker and a little cooler, which
            // keeps edges crisp without a black cartoon outline. The line stays one pixel wide
            // and fades as a pixel covers more world, so far buildings do not turn into ink.
            let face = textureLoad(colour_tex, p, s).rgb;
            let flat_ink = mix(g.ink.rgb, face, 0.28);
            let own = face * vec3<f32>(0.52, 0.55, 0.62);
            let strength = 1.0 - 0.8 * smoothstep(0.35, 1.45, px);
            acc = acc + mix(face, mix(flat_ink, own, g.lightning.w), strength);
        } else {
            acc = acc + textureLoad(colour_tex, p, s).rgb * ao;
        }
    }
    var col = acc / f32(SAMPLE_COUNT);
    if (g.look.z < 0.5 && (g.resolve.w > 0.5 || g.local_lights.w > 0.5)) {
        // occluded light is lost from the warm end first, so creases lean cool
        let open_sky = mix(1.0, near_geometry_light(p, centre), g.seasons.z);
        col *= mix(vec3<f32>(0.90, 0.96, 1.06) * open_sky, vec3<f32>(1.0), open_sky * open_sky);
    }
    if (g.look.z < 0.5 && g.resolve.w > 0.5) {
        let wide = wide_occlusion(p, centre);
        col *= wide.x * mix(vec3<f32>(0.92, 0.96, 1.06), vec3<f32>(1.0), wide.x);
        // open snow in the lee of a building or tree lies in a cool, softly rippled drift
        let snowy = min(col.r, min(col.g, col.b)) > 0.66 && g.look.y > 0.5;
        if (snowy && wide.y > 0.01 && u32(centre.y) == 0u) {
            let w = world_at(p, centre.x);
            let ripple = 0.5 + 0.5 * sin(dot(w.xy, vec2<f32>(0.7, 0.5)) * 2.4 + vnoise(w.xy * 0.4) * 5.0);
            col = mix(col, col * vec3<f32>(0.84, 0.91, 1.05), wide.y * (0.55 + 0.45 * ripple));
            col *= 1.0 + 0.05 * wide.y * smoothstep(0.55, 0.9, ripple);
        }
        if (g.local_lights.z > 0.01 && u32(centre.y) == 0u) {
            col += window_spill(p, centre) * g.local_lights.z;
        }
        col = screen_reflection(col, p, centre.x);
    }
    if (g.look.z < 0.5 && centre.x > 100000.0) {
        col = sky_dome(p);
    }
    if (g.look.z < 0.5 && (g.resolve.w > 0.5 || g.weather_flow.z > 0.02)) {
        col = aerial_perspective(col, p, centre.x);
    }
    if (g.look.z < 0.5 && centre.x > 2400.0 && centre.x < 100000.0) {
        // far land dissolves into the sky, so no edge of the world is ever drawn
        col = mix(col, sky_dome(p), smoothstep(2400.0, 11000.0, centre.x));
    }
    if (g.look.z < 0.5 && g.resolve.w > 0.5) {
        col = sun_shafts(col, p, centre.x);
    }
    if (g.look.z < 0.5 && g.weather_flow.w > 0.5) {
        col = tilt_focus(col, p);
    }
    var volume_opacity = 0.0;
    if (g.look.z < 0.5 && (g.fog.w > 0.005 || g.shade.w > 0.12)) {
        let volume = atmosphere_at(vec2<f32>(p) + 0.5, centre.x);
        volume_opacity = max(volume.r, volume.b);
        let sun_height = smoothstep(0.04, 0.58, g.sun.z);
        let sun_glow = mix(vec3<f32>(0.64, 0.43, 0.30), vec3<f32>(0.47, 0.51, 0.55), sun_height);
        col = mix(col, g.fog.rgb * 0.82, volume.r);
        let daylight = smoothstep(0.02, 0.37, g.sun.z);
        let ambient = mix(vec3<f32>(0.14, 0.21, 0.32), vec3<f32>(0.47, 0.53, 0.63), daylight)
            * (1.0 - 0.16 * g.weather_flow.z);
        col = mix(col, ambient, volume.b) + sun_glow * volume.g;
    }
    if (g.lightning.z > 0.035 && g.look.z < 0.5) {
        col += vec3<f32>(0.75, 0.88, 1.0) * lightning_bolt(p, centre.x) * 2.5;
    }
    return vec4<f32>(temporal_colour(col, p, centre.x, volume_opacity), min(65000.0, centre.x));
}

fn bright_pass(p: vec2<i32>) -> vec3<f32> {
    let c = textureLoad(current_frame, p, 0).rgb;
    let peak = max(c.r, max(c.g, c.b));
    return c * smoothstep(0.91, 1.17, peak) * min(0.63, max(0.0, peak - 0.84));
}

@fragment
fn fs_present(e: EOut) -> @location(0) vec4<f32> {
    let bounds = vec2<i32>(g.screen.xy) - vec2<i32>(1);
    let p = min(vec2<i32>(e.clip.xy * g.resolve.xy), bounds);
    var col = textureLoad(current_frame, p, 0).rgb;
    if (any(g.resolve.xy < vec2<f32>(1.0))) {
        let uv = e.clip.xy * g.resolve.xy / g.screen.xy;
        col = textureSampleLevel(current_frame, terrain_sampler, uv, 0.0).rgb;
    }
    if (g.temporal.y > 0.010) {
        let glow = array<vec2<i32>, 8>(vec2<i32>(-2, 0), vec2<i32>(2, 0), vec2<i32>(0, -2), vec2<i32>(0, 2),
            vec2<i32>(-4, -3), vec2<i32>(4, -3), vec2<i32>(-4, 3), vec2<i32>(4, 3));
        var scatter = vec3<f32>(0.0);
        for (var i = 0; i < 8; i++) {
            let q = clamp(p + glow[i], vec2<i32>(0), bounds);
            scatter += bright_pass(q) * select(0.13, 0.055, i < 4);
        }
        col += scatter * g.temporal.y * 0.53;
    }
    col *= g.temporal.z;
    if (g.temporal.w > 0.5) {
        // the grade: a gentle S curve for depth and a little more colour in the midtones
        let c = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
        col += (c * c * (3.0 - 2.0 * c) - c) * 0.30;
        let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
        col = max(vec3<f32>(0.0), mix(vec3<f32>(luma), col, 1.09));
        let peak = max(col.r, max(col.g, col.b));
        let shoulder = max(0.0, peak - 0.83);
        let compressed = min(peak, 0.83) + shoulder / (1.0 + shoulder * 5.2);
        col *= compressed / max(0.0001, peak);
    }
    return vec4<f32>(col, 1.0);
}
