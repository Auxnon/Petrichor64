struct VertexOutput {
    @builtin(position) proj_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) vpos:vec4<f32>,
};

struct InstanceInput {
    @location(4) uv_mod: vec4<f32>,
    @location(5) color: vec4<f32>,
    @location(6) effects: vec4<f32>,
    @location(7) model_matrix_0: vec4<f32>,
    @location(8) model_matrix_1: vec4<f32>,
    @location(9) model_matrix_2: vec4<f32>,
    @location(10) model_matrix_3: vec4<f32>,
};


struct Globals {
    view_mat: mat4x4<f32>,
    proj_mat: mat4x4<f32>,
    adjustments: mat4x4<f32>,
    //num_lights: vec4<u32>,
};


@group(0)
@binding(0)
var<uniform> globals: Globals;
@group(0) 
@binding(1)
var t_diffuse: texture_2d<f32>;
@group(0)
@binding(2)
var s_diffuse: sampler;


// struct Entity {
//     matrix: mat4x4<f32>;
//     color: vec4<f32>;
//     uv_mod:vec4<f32>;
//     effects:vec4<u32>;
// };

// @group(1), binding(0)
// var<uniform> ent: Entity;

@vertex
fn vs_main(
    @location(0) position: vec4<i32>,
    @location(1) normal: vec4<i32>,
    @location(2) tex_coords: vec2<f32>,

    
    instance: InstanceInput
) -> VertexOutput {

    let billboarded=false;
    // let tex_coords=instance.uv;
    // let w = ent.matrix;
    let w=mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let world_pos =  w *vec4<f32>(position); 
    var out: VertexOutput;
    out.world_normal = mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * vec3<f32>(normal.xyz);
    out.world_position = world_pos;
    let v=globals.view_mat;


    //globals.view_proj
    let pos=vec4<f32>(position);
    //    out.proj_position = globals.view_proj * world_pos;

    let bb=instance.effects[0];
    //billboard if true
    if(bb> 0.){
        // let billbo=mat4x4<f32>(
        //     vec4<f32>(bb,0.,0.,0.),
        //     vec4<f32>(0.,bb,0.,0.),
        //     vec4<f32>(0.,0.,bb,0.),
        //     vec4<f32>(0.,0.,0.,1.),
        // );
        // let sq=sqrt(2.)/2.;
        let r=instance.effects[1];
        let roo=mat4x4<f32>(
            vec4<f32>(cos(r),-sin(r),0.,0.),
            vec4<f32>(sin(r),cos(r),0.,0.),
            vec4<f32>(0.,0.,1.,0.),
            vec4<f32>(0.,0.,0.,1.),
        );

        // out.proj_position=globals.proj_mat*(globals.view_mat*billbo*w*vec4<f32>(1.,1.,1.,1.)+ pos);
    out.proj_position=globals.proj_mat*(globals.view_mat*w*vec4<f32>(0.,0.,0.,1./bb)+roo*vec4<f32>(pos.x,pos.y,0.,0.));
    }else{
        out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);
    }
    let uv_mod=instance.uv_mod;

    out.tex_coords=(tex_coords*vec2<f32>(uv_mod.z,uv_mod.w))+vec2<f32>(uv_mod.x,uv_mod.y);
    let vpos:vec4<f32>=out.proj_position;
  
    out.vpos=vec4<f32>(world_pos.x,world_pos.y,world_pos.z+globals.adjustments[0][0],world_pos.w);
    return out;
}

struct GuiFrag {
    @builtin(position) pos: vec4<f32>, 
    @location(1) screen: vec2<f32>,
    // @location(2) eh: array<f32>,
    // @location(2) adjustments: array<f32,12>,
};

struct PostFrag {
    @builtin(position) pos: vec4<f32>, 
    @location(1) screen: vec2<f32>,
    // @location(2) adjustments: array<f32,12>,
};


@vertex
fn gui_vs_main(@builtin(vertex_index) in_vertex_index: u32) ->GuiFrag{
    // @location(0) position: vec4<i32>,
    // @location(1) normal: vec4<i32>,
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
    
    out.screen=vec2<f32>(globals.adjustments[0][1],globals.adjustments[0][2]);
    // out.eh=vec2<f32>(globals.adjustments[3],globals.adjustments[4]);
    //out.adjustments=array<f32,12>(0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.);
    return out;
}

struct FragmentOutput {
    @location(0) f_color: vec4<f32>,
};


var<private> f_color: vec4<f32>;

fn main_2(in:VertexOutput) {
    //f_color = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    let v=abs((10.*in.vpos.z+0.01)/10.)%1.;
    
    f_color=textureSample(t_diffuse, s_diffuse, in.tex_coords);//vec4<f32>(abs(in.vpos.y)%1.,1.,1.,1.0);
    //f_color=f_color*v; //vec4<f32>(v,v,v,1.0);
    return;
}

