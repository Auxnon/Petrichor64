#define RESOLUTION 240.
#define LOW_RANGE 0.05
#define HIGH_RANGE 0.6
#define DARK_FACTOR 0.4
#define LUMEN_THRESHOLD 0.2
#define GLITCHY 3.
#define iResolution vec2(100., 100.)
#define iTime 1.
#define fragCoord vec2(0., 0.)


// layout(location = 0) uniform vec3 iResolution;
// layout(location=1) uniform float iTime;
// layout(location=2) uniform sampler2D iChannel0;
void main()
{
    float AR = iResolution.x / iResolution.y;
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = fragCoord/iResolution;
    vec4 output=vec4(0.,0.,0.,1.);
    
    
    const float corner_harshness=1.2;
    const float corner_ease=4.;
    
    float vv=2.-min(mod(iTime,10.),2.);
    //float vv=cos(iTime*1.)*2.;
    float fade=max(pow(vv,16.),1.);
    
    float xx=(abs(uv.x -0.5)*corner_harshness);
    float yy=(abs(uv.y -0.5)*corner_harshness)*fade;
    float rr=(1.+pow((xx*xx+yy*yy),corner_ease));
    vec2 tuv=(uv-0.5)*rr+0.5;
    tuv=clamp(tuv,0.,1.);
    //if(rr<1.01){
    
    uv=tuv;
    vec2 limit=step(vec2(0.,.0),uv)*step(uv,vec2(1.,1.));
    if(  uv.x>0. && uv.x<1. && uv.y>0. && uv.y<1.){

    
    //resolution factor
    float res=RESOLUTION;
    vec2 res2=vec2(res,res/AR);
    vec2 res3=res2;
    vec2 res4=res2;

    
    vec2 pre_i=floor(uv*res2);
    
    vec2 i=pre_i/res2;
    
    float even=0.;
    
    if(mod(pre_i.y,2.)==0.){
        even=0.5/res2.x;//2./res;

        uv.x+=even;
        float pre_y=floor(uv.x*res2.x);
        i.y=pre_y/res2.x;
        
        i=floor(uv*res2)/res2;
    }
    
    vec4 tex = vec4(1.,1.,1.,1.); //texture(iChannel0, i);
    float lum=(0.2126*tex.r + 0.7152*tex.g + 0.0722*tex.b);
    
    //this calculation is very particular
    float value=smoothstep(LOW_RANGE,HIGH_RANGE,1.-lum);
    
    float v=min(value,1.);
    
    
    //// scan lines?
    float tiny=cos(6.28*mod((uv.y+iTime*.1)*.2,.01)*300.);
    float L=0. +0.01*cos(uv.x*1.2+iTime*20.);
    float wave=cos(6.28*smoothstep(i.y,L,L+0.05))/5.;
    float scan=cos(1.57+3.14*(.2-wave)*tiny);
    ////
    
    
    //first factor to determine how much rgb pixels split up
    //1. is complete seperation, 0. is merged
    float split=max(.33,v);
    
    

 
    
    vec2 uv2=uv+vec2(-split*0.2/res,0.);
    
    vec2 uv3=uv+vec2(-split*0.4/res,0.);
    
    

    uv2.x+=GLITCHY*scan/RESOLUTION;
    uv3.y-=GLITCHY*scan/RESOLUTION;

    
    
    //vec2 c=mod(uv,1./res)*res;
    vec2 cr=mod(uv,1./res2)*res2;
    vec2 cg=mod(uv2,1./res3)*res3;
    vec2 cb=mod(uv3,1./res4)*res4;

    
    vec2 i2=floor(uv2*res3)/res3;//vec2(i.x+t*3.,i.y-t);
    vec2 i3=floor(uv3*res4)/res4;//vec2(i.x-t*6.,i.y+t);

    
    vec4 tex2 = vec4(1.,1.,1.,1.);//texture(iChannel0, i2);
    vec4 tex3 = vec4(1.,1.,1.,1.);//texture(iChannel0, i3);

  
    //darkness factor (between 0 and 1 or higher, higher means dark barely splits
    
    float ar=1.;
    float ag=1.;
    float ab=1.;
    
    
    
    float pixel_size=(DARK_FACTOR+1.-split)*2. ;
    

    cr.x*=0.66+split;
    cg.x*=0.66+split;
    cb.x*=0.66+split;
    
    if(split>LUMEN_THRESHOLD){
     ar=(0.5-abs(cr.x-.5))*(0.5-abs(cr.y-0.5))*pixel_size;
     ag=(0.5-abs(cg.x-.5))*(0.5-abs(cg.y-0.5))*pixel_size;
     ab=(0.5-abs(cb.x-.5))*(0.5-abs(cb.y-0.5))*pixel_size;
        
     }
    
    ar=min(floor(ar+0.97),1.0);
    ag=min(floor(ag+0.97),1.0);
    ab=min(floor(ab+0.97),1.0);
    
    

    // Time varying pixel color
    vec3 col = vec3(tex.r*ar,tex2.g*ag,tex3.b*ab); //vec3(r);//
    
    vec3 backup=vec3(tex.r,tex2.g,tex3.b);
    // Output to screen
    output = vec4(col,1.);//vec4(cos(i2.x*100.),1.0,1.,1.0);
    }
    
    
}