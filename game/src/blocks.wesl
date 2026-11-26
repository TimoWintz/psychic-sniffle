// The blocks' surfaces: the standard material, with a fine pattern drawn inside every face so turf,
// rock and sand stop reading as flat squares. Each vertex carries its block's material in `uv.x`
// (the material's id over 32); `uv.y` is unused. The pattern only scales the colour: lighting, the
// grade and the haze treat it like any other colour. It fades out where a texel of it would be
// smaller than a pixel, so far-off ground does not shimmer.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
};

// How strongly the pattern shows, 0 to 1.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> detail: f32;

// Texels per metre: a block face is a 16 by 16 picture.
const TEXELS: f32 = 16.0;

// The materials' ids (physics_core::materials::Material).
const ROCK: u32 = 1u;
const SOIL: u32 = 2u;
const TURF: u32 = 3u;
const SAND: u32 = 4u;
const STEEL: u32 = 6u;
const COPPER: u32 = 7u;
const TIMBER: u32 = 8u;
const CONCRETE: u32 = 9u;
const HULL: u32 = 11u;
const FOLIAGE: u32 = 12u;
const QUARTZ: u32 = 13u;
const LIMESTONE: u32 = 14u;
const GROOVE: u32 = 15u;

fn hash(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
}

// Smooth noise over a plane, one lattice point per unit.
fn noise(p: vec2<f32>, seed: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(vec3(i, seed));
    let b = hash(vec3(i + vec2(1.0, 0.0), seed));
    let c = hash(vec3(i + vec2(0.0, 1.0), seed));
    let d = hash(vec3(i + vec2(1.0, 1.0), seed));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// How much a face's colour is scaled at a point on it. `face` is the point in the face's own plane
// (metres, the second axis upwards on a side), `up` whether the face looks up, `edge` the distance to
// the nearest edge of the block's face.
fn pattern(id: u32, face: vec2<f32>, up: bool, edge: f32) -> f32 {
    let texel = floor(face * TEXELS);
    let h = hash(vec3(texel, f32(id)));
    switch id {
        case TURF: {
            // Tufts of light and shade, speckled, and now and then a pale blade.
            let tuft = (hash(vec3(floor(face * 8.0), 89.0)) - 0.5) * 0.12;
            let blade = select(0.0, 0.12, hash(vec3(texel, 91.0)) > 0.94);
            return 1.0 + tuft + (h - 0.5) * 0.16 + blade;
        }
        case ROCK: {
            let mottle = (noise(face * 3.0, 5.0) - 0.5) * 0.14 + (h - 0.5) * 0.08;
            if up {
                return 1.0 + mottle;
            }
            // Fine bedding lines across the face, broken here and there.
            let run = hash(vec3(floor(texel.x / 5.0), texel.y, 17.0));
            let line = select(0.0, 0.12, run > 0.82);
            return 1.0 + mottle - line;
        }
        case SOIL: {
            let clods = (hash(vec3(floor(face * 8.0), 23.0)) - 0.5) * 0.18;
            return 1.0 + clods + (h - 0.5) * 0.06;
        }
        case SAND: {
            var grain = (h - 0.5) * 0.12;
            if up {
                // Ripples left by the wind.
                grain += 0.06 * sin((face.x + face.y * 0.6) * 9.0 + noise(face * 1.5, 29.0) * 4.0);
            }
            return 1.0 + grain;
        }
        case LIMESTONE, GROOVE: {
            // The builders' dressed stone: smooth, faintly clouded, its edges worn darker.
            let cloud = (noise(face * 2.5, 37.0) - 0.5) * 0.06;
            let worn = 1.0 - 0.16 * (1.0 - smoothstep(0.0, 0.07, edge));
            return (1.0 + cloud + (h - 0.5) * 0.02) * worn;
        }
        case FOLIAGE: {
            // Fronds: strong light and shade, and gaps of deep shadow.
            let gap = select(1.0, 0.72, h < 0.12);
            return (1.0 + (hash(vec3(floor(face * 8.0), 43.0)) - 0.5) * 0.22) * gap;
        }
        case TIMBER: {
            if up {
                // Rings around the trunk's heart.
                let r = length(fract(face) - 0.5);
                return 1.0 + 0.07 * sin(r * 48.0) + (h - 0.5) * 0.05;
            }
            // Grain running up the trunk.
            let grain = hash(vec3(texel.x, floor(texel.y / 6.0), 47.0));
            return 1.0 + (grain - 0.5) * 0.16;
        }
        case STEEL, COPPER, CONCRETE, HULL: {
            // Panels half a metre across, with dark seams between them.
            let p = fract(face * 2.0);
            let seam = min(min(p.x, 1.0 - p.x), min(p.y, 1.0 - p.y));
            let dark = 1.0 - 0.2 * (1.0 - smoothstep(0.0, 0.04, seam));
            return dark * (1.0 + (h - 0.5) * 0.05);
        }
        case QUARTZ: {
            // Sleeping quartz: facets catching the light unevenly.
            return 1.0 + (hash(vec3(floor(face * 4.0), 53.0)) - 0.5) * 0.24;
        }
        default: {
            return 1.0;
        }
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // The face's own plane: across and up on a side, the ground's two directions on a top.
    let n = abs(in.world_normal);
    let p = in.world_position.xyz;
    let up = n.y > 0.5;
    var face = p.xz;
    if !up {
        face = select(vec2(p.x, p.y), vec2(p.z, p.y), n.x > 0.5);
    }
    // Fade the pattern out where its texels shrink below a pixel. Derivatives first, while every
    // fragment of the quad still runs the same code.
    let pixel = fwidth(face * TEXELS);
    let fade = 1.0 - smoothstep(0.6, 1.6, max(pixel.x, pixel.y));
    let edge_of = fract(face);
    let edge = min(min(edge_of.x, 1.0 - edge_of.x), min(edge_of.y, 1.0 - edge_of.y));

@if(VERTEX_UVS_A) {
    let id = u32(round(in.uv.x * 32.0));
    let scale = mix(1.0, pattern(id, face, up, edge), detail * fade);
    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * scale, pbr_input.material.base_color.a);
}

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
