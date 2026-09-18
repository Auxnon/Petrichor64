@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.5),
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5, -0.5),
    );
    let pos = positions[in_vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}


@vertex
fn gui_vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.6),
        vec2<f32>(-0.5, -0.6),
        vec2<f32>( 0.5, -0.5),
    );
    let pos = positions[in_vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);
    }

@fragment
fn gui_fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 0.0, 1.0);
}

@vertex
fn sky_vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
   var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.7),
        vec2<f32>(-0.5, -0.7),
        vec2<f32>( 0.5, -0.5),
    );
    let pos = positions[in_vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);}

@fragment
fn sky_fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}

@vertex
fn post_vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
   var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.8),
        vec2<f32>(-0.5, -0.8),
        vec2<f32>( 0.5, -0.5),
    );
    let pos = positions[in_vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);
}

@fragment
fn post_fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 1.0, 1.0, 1.0);
}
