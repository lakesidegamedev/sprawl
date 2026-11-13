// Cel shader: three stepped tones per face, hard shadows from one sun, and an ink edge pass.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    sun: vec4<f32>,    // xyz toward the sun, w shadow texel size in metres
    ink: vec4<f32>,    // rgb ink colour, a ink width in pixels (1 or 2)
    shade: vec4<f32>,  // x multiplier for a shadowed face, y shadow depth bias
    screen: vec4<f32>, // width, height of the main targets
    tone: vec4<f32>,   // rgb flat tint of the whole picture, a share of lit windows
    look: vec4<f32>,   // x autumn, y snow, z map view (desaturate), w lamps on (night)
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

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
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
    return g.light_vp * vec4<f32>(v.pos, 1.0);
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
    @location(1) aux: vec4<f32>,
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
    if (g.look.z < 0.5) {
        // autumn leaves
        if (g.look.x > 0.5 && cel) {
            let s1 = leaf_step(col, 88.0, 150.0, 76.0);
            if (s1 >= 0) { col = autumn_leaf(s1, false); }
            let s2 = leaf_step(col, 70.0, 128.0, 66.0);
            if (s2 >= 0) { col = autumn_leaf(s2, true); }
        }
        // lit windows and headlights
        if (!cel && g.tone.w > 0.0 && (near(col, 98.0, 156.0, 204.0) || near(col, 64.0, 88.0, 118.0))) {
            if (f32(hash_u(v.id) % 1000u) < g.tone.w * 1000.0) {
                col = vec3<f32>(255.0, 218.0, 112.0) / 255.0;
                emissive = true;
            }
        }
        if (!cel && g.look.w > 0.5 && near(col, 255.0, 236.0, 150.0)) {
            emissive = true;
        }
        // snow
        if (g.look.y > 0.5) {
            if (ground) {
                if (col.g > col.b + 0.08 && col.g > col.r - 0.06) {
                    col = vec3<f32>(236.0, 241.0, 247.0) / 255.0;
                }
            } else if (cel && !exempt && v.tint == 0u && n.z > 0.35) {
                col = vec3<f32>(238.0, 242.0, 248.0) / 255.0;
            }
        }
    } else if (!keep) {
        let lum = dot(col, vec3<f32>(0.3, 0.59, 0.11));
        col = mix(vec3<f32>(lum), col, 0.12);
    }
    var k = 1.0;
    if (cel) {
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
    o.aux = vec4<f32>(v.dist, f32(v.id), 0.0, 1.0);
    return o;
}

// ---- edge pass ----

@group(1) @binding(0) var colour_tex: texture_2d<f32>;
@group(1) @binding(1) var aux_tex: texture_2d<f32>;

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

fn aux_at(p: vec2<i32>) -> vec2<f32> {
    let size = vec2<i32>(i32(g.screen.x), i32(g.screen.y));
    let q = clamp(p, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
    return textureLoad(aux_tex, q, 0).xy;
}

// True when this pixel is on the near side of a face change: one pixel wide ink line.
fn ink_at(p: vec2<i32>) -> bool {
    let a = aux_at(p);
    let ida = u32(a.y);
    if (ida == EXEMPT) {
        return false;
    }
    let tol = 0.002 * a.x + 0.05;
    var offs = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    for (var i = 0; i < 4; i = i + 1) {
        let b = aux_at(p + offs[i]);
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
    var ink = ink_at(p);
    if (!ink && g.ink.a > 1.5) {
        ink = ink_at(p + vec2<i32>(-1, 0)) || ink_at(p + vec2<i32>(0, -1));
    }
    if (ink) {
        return vec4<f32>(g.ink.rgb, 1.0);
    }
    return vec4<f32>(textureLoad(colour_tex, p, 0).rgb, 1.0);
}