@fragment
fn fs_main( in: VertexOutput) -> FragmentOutput {
    main_2(in); 
    let e3: vec4<f32> = f_color;
    if (e3.a < 0.5) {
        discard;
    }
    return FragmentOutput(e3);
}

@fragment
fn gui_fs_main(in: GuiFrag) ->  @location(0) vec4<f32> {
  
    // let e3: vec4<f32> = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    // if (e3.a < 0.5) {
    //     discard;
    // }
    //return FragmentOutput(e3);
    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);
    f_color=textureSample(t_diffuse, s_diffuse, p);
   return f_color;//vec4<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y, 0., 1.0);
}

@vertex
fn sky_vs_main(@builtin(vertex_index) in_vertex_index: u32) ->GuiFrag{
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
    out.screen=vec2<f32>(globals.adjustments[0][1],globals.adjustments[0][2]);
    return out;
}

@fragment
fn sky_fs_main(in: GuiFrag) ->  @location(0) vec4<f32> {
    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);
    f_color=textureSample(t_diffuse, s_diffuse, p);
   return f_color;
}


@vertex
fn post_vs_main(@builtin(vertex_index) in_vertex_index: u32) ->GuiFrag{
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

    //out.adjustments=array<f32,12>(globals.adjustments[0],globals.adjustments[1],globals.adjustments[2],globals.adjustments[3],globals.adjustments[4],globals.adjustments[5],globals.adjustments[6],globals.adjustments[7],globals.adjustments[8],globals.adjustments[9],globals.adjustments[10],globals.adjustments[11]);
     //globals.adjustments;
    //  out.adjustments=globals.adjustments;
    //  out.adjustments=array<f32,12>(0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.);//globals.adjustments;
    out.screen=vec2<f32>(globals.adjustments[0][1],globals.adjustments[0][2]);
    

    return out;
}

fn findSplit(uv: vec2<f32>, res2: vec2<f32>, offset: vec2<f32>,low_range:f32,high_range:f32,t:texture_2d<f32>,s:sampler) -> vec4<f32> {
    let i: vec2<f32> =(offset+floor(uv*res2))/res2;
    let tex= textureSampleLevel(t,s, i,0.);
    if(i.x<0. || i.x>1.){
    return vec4<f32>(0.,0.,0.,1.);
    }
    
    let lum:f32=(0.2126*tex.r + 0.7152*tex.g + 0.0722*tex.b);
    let value=smoothstep(low_range,high_range,1.-lum);
    let v=min(value,1.);
    
    //first factor to determine how much rgb pixels split up
    //1. is complete seperation, 0. is merged
        let split=max(.33,v);
    return vec4<f32>(tex.xyz,split);
}
type vec2f = vec2<f32>;
type vec3f = vec3<f32>;

fn path(uv:vec2f, res:vec2f, mask:vec3f, shift:vec2f,low_range:f32,high_range:f32,dark:f32,lumen:f32,t:texture_2d<f32>,s:sampler)->vec2f{
    let v=findSplit(uv,res,vec2f(0.,0.),low_range,high_range,t,s);
    let vl=findSplit(uv,res,vec2f(-1.,0.),low_range,high_range,t,s);
    let vr=findSplit(uv,res,vec2f(1.,0.),low_range,high_range,t,s);
    let split=v.w;
    let split_l=vl.w;
    let split_r=vr.w;
    
    let c:vec2f=(uv+shift)%(1./res)*res;
    
    var full=mask.x*v.x+mask.y*v.y+mask.z*v.z;
    
    var total_split=split;
    if(c.x>0.75){
        let f=(1.-(c.x- 0.75)/.5);
        total_split=split*f+split_r*(1.-f);
        let side=mask.x*vr.x+mask.y*vr.y+mask.z*vr.z;
        full=full*f+side*(1.-f);
    }else if(c.x<0.25){
        let f=(c.x/.5)+0.5;
        //total_split=smoothstep(split_l,split,);
        let side=mask.x*vl.x+mask.y*vl.y+mask.z*vl.z;
        total_split=split*(f)+split_l*(1.-f);
        full=full*f+side*(1.-f);
    }
    
    let pixel_size=(dark+1.-total_split)*1. ;
    
    var a=1.;
    if(total_split>lumen){
     a=(0.5- abs(c.y- 0.5))*pixel_size*.2; //.2
    }
    a*=16.;
    return vec2(a,full);     
}




