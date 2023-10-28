struct VertexOutput {
    @builtin(position) proj_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) vpos:vec4<f32>,
    @location(4) specs:vec4<f32>,
    @location(5) time:f32,
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
    specs: vec4<f32>,
    //num_lights: vec4<u32>,
};

struct GuiFrag {
    @builtin(position) pos: vec4<f32>, 
    @location(1) screen: vec4<f32>,
    // @location(2) eh: array<f32>,
    // @location(2) adjustments: array<f32,12>,
};

struct FragmentOutput {
    @location(0) f_color: vec4<f32>,
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

@group(1) 
@binding(0)
var primary: texture_2d<f32>;
@group(1) 
@binding(1)
var secondary: texture_2d<f32>;
@group(1) 
@binding(2)
var trinary: texture_2d<f32>;

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

    // w is our model matrix
    let w=mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    var world_pos =  w *vec4<f32>(position); 
    // world_pos=round(world_pos/2.)*2.;
    var out: VertexOutput;
    out.world_position = world_pos;
    let v=globals.view_mat;


    let pos=vec4<f32>(position);

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

    out.world_normal =  mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * vec3<f32>(0.,1.,0.);
        // out.proj_position=globals.proj_mat*(globals.view_mat*billbo*w*vec4<f32>(1.,1.,1.,1.)+ pos);
    out.proj_position=globals.proj_mat*(globals.view_mat*w*vec4<f32>(0.,0.,0.,1./bb)+roo*vec4<f32>(pos.x,pos.y,0.,0.));
    }else{
    out.world_normal = mat3x3<f32>(w.x.xyz, w.y.xyz, w.z.xyz) * (vec3<f32>(normal.xyz)/100.);
        out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);
    }
    let uv_mod=instance.uv_mod;

    let vpos:vec4<f32>=out.proj_position;
    out.vpos=vec4<f32>((world_pos.x),(world_pos.y),(world_pos.z+globals.adjustments[0][0]),world_pos.w); 
    // out.proj_position=vec4(round(out.proj_position.xyz/1.5)*1.5,out.proj_position.w); // 1.2 harsh 1.5 too harsh
    // MARK
    // out.proj_position=vec4(round(out.proj_position.xyz*2.)/2.,out.proj_position.w); // 1.2 harsh 1.5 too harsh

    // let ntex=vec2<f32>(tex_coords.x*out.vpos.w,tex_coords.y*out.vpos.w);
    out.tex_coords=(tex_coords*vec2<f32>(uv_mod.z,uv_mod.w))+vec2<f32>(uv_mod.x,uv_mod.y);
    // MARK
    // out.tex_coords= vec2<f32>(out.tex_coords.x*vpos.w,out.tex_coords.y*vpos.w);
    out.specs=globals.specs;
    out.time=globals.adjustments[0][0];
    // FragPos = vec3(model * vec4(aPos, 1.0));
    // out.frag_pos=vec3<f32>(world_pos.x,world_pos.y,world_pos.z,1.);
    return out;
}




var<private> f_color: vec4<f32>;

