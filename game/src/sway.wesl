// The tufts in the turf, bent by the wind. Each vertex carries in `uv.x` how high it stands above
// the ground its plant grows from, and in `uv.y` the plant's own phase. The higher a vertex, the
// further it leans (with the square of its height, so a stalk curves); each plant flutters on its
// own, and slow gusts roll across the island from the sea, leaning them all the same way.

import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};

// How far a tip one metre up leans at the strongest gust, in metres.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> reach: f32;

// The way the wind blows, across the ground.
const WIND: vec2<f32> = vec2(0.8, 0.6);

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let v = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));

@if(VERTEX_UVS_A) {
    let height = max(v.uv.x, 0.0);
    let phase = v.uv.y * 6.2831853;
    let t = globals.time;
    // A gust is a slow swell rolling downwind; the flutter is each plant's own.
    let gust = 0.5 + 0.5 * sin(dot(world.xz, WIND) * 0.12 - t * 0.8);
    let flutter = sin(t * 2.3 + phase) * 0.35 + sin(t * 3.7 + phase * 1.7) * 0.15;
    let lean = (gust * 0.8 + flutter) * reach * height * height;
    world = vec4(world.x + WIND.x * lean, world.y - abs(lean) * 0.3 * height, world.z + WIND.y * lean, world.w);
}

    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, vertex_in.instance_index);
@if(VERTEX_UVS_A)
    out.uv = v.uv;
@if(VERTEX_COLORS)
    out.color = v.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}
