// Cel shader: three stepped tones per face, hard shadows from one sun, and an ink edge pass.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,     // xyz camera position, w near plane
    fog: vec4<f32>,     // rgb atmospheric light, w mist density
    terrain: vec4<f32>, // elevation base and range, inverse map size and texture size
    local_lights: vec4<f32>, // world tile origin, streetlight and headlight strength
    sun: vec4<f32>,    // xyz toward the sun, w shadow texel size in metres
    ink: vec4<f32>,    // rgb ink colour, a ink width in pixels (1 or 2)
    shade: vec4<f32>,  // x multiplier for a shadowed face, y shadow depth bias, z seasonal blend, w overcast
    screen: vec4<f32>, // width, height of the main targets
    tone: vec4<f32>,   // rgb flat tint of the whole picture, a share of lit windows
    look: vec4<f32>,   // x season (1 autumn, 2 spring, 3 summer), y snow, z map view (desaturate), w share of lamps and vehicle lights on (0 to 1)
    wx: vec4<f32>,     // x snow cover 0 to 1, y wet ground, zw wind (east, north) in m/s
    precipitation: vec4<f32>, // rain, snow and the lower corner of the weather grid
    rock: array<vec4<f32>, 8>, // rows 0 to 3 strata colours, 4 talus, 5 (snow line, tree line, band height, on), 6 (snow rgb, winter drop)
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;
@group(0) @binding(5) var lights_tex: texture_2d<f32>;

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

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let p = sway(v.pos, v.color.rgb * 255.0);
    o.clip = g.view_proj * vec4<f32>(p, 1.0);
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
    return g.light_vp * vec4<f32>(inst_world(v, i), 1.0);
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_vp * vec4<f32>(sway(v.pos, v.color.rgb * 255.0), 1.0);
}