@fragment
fn fs_main( in: VertexOutput) -> FragmentOutput {
    //f_color = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    // let v=abs((10.*in.vpos.z+0.01)/10.)%1.;
    // let v=abs((10.*in.vpos.z+0.01)/10.)%3.;
    // let v=1.-min(1.,length(in.world_position.xyz - in.specs.xyz)%10.);

    // let start=120.;

    // in.world_position.xyz

    let mutator=1.;//in.proj_position.w;
    let t=in.time/2.;
    let light_pos = vec3<f32>(1000.0*cos(t),1000.0*sin(t), 0.0);
    let light_color=vec3<f32>(1.,1.,1.);
    let norm = normalize(in.world_normal);
    let light_dir = normalize(light_pos - in.world_position.xyz); 
    let diff = max(dot(norm, light_dir), .1);
    let diffuse = light_color;//diff *  
    // vec3 result = (ambient + diffuse) * objectColor;
// FragColor = vec4(result, 1.0);
   
    f_color=textureSample(t_diffuse, s_diffuse, in.tex_coords*mutator);//vec4<f32>(abs(in.vpos.y)%1.,1.,1.,1.0);
   
    if( in.specs.w>0.){
        let end=in.specs.w;
        let dist=length(in.world_position.xyz-in.specs.xyz);      
        // let v= clamp((end - dist) / (end - start), 0.0, 1.0);
        let v= clamp((end - dist) / (  32.), 0.0, 1.0);
        // if (v<0.5){
        // discard;
        //  }
        f_color.a*=v;
    }

    let e3: vec4<f32> = f_color;
    if (e3.a < 0.1) {
        discard;
    }

    return FragmentOutput(e3*vec4<f32>(diffuse,1.));
}

@vertex
fn gui_vs_main(@builtin(vertex_index) in_vertex_index: u32) ->GuiFrag{

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
    
    out.screen=vec4<f32>(globals.adjustments[0][1],globals.adjustments[0][2],globals.adjustments[3][0],globals.adjustments[3][1]);
    return out;
}

@fragment
fn gui_fs_main(in: GuiFrag) ->  @location(0) vec4<f32> {
  
    // let e3: vec4<f32> = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
    // if (e3.a < 0.5) {
    //     discard;
    // }
    //return FragmentOutput(e3);
    let aspect=in.screen.z/in.screen.w;
    let f=1.;//min(in.screen.x,in.screen.y);

    let p =vec2<f32>(in.pos.x/in.screen.x, in.pos.y/in.screen.y);
    let system=textureSample(t_diffuse, s_diffuse, p);
    let primary=textureSample(primary, s_diffuse, p);
    let secondary=textureSample(secondary, s_diffuse, p);
    let trinary=textureSample(trinary, s_diffuse, p);

    f_color=system;
    if (system.a<0.1){
        f_color=primary;
        if (primary.a<0.1){
            f_color=secondary;
            if (secondary.a<0.1){
                f_color=trinary;
            }
        }
    }
    
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
    out.screen=vec4<f32>(globals.adjustments[0][1],globals.adjustments[0][2], globals.adjustments[3][0],globals.adjustments[3][1]);
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
    out.screen=vec4<f32>(globals.adjustments[0][1],globals.adjustments[0][2], globals.adjustments[3][0],globals.adjustments[3][1]);
    

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
    let glitchy_line: f32 =adj[3][2]; // 0.2
    let lumen_threshold:f32=adj[1][3]; //0.2

    var output: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    
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
            uv+=sin(min((iTime%1.),2.)*2000.)/(100.+9900.*(1.-glitchy));

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
             //distance .05 .02 
            let scan_size=5.*(1.-glitchy_line); //5
            let wave=cos(smoothstep(i.y*16.,L,L+glitchy_line*20.))/scan_size;
            
           
            let scanny=cos(1.57+3.14*(.2-wave));
            let sc=glitchy*scanny*cos(uv.x*32.+iTime*12.); // is iTime*16 bad for epilepsy?
            let sc2=glitchy*sc/20.;
            let vvv=2.*sc/res; //glitchy
            //========== END

           let r=path(uv+vec2(sc2,0.),res2,vec3f(1.,0.,0.),vec2f(0.,vvv),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
           let red=r.y;
           let ar=r.x;

            let uv2=uv;
            let uv3=uv;
            
            let g=path(uv2+vec2(0.,sc2/8.),res3,vec3f(0.,1.,0.),vec2f(0.,-vvv),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
            let b=path(uv3-vec2(-sc2,0.),res4,vec3f(0.,0.,1.),vec2f(0.,0.),low_range,high_range,dark_factor,lumen_threshold,t_diffuse,s_diffuse);
            
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