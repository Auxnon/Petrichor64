struct VertexOutput {
    [[builtin(position)]] proj_position: vec4<f32>;
    [[location(0)]] world_normal: vec3<f32>;
    [[location(1)]] world_position: vec4<f32>;
    [[location(2)]] tex_coords: vec2<f32>;
    [[location(3)]] vpos:vec4<f32>;
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
    time: vec4<f32>;
    //num_lights: vec4<u32>;
};

// [[group(1), binding(0)]]
// var<uniform> u_entity: Entity;

[[group(0), binding(0)]]
var<uniform> u_globals: Globals;
[[group(0), binding(1)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(2)]]
var s_diffuse: sampler;

//[[block]]
struct Entity {
    world: mat4x4<f32>;
    color: vec4<f32>;
    uv_mod:vec4<f32>;
};

[[group(1), binding(0)]]
var<uniform> u_entity: Entity;

[[stage(vertex)]]
fn vs_main(
    [[location(0)]] position: vec4<i32>,
    [[location(1)]] normal: vec4<i32>,
     [[location(2)]] tex_coords: vec2<f32>,

) -> VertexOutput {
    let w = u_entity.world;
    let world_pos =  u_entity.world *vec4<f32>(position); 
    var out: VertexOutput;
    out.world_normal = mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * vec3<f32>(normal.xyz);
    out.world_position = world_pos;
    out.proj_position = u_globals.view_proj * world_pos;
    out.tex_coords=(tex_coords*vec2<f32>(u_entity.uv_mod.z,u_entity.uv_mod.w))+vec2<f32>(u_entity.uv_mod.x,u_entity.uv_mod.y);
    let vpos:vec4<f32>=out.proj_position;
    //let vpos2=vec4<f32>(vpos.x,vpos.y,vpos.z+u_globals.time,vpos.w);
    //vpos.xyz=(vpos.y%0.1)*10.;
    //let vpos2=(vpos%1.0);
    //let vpos3=vec4<f32>(1.,0.,0.,0.);//vpos*100.;
    out.vpos=vec4<f32>(world_pos.x,world_pos.y,world_pos.z+u_globals.time.x,world_pos.w);
    return out;
}

struct FragmentOutput {
    [[location(0)]] f_color: vec4<f32>;
};

var<private> f_color: vec4<f32>;

fn main_2(in:VertexOutput) {
    //f_color = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    let v=abs((10.*in.vpos.z+0.01)/10.)%1.;
    
    f_color=textureSample(t_diffuse, s_diffuse, in.tex_coords);//vec4<f32>(abs(in.vpos.y)%1.,1.,1.,1.0);
    f_color=f_color*v; //vec4<f32>(v,v,v,1.0);
    return;
}

[[stage(fragment)]]
fn fs_main( in: VertexOutput) -> FragmentOutput {
    main_2(in); 
    let e3: vec4<f32> = f_color;
    return FragmentOutput(e3);
}