// Fraction of the comparison footprint that the sun reaches.
fn lit(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let p = world + n * g.sun.w * 1.5;
    let c = g.light_vp * vec4<f32>(p, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || c.z > 1.0) {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_tex, shadow_cmp, uv, c.z - g.shade.y);
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
fn tuft(w: vec2<f32>, cell: f32, seed: i32) -> f32 {
    let q = w / cell;
    let ci = vec2<i32>(floor(q));
    let h1 = hash2(ci + vec2<i32>(seed, 3));
    if (h1 < 0.55) {
        return 0.0;
    }
    let h2 = hash2(ci + vec2<i32>(7, seed));
    let c = vec2<f32>(0.3 + 0.4 * h1, 0.3 + 0.4 * h2);
    let d = fract(q) - c;
    // two blades leaning apart (a small V), width about a seventh of the cell
    let up = d.y * 1.0;
    let a = abs(d.x - up * 0.55);
    let b = abs(d.x + up * 0.55);
    let in_v = d.y > -0.05 && d.y < 0.3 && (a < 0.07 || b < 0.07);
    return select(0.0, 1.0, in_v);
}

// Stepped tone multiplier and optional colour for natural ground at a world place and camera
// distance. Fine detail fades into the mid-scale pattern with distance, never into flat colour.
fn ground_detail(col: vec3<f32>, base: vec3<f32>, kind: i32, w: vec2<f32>, dist: f32, snowy: bool, season: i32) -> vec3<f32> {
    var c = col;
    let near_d = dist < 140.0;
    let mid_d = dist < 600.0;
    // broad patches at three scales, in three tone bands
    let big = vnoise(w / 38.0);
    let mid = vnoise(w / 11.0);
    let small = vnoise(w / 3.6 + vec2<f32>(5.0, 9.0));
    var n = big * 0.4 + mid * 0.4 + small * 0.2;
    if (!mid_d) {
        n = big * 0.6 + mid * 0.4;
    }
    var k = 1.0;
    if (n > 0.66) {
        k = 1.05;
    } else if (n < 0.34) {
        k = 0.955;
    }
    if (snowy) {
        // soft blue shadows in the lee of drifts, no sparkle
        let d = vnoise(w / 7.0) * 0.6 + vnoise(w / 2.3) * 0.4;
        var s = vec3<f32>(1.0);
        if (d < 0.36) {
            s = vec3<f32>(0.88, 0.93, 1.02);
        } else if (d < 0.46) {
            s = vec3<f32>(0.95, 0.97, 1.01);
        }
        return c * s;
    }
    if (kind == 1 || kind == 4) {
        if (kind == 4) {
            // mown stripes
            let sp = floor((w.x + w.y * 0.35) / 5.0);
            if ((i32(sp) & 1) == 0) {
                k = k * 1.045;
            } else {
                k = k * 0.97;
            }
        }
        // clumps a couple of metres across, darker
        if (mid_d && vnoise(w / 1.9 + vec2<f32>(31.0, 4.0)) > 0.74) {
            k = k * 0.95;
        }
        // broad patches of yellower and deeper green, big enough to read from the far view
        let broad = vnoise(w / 150.0 + vec2<f32>(13.0, 2.0)) * 0.7 + vnoise(w / 55.0) * 0.3;
        if (broad > 0.6) {
            c = c * vec3<f32>(1.07, 1.03, 0.88);
        } else if (broad < 0.4) {
            c = c * vec3<f32>(0.9, 0.97, 1.0);
        }
        let footprint = g.screen.z * dist;
        let grass_detail = 1.0 - smoothstep(0.28, 0.85, footprint);
        let clover = vnoise(w * 1.62 + vec2<f32>(11.0, 17.0));
        let meadow = vnoise(w / 6.0);
        c = c * (k + (clover - 0.5) * 0.26 * grass_detail + (meadow - 0.5) * 0.14);
        let stems = 1.0 - smoothstep(0.10, 0.32, footprint);
        if (stems > 0.0 && tuft(w, 0.74, 23) > 0.5) {
            c = mix(c, c * vec3<f32>(0.81, 0.94, 0.79), stems);
        }
        if (near_d) {
            let tf = tuft(w, 1.5, 1);
            if (tf > 0.5) {
                c = c * 0.9;
            }
            let tl = tuft(w + vec2<f32>(0.7, 0.4), 2.3, 5);
            if (tl > 0.5) {
                c = c * 1.1;
            }
        }
        return c;
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
        return c * k;
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
fn wave_normal(w: vec2<f32>, px: f32, pond: bool) -> vec3<f32> {
    let velocity = g.wx.zw;
    let gust = min(length(velocity) / 13.0, 1.0);
    let drift = velocity * 0.14 + vec2<f32>(0.88, 0.44);
    let along = normalize(select(drift, vec2<f32>(0.88, 0.44), dot(drift, drift) < 0.001));
    let across = vec2<f32>(along.x * 0.61 - along.y * 0.79, along.x * 0.79 + along.y * 0.61);
    let t = g.screen.w;
    let long = dot(w, along) * 0.31 - t * (1.5 + gust * 1.3);
    let short = dot(w, across) * 1.73 - t * (3.1 + gust * 2.1);
    let breaks = vnoise(w * 0.067 + vec2<f32>(t * 0.035, 0.0));
    let fine = 1.0 - smoothstep(0.3, 1.35, px);
    let wind_scale = mix(0.40, 1.0, gust) * select(1.0, 0.57, pond);
    let slope = along * cos(long + breaks * 2.4) * 0.17
        + across * cos(short + breaks * 3.7) * (0.01 + 0.095 * fine);
    return normalize(vec3<f32>(-slope * wind_scale * (0.72 + breaks * 0.62), 1.0));
}

fn water_colour(kind: i32, world: vec3<f32>, dist: f32, normal: vec3<f32>) -> vec3<f32> {
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
    let near_horizon = 1.0 - clamp(ray.z, 0.0, 1.0);
    let blue = mix(vec3<f32>(0.30, 0.53, 0.71), vec3<f32>(0.68, 0.79, 0.83), near_horizon);
    let sky = mix(blue, vec3<f32>(0.52, 0.59, 0.64), g.shade.w);
    let facing = max(0.0, dot(normal, eye));
    let fresnel = 0.055 + 0.945 * pow(1.0 - facing, 5.0);
    let half_ray = normalize(eye + g.sun.xyz);
    let spec = pow(max(0.0, dot(half_ray, normal)), mix(117.0, 54.0, g.shade.w));
    let sparkle = spec * (0.27 + 0.55 * (1.0 - g.shade.w)) * lit(world, normal);
    var result = mix(base, sky, fresnel) + vec3<f32>(1.0, 0.86, 0.65) * sparkle;
    if (shore) {
        let t = g.screen.w;
        let foam = vnoise(world.xy * 0.43 + vec2<f32>(t * 0.22, -t * 0.14));
        let fine = 1.0 - smoothstep(0.8, 2.0, g.screen.z * dist);
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
    let bare = dry * (0.24 + 0.34 * hill) * (1.0 - g.wx.x);
    let detail = 1.0 - smoothstep(0.15, 0.65, px);
    let litter = vnoise(world.xy * 1.35);
    let soil = vec3<f32>(0.44, 0.36, 0.26) * (1.0 - g.wx.y * 0.26);
    let meadow = colour * (0.90 + 0.20 * sward + (litter - 0.5) * detail * 0.15);
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
fn season_leaf(step: i32, dark: bool, season: i32) -> vec3<f32> {
    if (season == 2) {
        if (step == 3) {
            if (dark) { return vec3<f32>(226.0, 142.0, 176.0) / 255.0; }
            return vec3<f32>(244.0, 176.0, 198.0) / 255.0;
        }
        if (dark) { return vec3<f32>(92.0, 158.0, 74.0) / 255.0; }
        return vec3<f32>(116.0, 180.0, 86.0) / 255.0;
    }
    if (dark) { return vec3<f32>(50.0, 112.0, 52.0) / 255.0; }
    return vec3<f32>(64.0, 134.0, 58.0) / 255.0;
}


// Natural ground of a land with a rock look: grass on gentle slopes, forest on the lower
// mountainsides, stone below faces, rock on steep slopes (stepped strata, or fractured granite),
// bare ground above the tree line and a clean snow cap above the snow line.
fn terrain_look(world: vec3<f32>, n: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
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
    if (s > rock_s) {
        if (g.rock[7].x > 0.5) {
            // fractured granite: irregular blocks and cracks, no stripes
            let blk = vnoise(world.xy / 6.0 + vec2<f32>(z / 8.0, -z / 6.0));
            let blk2 = vnoise(world.xy / 17.0 + vec2<f32>(-z / 21.0, z / 13.0));
            var pick = 1u;
            if (blk2 > 0.62) {
                pick = 3u;
            } else if (blk2 < 0.34) {
                pick = 0u;
            }
            if (blk < 0.22) {
                pick = 2u;
            }
            return g.rock[pick].rgb;
        }
        let bz = (z + (fine - 0.5) * 1.6) / g.rock[5].z;
        let pick = hash_u(u32(i32(floor(bz)) + 65536)) % 4u;
        var rc = g.rock[pick].rgb;
        // a thin dark ledge at the top of each stratum, and a fainter line inside it
        let lg = g.rock[7].w;
        if (lg > 0.0) {
            let fb = fract(bz);
            if (fb > 0.95) {
                rc = rc * (1.0 - lg);
            } else if (fract(bz * 4.0) < 0.1) {
                rc = rc * (1.0 - lg * 0.24);
            }
        }
        return rc;
    }
    if (s > talus_s) {
        var tc = g.rock[4].rgb;
        if (fine > 0.72) {
            tc = tc * 0.9;
        }
        return tc;
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
fn window_glow(anchor: vec3<f32>, shop: bool) -> f32 {
    let block = vnoise(anchor.xy / 145.0);
    let quantized = vec3<i32>(floor(anchor * 16.0 + 0.5));
    let room = hash_u(bitcast<u32>(quantized.x) ^ (bitcast<u32>(quantized.y) * 165557u)
        ^ (bitcast<u32>(quantized.z) * 224171u));
    let choice = f32(room % 1000u) / 1000.0;
    let schedule = clamp(g.tone.w * (0.87 + block * 0.36) + select(0.0, 0.16, shop), 0.0, 1.0);
    // Individual rooms fade over several simulation minutes rather than crossing a binary cutoff.
    let onset = mix(0.10, 1.04, choice);
    let awake = smoothstep(onset - 0.062, onset + 0.062, schedule);
    return awake * smoothstep(0.015, 0.18, g.tone.w);
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
    return light * g.local_lights.z;
}

// Surface studies in world metres. Filter the grain and seams against the pixel footprint.
fn textured_surface(col: vec3<f32>, base: vec3<f32>, world: vec3<f32>, normal: vec3<f32>, px: f32, cel: bool, object: bool) -> vec3<f32> {
    let w = world.xy;
    let aggregate = 1.0 - smoothstep(0.09, 0.28, px);
    if (near(base, 108.0, 108.0, 120.0)) {
        let repair = vnoise(w * 0.34);
        let dust = vnoise(w * 0.95 + vec2<f32>(19.0, 7.0));
        let specks = hash2(vec2<i32>(floor(w * 7.0))) - 0.5;
        var paved = col * vec3<f32>(0.87, 0.89, 0.90)
            * (0.89 + 0.17 * repair + 0.13 * dust + 0.20 * specks * aggregate);
        if (g.wx.y > 0.05) {
            let pool = vnoise(w * 0.54) * 0.7 + vnoise(w * 1.32) * 0.3;
            let puddle = smoothstep(0.77 - 0.15 * g.wx.y, 0.84 - 0.15 * g.wx.y, pool) * g.wx.y;
            let broken_sky = mix(vec3<f32>(0.46, 0.54, 0.57), vec3<f32>(0.35, 0.42, 0.48), g.shade.w);
            let fine = w * 1.9;
            let tile = vec2<i32>(floor(fine));
            let offset = vec2<f32>(hash2(tile), hash2(tile + vec2<i32>(17, 31))) * 0.5 + 0.25;
            let life = fract(g.screen.w * 1.65 + hash2(tile + vec2<i32>(31, 7)));
            let radius = length(fract(fine) - offset);
            let ring = 1.0 - smoothstep(0.025, 0.105, abs(radius - (0.06 + 0.38 * life)));
            let ripple = ring * (1.0 - life) * g.precipitation.x;
            paved = mix(paved, broken_sky * (1.0 + ripple * 0.47), puddle * (0.50 + 0.16 * dust));
        }
        return paved;
    }
    if (near(base, 194.0, 190.0, 182.0) || near(base, 206.0, 202.0, 194.0)) {
        let cell = w / vec2<f32>(1.8, 0.95);
        let in_tile = fract(cell);
        let seam = 1.0 - smoothstep(0.01, 0.10,
            min(min(in_tile.x, 1.0 - in_tile.x), min(in_tile.y, 1.0 - in_tile.y)));
        let variation = hash2(vec2<i32>(floor(cell))) - 0.5;
        let fade = 1.0 - smoothstep(0.2, 0.62, px);
        let stone = vec3<f32>(0.76, 0.73, 0.68) * (1.0 - 0.22 * g.wx.y);
        return stone * (0.97 + 0.08 * vnoise(w * 0.32) + fade * (0.11 * variation - 0.22 * seam));
    }
    if (near(base, 128.0, 104.0, 84.0) || near(base, 198.0, 194.0, 186.0)) {
        let grains = hash2(vec2<i32>(floor(w * 6.0)));
        return col * (0.92 + 0.14 * vnoise(w * 1.2) + (grains - 0.5) * 0.24 * aggregate);
    }
    if (near(base, 83.0, 69.0, 58.0)) {
        return col * (0.89 + 0.22 * vnoise(w * vec2<f32>(5.0, 1.4)));
    }
    if (!object || !cel || px > 0.49) {
        return col;
    }
    let fade = 1.0 - smoothstep(0.15, 0.49, px);
    if (abs(normal.z) < 0.08 && base.r - base.g > 0.11 && base.g - base.b > 0.03
        && base.r < 0.83 && base.g < 0.65) {
        let along = dot(w, vec2<f32>(-normal.y, normal.x));
        let course = world.z / 0.42;
        let block = along / 0.86 + 0.5 * (floor(course) % 2.0);
        let edge = min(min(fract(course), 1.0 - fract(course)), min(fract(block), 1.0 - fract(block)));
        let mortar = 1.0 - smoothstep(0.02, 0.13, edge);
        let kiln = hash2(vec2<i32>(i32(floor(block)), i32(floor(course))));
        return mix(col * (1.0 + (kiln - 0.5) * 0.23 * fade), vec3<f32>(0.64, 0.59, 0.52), mortar * fade * 0.4);
    }
    if (normal.z > 0.42 && normal.z < 0.94 && !is_green(base)) {
        let run = normalize(normal.xy);
        let row = dot(w, run) / 0.52;
        let seam = 1.0 - smoothstep(0.0, 0.16, fract(row));
        let tile = vec2<i32>(i32(floor(dot(w, vec2<f32>(-run.y, run.x)) * 1.8 + 0.5 * floor(row))), i32(floor(row)));
        return col * (1.0 + fade * ((hash2(tile) - 0.5) * 0.22 - seam * 0.24));
    }
    return col;
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
    return season_leaf(step, dark, season);
}

@fragment
fn fs_main(v: VOut) -> FOut {
    var n = normalize(v.normal);
    var l = dot(n, g.sun.xyz);
    // the true normal of the triangle: steep rock is shaded in flat facets, not in the smooth
    // gradients that interpolated vertex normals give on a coarse grid
    var facet = normalize(cross(dpdx(v.world), dpdy(v.world)));
    if (dot(facet, n) < 0.0) {
        facet = -facet;
    }
    var col = v.color.rgb;
    let a = v.color.a;
    let cel = a > 0.5;
    let keep = a > 0.001 && a < 0.999;
    let ground = v.id == 0u;
    let exempt = v.id == 16777215u;
    if (v.tint != 0u) {
        if (near(col, 255.0, 0.0, 255.0)) {
            col = vec3<f32>(f32((v.tint >> 16u) & 255u), f32((v.tint >> 8u) & 255u), f32(v.tint & 255u)) / 255.0;
        }
    }
    if (ground && abs(col.r * 255.0 - 116.0) < 0.6 && abs(col.g * 255.0 - 172.0) < 0.6 && abs(col.b * 255.0 - 95.0) < 0.6) {
        col = terrain_look(v.world, n, vec3<f32>(116.0, 172.0, 92.0) / 255.0);
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
        col = terrain_look(v.world, n, col);
    }
    var emissive = false;
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
            var wc = vec3<f32>(214.0, 178.0, 66.0);
            if (ws == 1) { wc = vec3<f32>(202.0, 164.0, 54.0); }
            if (season == 2) { wc = vec3<f32>(130.0, 176.0, 84.0); }
            if (season == 1) { wc = vec3<f32>(178.0, 148.0, 88.0); }
            if (season == 0) { wc = vec3<f32>(150.0, 134.0, 100.0); }
            col = wc / 255.0;
        }
        if (fs >= 0) {
            if (season == 0) { discard; }
            var fc = vec3<f32>(250.0, 214.0, 70.0);
            if (fs == 1) { fc = vec3<f32>(250.0, 248.0, 238.0); }
            if (fs == 2) { fc = vec3<f32>(240.0, 128.0, 166.0); }
            if (fs == 3) { fc = vec3<f32>(156.0, 116.0, 208.0); }
            if (season == 1) { fc = vec3<f32>(150.0, 110.0, 70.0); }
            col = fc / 255.0;
        }
        if (bs >= 0) {
            if (season == 2 || season == 0) { discard; }
            col = vec3<f32>(226.0, 192.0, 92.0) / 255.0;
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
                let step = max(s1, s2);
                let dark = s2 >= 0;
                col = mix(leaf_palette(col, step, dark, previous), leaf_palette(col, step, dark, season), season_phase(v.world));
                let foliage = vnoise(v.world.xy * 0.95 + v.world.z * 0.63);
                col = col * (0.86 + foliage * 0.29);
            }
        }
        // Buildings brighten in neighborhoods, followed by continuously lit
        // lamps and vehicles. A window or paired set of lights never samples an outline id.
        if (!cel && g.tone.w > 0.0 && (near(col, 98.0, 156.0, 204.0) || near(col, 64.0, 88.0, 118.0))) {
            glow = vec3<f32>(1.0, 0.84, 0.52);
            glow_amount = window_glow(v.room_anchor, near(col, 64.0, 88.0, 118.0));
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
                    if (h < reach) {
                        col = vec3<f32>(236.0, 241.0, 247.0) / 255.0;
                    }
                }
            } else if (is_cover) {
                let h = vnoise(v.world.xy / 16.0) * 0.7 + vnoise(v.world.xy / 5.0) * 0.3;
                if (h < reach) {
                    col = vec3<f32>(228.0, 235.0, 244.0) / 255.0;
                }
            } else if (cel && !exempt && v.tint == 0u && n.z > 0.35) {
                let h = f32(hash_u(v.id / 16u) % 1000u) / 1000.0;
                if (h < reach) {
                    col = vec3<f32>(238.0, 242.0, 248.0) / 255.0;
                }
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
        n = wave_normal(v.world.xy, g.screen.z * v.dist, wkind == 2);
        col = water_colour(wkind, v.world, v.dist, n);
    } else if (gkind > 0) {
        let snowy = min(col.r, min(col.g, col.b)) > 0.88;
        col = ground_detail(col, v.color.rgb, gkind, v.world.xy, v.dist, snowy, season);
        if ((gkind == 1 || gkind == 4) && !snowy) {
            col = grass_soil(col, v.world, n, g.screen.z * v.dist);
        }
    }
    if (!emissive && g.look.z < 0.5 && v.tint == 0u && wkind == 0) {
        col = textured_surface(col, v.color.rgb, v.world, n, g.screen.z * v.dist, cel, !ground && !exempt);
    }
    var k = 1.0;
    if (cel && ground && wkind == 0) {
        // hillshade: stepped tones by how the slope faces the sun compared with level ground
        var d = l - g.sun.z;
        if (g.rock[7].z > 0.5) {
            // flat desert and sea flats must read flat: a faint swell of the land is not shaded
            d = d * (1.0 - 0.97 * smoothstep(0.9, 0.97, facet.z));
        }
        if (d > 0.03) {
            k = 1.1;
        } else if (d > 0.012) {
            k = 1.05;
        } else if (d > -0.012) {
            k = 1.0;
        } else if (d > -0.03) {
            k = 0.93;
        } else if (d > -0.055) {
            k = 0.86;
        } else if (d > -0.09) {
            k = 0.78;
        } else {
            k = 0.7;
        }
    } else if (cel && wkind == 0) {
        // top brightest, sun side, shade side: three steps
        if (n.z > 0.9) {
            k = 1.0;
        } else if (l > 0.45) {
            k = 0.93;
        } else if (l > 0.1) {
            k = 0.8;
        } else {
            k = 0.66;
        }
    }
    if (l > 0.02 && k > g.shade.x && wkind == 0) {
        k = mix(g.shade.x, k, lit(v.world, n));
    }
    let material_colour = col;
    col = col * k;
    if (k < 0.97 && !emissive && g.look.z < 0.5) {
        // cool sky fill in shade: shadowed colour leans blue instead of just darker
        let sh = clamp((0.97 - k) * 3.0, 0.0, 1.0);
        col = col * mix(vec3<f32>(1.0), vec3<f32>(0.9, 0.96, 1.08), sh);
    }
    if (!emissive) {
        col = col * g.tone.rgb;
    }
    if (g.local_lights.z > 0.01 && g.look.z < 0.5 && !emissive) {
        let incoming = night_illumination(v.world, n);
        // Approximate direct light in linear space while the existing art palette
        // remains in display space. Materials keep their colour in the lit area.
        col = sqrt(col * col + material_colour * material_colour * incoming);
    }
    col = mix(col, glow, glow_amount);
    var o: FOut;
    o.colour = vec4<f32>(col, 1.0);
    o.aux = vec4<u32>(bitcast<u32>(v.dist), v.id, 0u, 1u);
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

const EXEMPT: u32 = 16777215u;

fn aux_at(p: vec2<i32>, s: i32) -> vec2<f32> {
    let size = vec2<i32>(i32(g.screen.x), i32(g.screen.y));
    let q = clamp(p, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
    let t = textureLoad(aux_tex, q, s);
    return vec2<f32>(bitcast<f32>(t.x), f32(t.y));
}

fn terrain_height(xy: vec2<f32>) -> f32 {
    let texel = g.terrain.w;
    let uv = clamp(xy * g.terrain.z, vec2<f32>(0.0), vec2<f32>(1.0));
    let heights = textureSampleLevel(terrain_tex, terrain_sampler,
        uv * (1.0 - texel) + 0.5 * texel, 0.0).rg;
    // Filtering both bytes is equivalent to filtering their combined heights.
    return g.terrain.x + g.terrain.y * dot(heights, vec2<f32>(256.0 / 257.0, 1.0 / 257.0));
}

struct Precipitation {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
    @location(2) @interpolate(flat) snow: f32,
};

@vertex
fn vs_precipitation(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Precipitation {
    let column = instance / 6u;
    let cell = vec2<i32>(g.precipitation.zw) + vec2<i32>(i32(column % 36u), i32(column / 36u));
    let slot = i32(instance % 6u);
    let seed = cell * 6 + vec2<i32>(slot, slot * 17);
    let origin = (vec2<f32>(cell) + vec2<f32>(hash2(seed), hash2(seed + vec2<i32>(41, 29)))) * 12.0;
    let is_snow = g.precipitation.y > g.precipitation.x;
    let strength = max(g.precipitation.x, g.precipitation.y);
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
    let wind = clamp(g.wx.zw, vec2<f32>(-12.0), vec2<f32>(12.0)) * select(0.62, 0.37, is_snow);
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

fn fog_opacity(pixel: vec2<f32>, depth: f32) -> f32 {
    // Clear pixels have no surface to shade. Geometry clips the volume at the
    // visible surface, so distant mist cannot be painted over a foreground roof.
    if (depth > 100000.0 || depth <= g.eye.w) {
        return 0.0;
    }
    let ndc = (pixel / g.screen.xy * 2.0 - 1.0) * vec2<f32>(1.0, -1.0);
    let q = g.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let ray = q.xyz - g.eye.xyz * q.w;
    let ray_length = length(ray);
    let dir = ray / ray_length;
    let hit = g.eye.xyz + ray * depth;
    let limit = (depth - g.eye.w) * ray_length;
    let wind = clamp(g.wx.zw, vec2<f32>(-9.0), vec2<f32>(9.0));
    let shift = (vec2<f32>(0.82, 0.34) + wind * 0.17) * g.screen.w;

    // Short intervals resolve wisps at the ground. Increasing intervals reach the
    // tops of banks and keep the cost independent of zoom and map size.
    let ends = array<f32, 6>(3.5, 8.0, 15.0, 26.0, 45.0, 77.0);
    var start = 0.0;
    var extinction = 0.0;
    for (var i = 0; i < 6; i++) {
        let end = min(ends[i], limit);
        let point = hit - dir * ((start + end) * 0.5);
        let above = point.z - terrain_height(point.xy);
        if (above >= 0.0 && above < 26.0) {
            let flow = point.xy - shift;
            let bank = vnoise(flow * 0.018);
            let roll = vnoise((flow + vec2<f32>(1.3, -0.8) * point.z) * 0.054);
            let shape = smoothstep(0.22, 0.79, bank * 0.72 + roll * 0.28);
            let top = 3.5 + shape * 18.5 * g.fog.w;
            let layer = 1.0 - smoothstep(0.5, top, above);
            extinction += (end - start) * layer * shape;
        }
        if (end >= limit) {
            break;
        }
        start = end;
    }
    return 1.0 - exp(-extinction * g.fog.w * 0.24);
}

@fragment
fn fs_fog(e: EOut) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(e.clip.xy) * 2;
    let depth = aux_at(pixel, 0).x;
    return vec4<f32>(fog_opacity(vec2<f32>(pixel) + 0.5, depth), min(depth, 65504.0), 0.0, 1.0);
}

fn fog_at(pixel: vec2<f32>, depth: f32) -> f32 {
    if (depth > 100000.0) {
        return 0.0;
    }
    let uv = (pixel - 0.5) * 0.5;
    let base = vec2<i32>(floor(uv));
    let f = fract(uv);
    let bound = vec2<i32>(textureDimensions(fog_tex)) - vec2<i32>(1);
    let tol = 1.0 + depth * 0.007;
    var result = 0.0;
    var weights = 0.0;
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            let tap = textureLoad(fog_tex, clamp(base + vec2<i32>(x, y), vec2<i32>(0), bound), 0).rg;
            let a = mix(1.0 - f.x, f.x, f32(x));
            let b = mix(1.0 - f.y, f.y, f32(y));
            let weight = a * b * (1.0 - smoothstep(tol, 2.0 * tol, abs(tap.y - depth)));
            result += tap.x * weight;
            weights += weight;
        }
    }
    if (weights < 0.0001) {
        return fog_opacity(pixel, depth);
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
            return 0.88;
        }
    }
    return 1.0;
}

@fragment
fn fs_edge(e: EOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(i32(e.clip.x), i32(e.clip.y));
    var acc = vec3<f32>(0.0);
    // ink thins out as a pixel covers more of the world: a one pixel line from about half a
    // metre a pixel, none from a metre and a half (whole-town views keep their roof colours)
    let centre = aux_at(p, 0);
    let px = g.screen.z * centre.x;
    let inked = px < 1.5;
    var ao = 1.0;
    if (inked) {
        ao = contact(p, px, centre);
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
            let face = textureLoad(colour_tex, p, s).rgb;
            acc = acc + mix(g.ink.rgb, face, 0.28);
        } else {
            acc = acc + textureLoad(colour_tex, p, s).rgb * ao;
        }
    }
    var col = acc / f32(SAMPLE_COUNT);
    if (g.fog.w > 0.005) {
        col = mix(col, g.fog.rgb, fog_at(e.clip.xy, centre.x));
    }
    return vec4<f32>(col, 1.0);
}
