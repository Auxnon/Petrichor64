struct VertexOutput {
    [[builtin(position)]] proj_position: vec4<f32>;
    [[location(0)]] world_normal: vec3<f32>;
    [[location(1)]] world_position: vec4<f32>;
    [[location(2)]] tex_coords: vec2<f32>;
    [[location(3)]] vpos:vec4<f32>;
};


struct Globals {
    view_mat: mat4x4<f32>;
    proj_mat: mat4x4<f32>;
    time: vec4<f32>;
    //num_lights: vec4<u32>;
};

// [[group(1), binding(0)]]
// var<uniform> u_entity: Entity;

[[group(0), binding(0)]]
var<uniform> globals: Globals;
[[group(0), binding(1)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(2)]]
var s_diffuse: sampler;

struct Entity {
    matrix: mat4x4<f32>;
    color: vec4<f32>;
    uv_mod:vec4<f32>;
    effects:vec4<u32>;
};

[[group(1), binding(0)]]
var<uniform> ent: Entity;

[[stage(vertex)]]
fn vs_main(
    [[location(0)]] position: vec4<i32>,
    [[location(1)]] normal: vec4<i32>,
    [[location(2)]] tex_coords: vec2<f32>,
) -> VertexOutput {

    let billboarded=false;
    let w = ent.matrix;
    let world_pos =  ent.matrix *vec4<f32>(position); 
    var out: VertexOutput;
    out.world_normal = mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * vec3<f32>(normal.xyz);
    out.world_position = world_pos;
    let v=globals.view_mat;


    //globals.view_proj
    let pos=vec4<f32>(position);
    //    out.proj_position = globals.view_proj * world_pos;

    if(ent.effects[0] > 0u){
        out.proj_position=globals.proj_mat*(globals.view_mat*ent.matrix*vec4<f32>(0.0, 0.0, 0.0, 1.0) +vec4<f32>(pos.x,pos.y,0.,0.));
    }else{
        out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);
    }

    out.tex_coords=(tex_coords*vec2<f32>(ent.uv_mod.z,ent.uv_mod.w))+vec2<f32>(ent.uv_mod.x,ent.uv_mod.y);
    let vpos:vec4<f32>=out.proj_position;
    //let vpos2=vec4<f32>(vpos.x,vpos.y,vpos.z+globals.time,vpos.w);
    //vpos.xyz=(vpos.y%0.1)*10.;
    //let vpos2=(vpos%1.0);
    //let vpos3=vec4<f32>(1.,0.,0.,0.);//vpos*100.;
    out.vpos=vec4<f32>(world_pos.x,world_pos.y,world_pos.z+globals.time.x,world_pos.w);
    return out;
}

struct GuiFrag {
     [[builtin(position)]] pos: vec4<f32>; 
    [[location(1)]] screen: vec2<f32>;
};

[[stage(vertex)]]
fn gui_vs_main([[builtin(vertex_index)]] in_vertex_index: u32) ->GuiFrag{
    // [[location(0)]] position: vec4<i32>,
    // [[location(1)]] normal: vec4<i32>,
    //) -> VertexOutput {
    

    var out: GuiFrag;
    // //return out;
    // let n=i32(in_vertex_index)%2;
    // let m=i32(in_vertex_index)/2;
    // let x = f32(n - 1);
    // let y = f32(max(m+n,1)  - 1);

    if (in_vertex_index==0u){
        out.pos=vec4<f32>(-1.,-1., 0.0, 1.0);
    }else if (in_vertex_index==1u){
        out.pos=vec4<f32>(1.,-1., 0.0, 1.0);
    }else if (in_vertex_index==2u){
        out.pos=vec4<f32>(-1.,1., 0.0, 1.0);
    }else{
        out.pos=vec4<f32>(1.,1., 0.0, 1.0);
    }
    
    out.screen=vec2<f32>(globals.time.z,globals.time.w);
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
    //f_color=f_color*v; //vec4<f32>(v,v,v,1.0);
    return;
}

[[stage(fragment)]]
fn fs_main( in: VertexOutput) -> FragmentOutput {
    main_2(in); 
    let e3: vec4<f32> = f_color;
    if (e3.a < 0.5) {
        discard;
    }
    return FragmentOutput(e3);
}

[[stage(fragment)]]
fn gui_fs_main(in: GuiFrag) ->  [[location(0)]] vec4<f32> {
  
    // let e3: vec4<f32> = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    // if (e3.a < 0.5) {
    //     discard;
    // }
    //return FragmentOutput(e3);
    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);
    f_color=textureSample(t_diffuse, s_diffuse, p);
   return f_color;//vec4<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y, 0., 1.0);
}