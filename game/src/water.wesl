// The sea's surface. The map (water.rs) says how deep the water stands over each column of the island
// and how near the land is; from those the water is turquoise in the shallows and dark teal in the
// depths, clearer where it is shallow, and white with foam where it meets a shore or a cliff. Small
// waves move its normal, so the sun glints on it and the sky it reflects (strongest at a glancing
// look) wavers.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
};

struct WaterLook {
    shallow: vec4<f32>,
    deep: vec4<f32>,
    foam: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    // Map width and depth in metres, the depth its red 1 stands for, the reach of its green.
    extent: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> look: WaterLook;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var map_sampler: sampler;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// The slope of the surface at a point: a few trains of small waves crossing at angles.
fn slope(p: vec2<f32>, t: f32) -> vec2<f32> {
    var g = vec2(0.0);
    let dirs = array(vec2(0.86, 0.50), vec2(-0.34, 0.94), vec2(0.97, -0.24), vec2(-0.71, -0.70));
    let lengths = array(7.0, 4.3, 2.6, 1.7);
    let heights = array(0.04, 0.028, 0.018, 0.011);
    for (var i = 0; i < 4; i++) {
        let k = 6.2831853 / lengths[i];
        let speed = sqrt(9.81 / k);
        g += dirs[i] * heights[i] * k * cos(dot(dirs[i], p) * k - speed * k * t * 0.35);
    }
    return g;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xz;
    let t = globals.time;

    let m = textureSampleLevel(map, map_sampler, p / look.extent.xy, 0.0);
    let depth = m.r * look.extent.z;
    let near = m.g;

    // Shallow and clear over the sand, deep and dark further out.
    var water = mix(look.shallow, look.deep, 1.0 - exp(-depth / 3.0));

    // The waves tilt the surface; calmer right at the land, where the foam is.
    let g = slope(p, t) * (1.0 - 0.6 * near);
    let n = normalize(vec3(-g.x, 1.0, -g.y));
    pbr_input.N = n;
    pbr_input.world_normal = n;

    // Foam: a broken line where the water touches the land, fainter bands washing in and out just
    // off it, and surf breaking over a reef that lies just under the surface.
    let ragged = noise(p * 1.7 + vec2(t * 0.15, -t * 0.1));
    let line = smoothstep(0.80, 0.94, near + (ragged - 0.5) * 0.12);
    let wash = sin(near * 15.0 - t * 1.4 + ragged * 4.0) * 0.5 + 0.5;
    let bands = smoothstep(0.55, 0.95, wash) * smoothstep(0.35, 0.8, near) * 0.55;
    let reef = (1.0 - smoothstep(0.35, 0.8, depth)) * smoothstep(0.4, 0.75, ragged) * 0.6;
    let foam = clamp(line + bands * step(0.45, noise(p * 3.1 - t * 0.2)) + reef, 0.0, 1.0);

    water = mix(water, look.foam, foam);
    pbr_input.material.base_color = water;
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.9, foam);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);

    // The sky, seen in the water: more of it the more glancing the look (Schlick's Fresnel), less
    // where there is foam.
    let ndv = clamp(dot(n, pbr_input.V), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
    let r = reflect(-pbr_input.V, n);
    let sky = mix(look.horizon.rgb, look.zenith.rgb, pow(clamp(r.y, 0.0, 1.0), 0.6));
    let seen = fresnel * (1.0 - foam) * 0.55;
    out.color = vec4(mix(out.color.rgb, sky, seen), mix(out.color.a, 1.0, seen));

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
