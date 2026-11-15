// Cel shader: three stepped tones per face, hard shadows from one sun, and an ink edge pass.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    sun: vec4<f32>,    // xyz toward the sun, w shadow texel size in metres
    ink: vec4<f32>,    // rgb ink colour, a ink width in pixels (1 or 2)
    shade: vec4<f32>,  // x multiplier for a shadowed face, y shadow depth bias
    screen: vec4<f32>, // width, height of the main targets
    tone: vec4<f32>,   // rgb flat tint of the whole picture, a share of lit windows
    look: vec4<f32>,   // x season (1 autumn, 2 spring, 3 summer), y snow, z map view (desaturate), w share of lamps and vehicle lights on (0 to 1)
    wx: vec4<f32>,     // x snow cover 0 to 1, y wet ground, zw wind (east, north) in m/s
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;

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
};

// Tree crowns lean downwind and sway with the wind's strength. Only leaf-coloured vertices move.
fn sway(p: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let w = g.wx.zw;
    if (abs(w.x) + abs(w.y) < 0.5) {
        return p;
    }
    if (leaf_step(c, 88.0, 150.0, 76.0) < 0 && leaf_step(c, 70.0, 128.0, 66.0) < 0) {
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

// 1 when the sun reaches the point, 0 when a caster is in front of it.
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

// Tone multiplier for natural ground at a world place and camera distance.
fn ground_grain(w: vec2<f32>, dist: f32, snowy: bool) -> f32 {
    var k = 1.0;
    if (dist < 1800.0) {
        // broad patches: worn and lush grass, damp and dry soil, drifts
        let n = vnoise(w / 9.0) * 0.6 + vnoise(w / 27.0) * 0.4;
        if (n > 0.64) {
            k = 1.04;
        } else if (n < 0.38) {
            k = 0.955;
        }
    }
    if (dist < 420.0) {
        // tufts and stones up close
        let t = hash2(vec2<i32>(floor(w / 1.3)));
        if (t > 0.9) {
            k = k * 0.93;
        } else if (t < 0.05) {
            k = k * 1.05;
        }
    }
    if (snowy) {
        k = 1.0 + (k - 1.0) * 0.5;
    }
    return k;
}

// Layers of a steep rock face: the tone steps with height in uneven bands, with a thin dark ledge
// line between them, so a wall reads as rock whatever the colour of the style.
fn rock_bands(z: f32, nz: f32) -> f32 {
    let t = z / 6.5;
    let band = floor(t);
    let tone = 0.86 + 0.2 * hash2(vec2<i32>(i32(band), 31));
    var k = tone;
    if (fract(t) < 0.07) {
        k = 0.74;
    }
    // fade out towards gentle ground so scree is not striped
    let w = clamp((0.72 - nz) / 0.12, 0.0, 1.0);
    return 1.0 + (k - 1.0) * w;
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

@fragment
fn fs_main(v: VOut) -> FOut {
    let n = normalize(v.normal);
    let l = dot(n, g.sun.xyz);
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
    var emissive = false;
    let season = i32(g.look.x + 0.5);
    let pstep = patch_step(col);
    if (pstep >= 0) {
        if (season == 1 && g.look.z < 0.5 && g.look.y < 0.5) {
            col = autumn_leaf(pstep, pstep >= 2) * 0.94;
        } else {
            discard;
        }
    }
    if (ground && near(col, 247.0, 248.0, 253.0) && (g.look.y < 0.5 || g.look.z > 0.5)) {
        discard;
    }
    if (!cel && near(col, 250.0, 232.0, 150.0)) {
        if (f32(hash_u(v.id) % 1000u) >= g.look.w * 1000.0 || g.look.z > 0.5) {
            discard;
        }
        col = vec3<f32>(214.0, 194.0, 120.0) / 255.0;
        emissive = true;
    }
    if (g.look.z < 0.5) {
        // seasonal ground and leaves
        if (ground && season > 0 && is_green(col)) {
            col = season_ground(col, season);
        }
        if (season > 0 && cel) {
            let s1 = leaf_step(col, 88.0, 150.0, 76.0);
            let s2 = leaf_step(col, 70.0, 128.0, 66.0);
            if (season == 1) {
                if (s1 >= 0) { col = autumn_leaf(s1, false); }
                if (s2 >= 0) { col = autumn_leaf(s2, true); }
            } else {
                if (s1 >= 0) { col = season_leaf(s1, false, season); }
                if (s2 >= 0) { col = season_leaf(s2, true, season); }
            }
        }
        // lit windows and headlights
        if (!cel && g.tone.w > 0.0 && (near(col, 98.0, 156.0, 204.0) || near(col, 64.0, 88.0, 118.0))) {
            if (f32(hash_u(v.id) % 1000u) < g.tone.w * 1000.0) {
                col = vec3<f32>(255.0, 218.0, 112.0) / 255.0;
                emissive = true;
            }
        }
        if (!cel && g.look.w > 0.0 && near(col, 255.0, 236.0, 150.0)) {
            if (f32(hash_u(v.id) % 1000u) < g.look.w * 1000.0) {
                emissive = true;
            }
        }
        if (!cel && g.look.w > 0.0 && near(col, 222.0, 36.0, 44.0)) {
            if (f32(hash_u(v.id) % 1000u) < g.look.w * 1000.0) {
                col = vec3<f32>(255.0, 52.0, 52.0) / 255.0;
                emissive = true;
            }
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
    if (cel && ground && g.look.z < 0.5 && natural(col)) {
        let snowy = min(col.r, min(col.g, col.b)) > 0.88;
        col = col * ground_grain(v.world.xy, v.dist, snowy);
    }
    if (cel && ground && n.z < 0.72 && natural(col)) {
        col = col * rock_bands(v.world.z, n.z);
    }
    var k = 1.0;
    if (cel && ground) {
        // hillshade: stepped tones by how the slope faces the sun compared with level ground
        let d = l - g.sun.z;
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
    } else if (cel) {
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
    if (l > 0.02 && lit(v.world, n) < 0.5) {
        k = min(k, g.shade.x);
    }
    col = col * k;
    if (!emissive) {
        col = col * g.tone.rgb;
    }
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

const EXEMPT: u32 = 16777215u;

fn aux_at(p: vec2<i32>, s: i32) -> vec2<f32> {
    let size = vec2<i32>(i32(g.screen.x), i32(g.screen.y));
    let q = clamp(p, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
    let t = textureLoad(aux_tex, q, s);
    return vec2<f32>(bitcast<f32>(t.x), f32(t.y));
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

@fragment
fn fs_edge(e: EOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(i32(e.clip.x), i32(e.clip.y));
    var acc = vec3<f32>(0.0);
    // ink thins out as a pixel covers more of the world: a one pixel line from about half a
    // metre a pixel, none from a metre and a half (whole-town views keep their roof colours)
    let px = g.screen.z * aux_at(p, 0).x;
    let inked = px < 1.5;
    for (var s = 0; s < SAMPLE_COUNT; s = s + 1) {
        var ink = false;
        if (inked) {
            ink = ink_at(p, s);
            if (!ink && g.ink.a > 1.5 && px < 0.5) {
                ink = ink_at(p + vec2<i32>(-1, 0), s) || ink_at(p + vec2<i32>(0, -1), s);
            }
        }
        if (ink) {
            acc = acc + g.ink.rgb;
        } else {
            acc = acc + textureLoad(colour_tex, p, s).rgb;
        }
    }
    return vec4<f32>(acc / f32(SAMPLE_COUNT), 1.0);
}
