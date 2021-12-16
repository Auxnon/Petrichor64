struct VertexOutput {
    [[builtin(position)]] member: vec4<f32>;
};

var<private> gl_Position: vec4<f32>;
var<private> gl_VertexIndex: u32;

fn main_1() {
    var local: array<vec2<f32>,3u> = array<vec2<f32>,3u>(vec2<f32>(0.0, 0.5), vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5));

    let e3: u32 = gl_VertexIndex;
    let e6: vec2<f32> = local[e3];
    gl_Position = vec4<f32>(e6, 0.0, 1.0);
    return;
}

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] param: u32,
[[location(0)]] position: vec4<i32>,
    [[location(1)]] normal: vec4<i32>,
    ) -> VertexOutput {
    gl_VertexIndex = param;
    main_1();
    let e18: vec4<f32> = gl_Position;
    return VertexOutput(e18);
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