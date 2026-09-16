struct VertexOutput {
	@builtin(position) proj_position: vec4<f32>,
	@location(0) world_normal: vec3<f32>,
	@location(1) world_position: vec4<f32>,
	@location(2) tex_coords: vec2<f32>,
	@location(3) vpos:vec4<f32>,
	@location(4) specs:vec4<f32>,
	@location(5) time:f32,
	// Screen-space linear (not perspective-correct) copy of tex_coords, for the
	// R30 chip's affine texture mapping. Always computed; fs_main only uses it
	// when vertex_snap (adjustments[3][3]) is >0, since only R30 wants it.
	@location(6) @interpolate(linear) tex_coords_affine: vec2<f32>,
	// Vertex-colour tint (guide/entity.md), passed through unchanged.
	@location(7) tint: vec4<f32>,
	// Gouraud (guide/gour.md): per-vertex shade, used instead of fs_main's own
	// per-fragment computation when gour() is on. Always computed — cheap
	// relative to per-fragment, and this way there's no shader permutation.
	@location(8) shade: vec3<f32>,
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
	// L0 retro lighting: directional sun. light_color.w = ambient.
	light_dir: vec4<f32>,
	light_color: vec4<f32>,
	// L2 distance fog: rgb + w = far distance (w=0 disables).
	fog_color: vec4<f32>,
	// L2 hemisphere ambient: sky rgb (w>0 enables) + ground rgb. Sun shape only.
	amb_sky: vec4<f32>,
	amb_ground: vec4<f32>,
	// lum{} shape state: xyz = world pos (cone/sphere), w = falloff range.
	light_pos: vec4<f32>,
	// x = shape (0=sun,1=cone,2=sphere), y = cone half-angle (radians).
	light_shape: vec4<f32>,
	// Light-space view*proj for the shadow map (see monitor()/fs_main), and
	// x = shdw() on/off.
	light_view_proj: mat4x4<f32>,
	shadow_on: vec4<f32>,
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

// The shadow map (see guide/shdw.md) — a small, deliberately low-resolution
// depth texture rendered from the lum{} light's point of view by
// shadow_vs_main, sampled here with a hardware comparison sampler (one tap,
// no PCF — matches the "primitive" brief).
@group(2)
@binding(0)
var t_shadow: texture_depth_2d;
@group(2)
@binding(1)
var s_shadow: sampler_comparison;

// The one lum{} light: 0=sun (directional, infinite), 1=cone (spot), 2=sphere
// (point). shade = ambient + max(dot(N,-L),0) * color * atten. Defaults
// (color=0, ambient=1, shape=sun) leave the scene fullbright/unchanged.
// Shared by fs_main (per-fragment, default) and vs_main (per-vertex, when
// gour() is on — see guide/gour.md) so there's one copy of this math, not two
// drifting apart.
fn compute_shade(norm: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
	let light_shape = globals.light_shape.x;
	var ndl: f32;
	var atten = 1.0;
	// Ambient: flat scalar, or (sun only) L2 hemisphere by N.z.
	var ambient = vec3<f32>(globals.light_color.w);
	if (light_shape < 0.5) {
		let ldir = normalize(globals.light_dir.xyz);
		ndl = max(dot(norm, -ldir), 0.0);
		if (globals.amb_sky.w > 0.) {
			ambient = mix(globals.amb_ground.rgb, globals.amb_sky.rgb, norm.z * 0.5 + 0.5);
		}
	} else {
		// Cone/sphere: a real position, falls off with distance.
		let to_light = globals.light_pos.xyz - world_pos;
		let dist = length(to_light);
		let ldir = to_light / max(dist, 0.0001);
		ndl = max(dot(norm, ldir), 0.0);
		let range = max(globals.light_pos.w, 0.0001);
		atten = clamp(1.0 - dist / range, 0.0, 1.0);
		if (light_shape < 1.5) {
			// Cone only: angular falloff from the aim direction, with a
			// soft edge (smoothstep) rather than a hard cutoff.
			let aim = normalize(globals.light_dir.xyz);
			let cos_angle = cos(globals.light_shape.y);
			let spot = smoothstep(cos_angle, mix(cos_angle, 1.0, 0.2), dot(-ldir, aim));
			atten = atten * spot;
		}
	}
	return ambient + ndl * atten * globals.light_color.rgb;
}

// Depth-only pass for the shadow map: same vertex data and model matrix as
// vs_main, projected through the light's view*proj instead of the camera's.
// Billboard rotation is skipped (shadows use the entity's base transform) —
// a reasonable simplification for a primitive/blocky shadow system.
@vertex
fn shadow_vs_main(
	@location(0) position: vec4<i32>,
	@location(1) normal: vec4<i32>,
	@location(2) tex_coords: vec2<f32>,
	instance: InstanceInput
) -> @builtin(position) vec4<f32> {
	let w=mat4x4<f32>(
		instance.model_matrix_0,
		instance.model_matrix_1,
		instance.model_matrix_2,
		instance.model_matrix_3,
	);
	let world_pos = w * vec4<f32>(position);
	return globals.light_view_proj * world_pos;
}

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

	out.world_normal =  mat3x3<f32>(w[0].xyz, w[1].xyz, w[2].xyz) * vec3<f32>(0.,1.,0.);
		// out.proj_position=globals.proj_mat*(globals.view_mat*billbo*w*vec4<f32>(1.,1.,1.,1.)+ pos);
	out.proj_position=globals.proj_mat*(globals.view_mat*w*vec4<f32>(0.,0.,0.,1./bb)+roo*vec4<f32>(pos.x,pos.y,0.,0.));
	}else{
	out.world_normal = mat3x3<f32>(w[0].xyz, w[1].xyz, w[2].xyz) * (vec3<f32>(normal.xyz)/100.);
		out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);
	}
	let uv_mod=instance.uv_mod;

	let vpos:vec4<f32>=out.proj_position;
	out.vpos=vec4<f32>((world_pos.x),(world_pos.y),(world_pos.z+globals.adjustments[0][0]),world_pos.w);
	// R30 chip: clip-space vertex snap (the PS1 "wobble" — no subpixel precision).
	// snap is also the flag fs_main uses to gate affine UV mixing and dithering,
	// since only R30 wants any of the three; see ScreenBinds::vertex_snap.
	let snap=globals.adjustments[3][3];
	if(snap>0.){
		out.proj_position=vec4<f32>(round(out.proj_position.xyz/snap)*snap,out.proj_position.w);
	}

	// let ntex=vec2<f32>(tex_coords.x*out.vpos.w,tex_coords.y*out.vpos.w);
	out.tex_coords=(tex_coords*vec2<f32>(uv_mod.z,uv_mod.w))+vec2<f32>(uv_mod.x,uv_mod.y);
	out.tex_coords_affine=out.tex_coords;
	out.specs=globals.specs;
	out.time=globals.adjustments[0][0];
	out.tint=instance.color;
	// Gouraud (guide/gour.md): always computed, cheap relative to
	// per-fragment — fs_main picks this or its own per-fragment shade based
	// on gour()'s toggle (shadow_on.y), no shader permutation needed.
	out.shade=compute_shade(out.world_normal, world_pos.xyz);
	// FragPos = vec3(model * vec4(aPos, 1.0));
	// out.frag_pos=vec3<f32>(world_pos.x,world_pos.y,world_pos.z,1.);
	return out;
}




