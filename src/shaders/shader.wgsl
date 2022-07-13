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

    //billboard if true
    if(ent.effects[0] > 0u){
        out.proj_position=globals.proj_mat*(globals.view_mat*ent.matrix*vec4<f32>(0.0, 0.0, 0.0, 1.0) +vec4<f32>(pos.x,pos.y,0.,0.));
    }else{
        out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);
    }

    out.tex_coords=(tex_coords*vec2<f32>(ent.uv_mod.z,ent.uv_mod.w))+vec2<f32>(ent.uv_mod.x,ent.uv_mod.y);
    let vpos:vec4<f32>=out.proj_position;
  
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


[[stage(vertex)]]
fn post_vs_main([[builtin(vertex_index)]] in_vertex_index: u32) ->GuiFrag{
    var out: GuiFrag;
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

// [[stage(vertex)]]
// fn post_vs_main([[builtin(vertex_index)]] vertex_index: u32) -> VertexOutput {
//     let x: f32 = f32(i32(vertex_index & 1u) << 2u) - 1.0;
//     let y: f32 = f32(i32(vertex_index & 2u) << 1u) - 1.0;
//     var result: VertexOutput;
//     result.proj_position = vec4<f32>(x, -y, 0.0, 1.0);
//     result.tex_coords = vec2<f32>(x + 1.0, y + 1.0) * 0.5;
//     return result;
// }


fn monitor(texture:texture_2d<f32>,samp:sampler,in_coords:vec2<f32>,globals:vec2<f32>)-> vec4<f32>  {
    var AR: f32;
    var uv: vec2<f32>;
    var output: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    var corner_harshness: f32 = 1.2000000476837158;
    var corner_ease: f32 = 4.0;
    var vv: f32;
    var fade: f32;
    var xx: f32;
    var yy: f32;
    var rr: f32;
    var tuv: vec2<f32>;
    var limit: vec2<f32>;
    var res: f32 = 180.0;
    var res2_: vec2<f32>;
    var res3_: vec2<f32>;
    var res4_: vec2<f32>;
    var pre_i: vec2<f32>;
    var i: vec2<f32>;
    var even: f32 = 0.0;
    var pre_y: f32;
    var tex: vec4<f32>;
    var lum: f32;
    var value: f32;
    var v: f32;
    var tiny: f32;
    var L: f32;
    var wave: f32;
    var scan: f32;
    var split: f32;
    var uv2_: vec2<f32>;
    var uv3_: vec2<f32>;
    var cr: vec2<f32>;
    var cg: vec2<f32>;
    var cb: vec2<f32>;
    var i2_: vec2<f32>;
    var i3_: vec2<f32>;
    var tex2_: vec4<f32>;
    var tex3_: vec4<f32>;
    var ar: f32 = 1.0;
    var ag: f32 = 1.0;
    var ab: f32 = 1.0;
    var pixel_size: f32;
    var col: vec3<f32>;
    var backup: vec3<f32>;

    let iTime=globals.x;

    var resolution=vec2<f32>(1600.,1200.);

    AR=resolution.x/resolution.y;

    var coords=in_coords;//(in_coords+1.)/2.;//vec2<f32>(in_coords.x,in_coords.y*resolution.y);

    uv = (coords );
    vv = (2.0 - min((1.0 % 10.0), 2.0));
    let _e46 = vv;
    let _e52 = vv;
    fade = max(pow(_e52, 16.0), 1.0);
    let _e58 = uv;
    let _e62 = uv;
    let _e67 = corner_harshness;
    xx = (abs((_e62.x - 0.5)) * _e67);
    let _e70 = uv;
    let _e74 = uv;
    let _e79 = corner_harshness;
    let _e81 = fade;
    yy = ((abs((_e74.y - 0.5)) * _e79) * _e81);

    rr = (1.0 + pow(((xx * xx) + (yy * yy)), corner_ease));

    tuv = (((uv - vec2<f32>(0.5)) * rr) + vec2<f32>(0.5));
    tuv = clamp(tuv, vec2<f32>(0.0), vec2<f32>(1.0));
    let _e123 = tuv;
    uv = _e123;
    let _e131 = uv;
    let _e137 = uv;
    limit = (step(vec2<f32>(0.0, 0.0), _e131) * step(_e137, vec2<f32>(1.0, 1.0)));
    let _e144 = uv;
    let _e148 = uv;
    let _e153 = uv;
    let _e158 = uv;
    
            res2_ = vec2<f32>(res, (res / AR));
            let _e171 = res2_;
            res3_ = _e171;
            let _e173 = res2_;
            res4_ = _e173;
            let _e175 = uv;
            let _e176 = res2_;
            let _e178 = uv;
            let _e179 = res2_;
            pre_i = floor((_e178 * _e179));
            let _e183 = pre_i;
            let _e184 = res2_;
            i = (_e183 / _e184);
            let _e189 = pre_i;
            let _e192 = pre_i;
            if (((_e192.y % 2.0) == 0.0)) {
                {
                    let _e199 = res2_;
                    even = (0.5 / _e199.x);
                    let _e203 = uv;
                    let _e205 = even;
                    uv.x = (_e203.x + _e205);
                    let _e207 = uv;
                    let _e209 = res2_;
                    let _e212 = uv;
                    let _e214 = res2_;
                    pre_y = floor((_e212.x * _e214.x));
                    let _e220 = pre_y;
                    let _e221 = res2_;
                    i.y = (_e220 / _e221.x);
                    let _e224 = uv;
                    let _e225 = res2_;
                    let _e227 = uv;
                    let _e228 = res2_;
                    let _e231 = res2_;
                    i = (floor((_e227 * _e228)) / _e231);
                }
            }
            
            //MARK 
            tex= textureSample(t_diffuse,s_diffuse, i);
            lum = (((0.2125999927520752 * tex.x) + (0.7152000069618225 * tex.y)) + (0.0722000002861023 * tex.z));
         
            value = smoothStep(0.05, 0.6, (1.0 - lum));
            v = min(value, 1.0);
            let _e273 = uv;
            let _e282 = uv;
            let _e296 = uv;
            let _e305 = uv;
            tiny = cos(((6.28 * (((_e305.y + (iTime* 0.1)) * 0.2) % 0.01)) * 300.0));
            let _e322 = uv;
            let _e330 = uv;
            L = (0.0 + (0.01 * cos(((_e330.x * 1.200) + (iTime * 20.0)))));


            let _e364 = i;
            let _e366 = L;
            let _e367 = L;
            wave = (cos((6.28000020980835 * smoothStep(_e364.y, _e366, (_e367 + 0.05000000074505806)))) / 5.0);
            let _e379 = wave;
            let _e382 = tiny;
            let _e388 = wave;
            let _e391 = tiny;
            scan = cos((1.5700000524520874 + ((3.140000104904175 * (0.20000000298023224 - _e388)) * _e391)));
            let _e399 = v;
            split = max(0.33000001311302185, _e399);
            let _e402 = uv;
            let _e403 = split;
            let _e407 = res;
            uv2_ = (_e402 + vec2<f32>(((-(_e403) * 0.20000000298023224) / _e407), 0.0));
            let _e413 = uv;
            let _e414 = split;
            let _e418 = res;
            uv3_ = (_e413 + vec2<f32>(((-(_e414) * 0.4000000059604645) / _e418), 0.0));
            let _e425 = uv2_;
            let _e428 = scan;
            uv2_.x = (_e425.x + ((3.0 * _e428) / 240.0));
            let _e434 = uv3_;
            let _e437 = scan;
            uv3_.y = (_e434.y - ((3.0 * _e437) / 240.0));
            let _e444 = res2_;
            let _e447 = uv;
            let _e449 = res2_;
            let _e453 = res2_;
            cr = ((_e447 % (vec2<f32>(1.0) / _e449)) * _e453);
            let _e458 = res3_;
            let _e461 = uv2_;
            let _e463 = res3_;
            let _e467 = res3_;
            cg = ((_e461 % (vec2<f32>(1.0) / _e463)) * _e467);
            let _e472 = res4_;
            let _e475 = uv3_;
            let _e477 = res4_;
            let _e481 = res4_;
            cb = ((_e475 % (vec2<f32>(1.0) / _e477)) * _e481);
            let _e484 = uv2_;
            let _e485 = res3_;
            let _e487 = uv2_;
            let _e488 = res3_;
            let _e491 = res3_;
            i2_ = (floor((_e487 * _e488)) / _e491);
            let _e494 = uv3_;
            let _e495 = res4_;
            let _e497 = uv3_;
            let _e498 = res4_;
            let _e501 = res4_;
            i3_ = (floor((_e497 * _e498)) / _e501);

            tex2_ = textureSample(texture,samp, i2_);
            tex3_ = textureSample(texture,samp, i3_);

            pixel_size = (((0.4000000059604645 + 1.0) - split) * 2.0);
            cr.x = (cr.x * (0.6600000262260437 + split));
            cg.x = (cg.x * (0.6600000262260437 + split));
            cb.x = (cb.x * (0.6600000262260437 + split));
            if ((split > 0.2)) {
                {
                    ar = (((0.5 - abs((cr.x - 0.5))) * (0.5 - abs((cr.y - 0.5)))) * pixel_size);
                    ag = (((0.5 - abs((cg.x - 0.5))) * (0.5 - abs((cg.y - 0.5)))) * pixel_size);
                    ab = (((0.5 - abs((cb.x - 0.5))) * (0.5 - abs((cb.y - 0.5)))) * pixel_size);
                }
            }
         
            ar = min(floor((ar + 0.9700000286102295)), 1.0);
            ag = min(floor((ag + 0.9700000286102295)), 1.0);
            ab = min(floor((ab + 0.9700000286102295)), 1.0);
            col = vec3<f32>((tex.x * ar), (tex2_.y * ag), (tex3_.z * ab));
            //,col.z, col.y, col.x,
            output = vec4<f32>(col.x,col.y,col.z,1.);
        
    
    if (((((uv.x > 0.0) && (uv.x < 1.0)) && (uv.y > 0.0)) && (uv.y < 1.0))) {
        {
    return output;
        }
    }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        
}

[[stage(fragment)]]
fn post_fs_main(in: GuiFrag) ->  [[location(0)]] vec4<f32> {
    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);


    //f_color=textureSample(t_diffuse, s_diffuse, p);

    f_color=monitor(t_diffuse, s_diffuse,p,vec2<f32>(in.pos.x,in.pos.y));
   return f_color;//vec4<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y, 0., 1.0);
}