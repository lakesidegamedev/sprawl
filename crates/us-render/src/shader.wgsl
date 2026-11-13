// Cel shader: three stepped tones per face, hard shadows from one sun, and an ink edge pass.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    sun: vec4<f32>,    // xyz toward the sun, w shadow texel size in metres
    ink: vec4<f32>,    // rgb ink colour, a ink width in pixels (1 or 2)
    shade: vec4<f32>,  // x multiplier for a shadowed face, y shadow depth bias
    screen: vec4<f32>, // width, height of the main targets
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
    return o;
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

@fragment
fn fs_main(v: VOut) -> FOut {
    let n = normalize(v.normal);
    let l = dot(n, g.sun.xyz);
    var k = 1.0;
    if (v.color.a > 0.5) {
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
    var o: FOut;
    o.colour = vec4<f32>(v.color.rgb * k, 1.0);
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
