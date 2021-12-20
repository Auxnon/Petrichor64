struct VertexOutput {
    [[builtin(position)]] proj_position: vec4<f32>;
    [[location(0)]] world_normal: vec3<f32>;
    [[location(1)]] world_position: vec4<f32>;
    [[location(2)]] vpos:vec4<f32>;
};
// var<private> gl_Position: vec4<f32>;
// var<private> gl_VertexIndex: u32;
//var<f32> vpos: vec4<f32>;

// struct Entity {
//     world: mat4x4<f32>;
//     color: vec4<f32>;
// };

//[[block]]
struct Globals {
    view_proj: mat4x4<f32>;
    //num_lights: vec4<u32>;
};

// [[group(1), binding(0)]]
// var<uniform> u_entity: Entity;

[[group(0), binding(0)]]
var<uniform> u_globals: Globals;

//[[block]]
struct Entity {
    world: mat4x4<f32>;
    color: vec4<f32>;
};

[[group(1), binding(0)]]
var<uniform> u_entity: Entity;

[[stage(vertex)]]
fn vs_main(
    [[location(0)]] position: vec4<i32>,
    [[location(1)]] normal: vec4<i32>,

) -> VertexOutput {
    let w = u_entity.world;
    let world_pos =  u_entity.world *vec4<f32>(position); 
    var out: VertexOutput;
    out.world_normal = mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * vec3<f32>(normal.xyz);
    out.world_position = world_pos;
    out.proj_position = u_globals.view_proj * world_pos;
    let vpos:vec4<f32>=out.proj_position;
    //vpos.xyz=(vpos.y%0.1)*10.;
    //let vpos2=(vpos%1.0);
    //let vpos3=vec4<f32>(1.,0.,0.,0.);//vpos*100.;
    out.vpos=world_pos;
    return out;
}

struct FragmentOutput {
    [[location(0)]] f_color: vec4<f32>;
};

var<private> f_color: vec4<f32>;

fn main_2(in:VertexOutput) {
    //f_color = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    f_color=vec4<f32>(abs(in.vpos.y)%1.,1.,1.,1.0);
    return;
}

[[stage(fragment)]]
fn fs_main( in: VertexOutput) -> FragmentOutput {
    main_2(in);
    let e3: vec4<f32> = f_color;
    return FragmentOutput(e3);
}