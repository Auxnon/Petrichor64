// struct VertexOutput {
//     [[builtin(0)]] v_pos: array<vec2<f32>,3u>;
//     [[builtin(position)]] member: vec4<f32>;
// };

struct VertexOutput {
    [[location(0)]] tex_coord: vec2<f32>;
    [[builtin(position)]] position: vec4<f32>;
};

[[block]]
struct Locals {
    transform: mat4x4<f32>;
};
[[group(0), binding(0)]]
var r_locals: Locals;

var<private> gl_Position: vec4<f32>;
var<private> gl_VertexIndex: u32;

// fn main_1() {
//     //var local: array<vec2<f32>,3u> = array<vec2<f32>,3u>(vec2<f32>(0.0, 0.5), vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5));

//     let e3: u32 = gl_VertexIndex;
//     let e6: vec2<f32> = local[e3];
//     gl_Position = vec4<f32>(e6, 0.0, 1.0);
//     return;
// }

// struct Thingy{
//     [[location(0)]] v_tri: array<vec2<f32>,3u>;
// }

[[stage(vertex)]]
fn vs_main(
    [[builtin(vertex_index)]] param: u32,
    [[location(0)]] position: vec4<f32>,
    [[location(1)]] tex_coord: vec2<f32>
    ) -> VertexOutput {
    //gl_VertexIndex = param;
   // main_1();
    //let e18: vec4<f32> = gl_Position;
    //return VertexOutput(e18);

     var out: VertexOutput;
    out.tex_coord = tex_coord;
    out.position = r_locals.transform * position;
    return out;
}

struct FragmentOutput {
    [[location(0)]] f_color: vec4<f32>;
};

var<private> f_color: vec4<f32>;

fn main_2() {
    f_color = vec4<f32>(0.30000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    return;
}

[[stage(fragment)]]
fn fs_main() -> FragmentOutput {
    main_2();
    let e3: vec4<f32> = f_color;
    return FragmentOutput(e3);
}