var<private> f_color: vec4<f32>;

// R30 chip: 4x4 ordered (Bayer) dither matrix, used to fake PS1-style ~15-bit
// color depth without a true post-process pass — see fs_main.
const BAYER4: array<f32,16> = array<f32,16>(
	0.,8.,2.,10.,
	12.,4.,14.,6.,
	3.,11.,1.,9.,
	15.,7.,13.,5.,
);

@fragment
fn fs_main( in: VertexOutput) -> FragmentOutput {
	//f_color = vec4<f32>(0.10000001192092896, 0.20000000298023224, 0.10000000149011612, 1.0);
	// let v=abs((10.*in.vpos.z+0.01)/10.)%1.;
	// let v=abs((10.*in.vpos.z+0.01)/10.)%3.;
	// let v=1.-min(1.,length(in.world_position.xyz - in.specs.xyz)%10.);

	// let start=120.;

	// in.world_position.xyz

	let mutator=1.;//in.proj_position.w;
	// Gouraud (guide/gour.md): gour()'s toggle (shadow_on.y) picks the
	// per-vertex shade vs_main already computed, or recomputes it per-
	// fragment (default, "smooth") — same compute_shade either way, so the
	// two paths can never drift apart.
	let norm = normalize(in.world_normal);
	var shade = in.shade;
	if (globals.shadow_on.y < 0.5) {
		shade = compute_shade(norm, in.world_position.xyz);
	}

	// Shadow map (guide/shdw.md): one hard comparison tap, no PCF — off by
	// default (shadow_on.x, set by shdw()) and a no-op outside the light's
	// small frustum (lit=1, not shadowed, rather than clamping artifacts).
	// Always per-fragment (stays sharp even with Gouraud shading on).
	var lit = 1.0;
	if (globals.shadow_on.x > 0.5) {
		let light_clip = globals.light_view_proj * in.world_position;
		let light_ndc = light_clip.xyz / light_clip.w;
		let shadow_uv = light_ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
		if (shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 && light_ndc.z >= 0.0 && light_ndc.z <= 1.0) {
			lit = textureSampleCompare(t_shadow, s_shadow, shadow_uv, light_ndc.z - 0.002);
		}
	}
	shade = shade * lit;

	// R30 chip: snap>0 also selects affine (screen-space linear) UVs instead of
	// perspective-correct ones — the PS1's characteristic texture warp.
	let snap=globals.adjustments[3][3];
	let uv=mix(in.tex_coords,in.tex_coords_affine,select(0.,1.,snap>0.));
	f_color=textureSample(t_diffuse, s_diffuse, uv*mutator);//vec4<f32>(abs(in.vpos.y)%1.,1.,1.,1.0);

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

	// Vertex-colour tint (guide/entity.md): {1,1,1,1} default is a no-op.
	var rgb = e3.rgb * shade * in.tint.rgb;
	// L2 distance fog: blend toward fog rgb as the fragment approaches the fog
	// far distance (fog_color.w). specs.xyz is the camera's world position.
	if (globals.fog_color.w > 0.) {
		let fog_t = clamp(length(in.world_position.xyz - in.specs.xyz) / globals.fog_color.w, 0., 1.);
		rgb = mix(rgb, globals.fog_color.rgb, fog_t);
	}

	// R30 chip: ordered-dither down to ~5 bits/channel (PS1-era color depth).
	if(snap>0.){
		let levels=31.;
		let bidx=(i32(in.proj_position.x)%4)+(i32(in.proj_position.y)%4)*4;
		let bayer=BAYER4[bidx]/16.-0.5;
		rgb=clamp(rgb+bayer/levels,vec3<f32>(0.),vec3<f32>(1.));
		rgb=floor(rgb*levels+0.5)/levels;
	}

	return FragmentOutput(vec4<f32>(rgb, e3.a));
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
alias vec2f = vec2<f32>;
alias vec3f = vec3<f32>;

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