// 0:iTime, 1:native_res.0,2:native_res.1, 3:res,4:corner_harshness,5:corner_ease,6:glitchy,7:lumen_threshold,8:dark,9:low,10:high
// native_res, res,corner_harshness,corner_ease, glitchy,lumen_threshold,dark,low,high
fn monitor(texture:texture_2d<f32>,samp:sampler,in_coords:vec2<f32>,adj:mat4x4<f32>)-> vec4<f32>  {
    let iTime=adj[0][0]; 
    let dark_factor:f32=adj[2][0]; //0.4
    let low_range:f32=adj[2][1]; //.05
    let high_range:f32=adj[2][2]; //0.6
    let resolution=vec2<f32>(adj[0][1],adj[0][2]);
    let corner_harshness: f32 =adj[1][0]; // 1.2
    let corner_ease: f32 = adj[1][1]; // 4.0
    let resi: f32 =adj[0][3]; //  320.0
    let glitchy: f32 =adj[1][2]; // 3.0  
    let lumen_threshold:f32=adj[1][3]; //0.2


    var uv: vec2<f32>;
    var output: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    
    var vv: f32;
    var fade: f32;
    var xx: f32;
    var yy: f32;
    var rr: f32;
    var tuv: vec2<f32>;
    var limit: vec2<f32>;
    
  


    var AR=resolution.x/resolution.y;

    var coords=in_coords;//(in_coords+1.)/2.;//vec2<f32>(in_coords.x,in_coords.y*resolution.y);

    var uv:vec2<f32> = (coords );
    let vv:f32 = (2.0 - min(((iTime) ), 2.0)); //%10.

   let fade = max(pow(vv, 16.0), 1.0);
    let xx:f32 = (abs(uv.x - 0.5)*corner_harshness);
    var yy:f32  = (abs(uv.y - 0.5)*corner_harshness);
    var rr:f32=(1.+pow((xx*xx+yy*yy),corner_ease));
    var tuv:vec2<f32> =clamp((uv-vec2(0.5))*rr+0.5,vec2(0.),vec2(1.));
    uv=tuv;


     //========END=========================
    
    if(  uv.x>0. && uv.x<1. && uv.y>0. && uv.y<1.){
    
        //===== START additional curvature for glass to allow fade in out but keep glass background
        yy=(abs(uv.y - 0.5)*corner_harshness)*fade;
        rr=(1.+pow((xx*xx+yy*yy),corner_ease));
        tuv=(uv-vec2(0.5))*rr+0.5;
        tuv=clamp(tuv,vec2(0.),vec2(1.));
        uv=tuv;
        //===END==========================
        
        if(  uv.x>0. && uv.x<1. && uv.y>0. && uv.y<1.){
            
            //flicker
            uv+=sin(min((iTime%1.),2.)*2000.)/10000.;

            //resolution factor
            let res=min(resi,resolution.x);
            let res2=vec2<f32>(res,res/AR);
            let res3=res2;
            let res4=res2;
            
            
            let shift=1./res;

            let i=floor(uv*res2)/res2;
            let tex = textureSampleLevel(t_diffuse,s_diffuse, i,0.);
            let lum=(0.2126*tex.r + 0.7152*tex.g + 0.0722*tex.b);
            let value=smoothstep(low_range,high_range,1.-lum);
            let v=min(value,1.);


            //first factor to determine how much rgb pixels split up
            //1. is complete seperation, 0. is merged
            let split=max(.33,v);
            
            ////===== START scan lines
            let L=0.01*cos(uv.x*1.2+iTime*20.);
            let wave=cos(6.28*smoothstep(i.y,L,L+0.05))/5.;
            
           
            let scanny=cos(1.57+3.14*(.2-wave));
            let vvv=2.*scanny*cos(uv.x*16.+iTime*16.)/res;
            //========== END

           let r=path(uv,res2,vec3f(1.,0.,0.),vec2f(0.,vvv),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
           let red=r.y;
           let ar=r.x;

            let uv2=uv;
            let uv3=uv;
            
            let g=path(uv2,res3,vec3f(0.,1.,0.),vec2f(0.,0.),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
            let b=path(uv3,res4,vec3f(0.,0.,1.),vec2f(0.,-vvv),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
            
            let ag=g.x;
            let green=g.y;
            let ab=b.x;
            let blue=b.y;
             
            // Time varying pixel color
            let col = vec3<f32>(red*ar,green*ag,blue*ab); 

            return vec4<f32>(col,1.);
          
        }else{
            let tex= textureSampleLevel(t_diffuse,s_diffuse, vec2f(0.,0.),0.);
           return vec4<f32>(0.,0.,0.,1.);
        }
    }else{
        let tex=textureSampleLevel(t_diffuse,s_diffuse, vec2f(0.,0.),0.);
        let vx=abs(uv.x- 0.5);
        let vy=abs(uv.y- 0.5);
        let r=min(abs(vx- vy),0.25);
        return vec4<f32>(r,r,r,1.);
    }
    // return vec4<f32>(0.,0.,0.,1.);
        
}

@fragment
fn post_fs_main(in: GuiFrag) ->  @location(0) vec4<f32> {
    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);

    if (globals.adjustments[2][3]>0.){
        f_color=textureSample(t_diffuse, s_diffuse, p);
    }else{
        f_color=monitor(t_diffuse, s_diffuse,p,globals.adjustments);//array<f32,12>(0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.0));
    }
    //f_color=textureSample(t_dif,fuse, s_diffuse, p);
// globals.adjustments
   return f_color;//vec4<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y, 0., 1.0);
}