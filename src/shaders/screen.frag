#define RESOLUTION 320.
#define LOW_RANGE 0.05
#define HIGH_RANGE 0.6
#define DARK_FACTOR 0.8
#define LUMEN_THRESHOLD .0
#define GLITCHY 3.
#define iResolution vec2(100., 100.)
#define iTime 1.
#define fragCoord vec2(0., 0.)


vec4 findSplit(vec2 uv, vec2 res2,vec2 offset){
    vec2 i=(offset+floor(uv*res2))/res2;
    if(i.x<0. || i.x>1.){
    return vec4(0.,0.,0.,1.);
    }
    vec4 tex = vec4(1.,1.,1.,1.);//texture(t_diffuse, i);
    float lum=(0.2126*tex.r + 0.7152*tex.g + 0.0722*tex.b);
    float value=smoothstep(LOW_RANGE,HIGH_RANGE,1.-lum);
    float v=min(value,1.);
    
    //first factor to determine how much rgb pixels split up
    //1. is complete seperation, 0. is merged
    float split=max(.33,v);
    return vec4(tex.xyz,split);
}

vec2 path(vec2 uv, vec2 res,vec3 mask,vec2 shift){

    vec4 v=findSplit(uv,res,vec2(0.,0.));
    vec4 vl=findSplit(uv,res,vec2(-1.,0.));
    vec4 vr=findSplit(uv,res,vec2(1.,0.));
    float split=v.w;
    float split_l=vl.w;
    float split_r=vr.w;
    
    
    
    vec2 c=mod(uv+shift,1./res)*res;
    
    
    float full=mask.x*v.x+mask.y*v.y+mask.z*v.z;
    
    float total_split=split;
    if(c.x>0.75){
        float f=(1.-(c.x-0.75)/.5);
        total_split=split*f+split_r*(1.-f);
        float side=mask.x*vr.x+mask.y*vr.y+mask.z*vr.z;
        full=full*f+side*(1.-f);
    }else if(c.x<0.25){
        float f=(c.x/.5)+0.5;
        //total_split=smoothstep(split_l,split,);
        float side=mask.x*vl.x+mask.y*vl.y+mask.z*vl.z;
        total_split=split*(f)+split_l*(1.-f);
        full=full*f+side*(1.-f);
    }
    
    
    float pixel_size=(DARK_FACTOR+1.-total_split)*1. ;
    
    float a=1.;
    if(total_split>LUMEN_THRESHOLD){
     a=(0.5-abs(c.y-0.5))*pixel_size*.2; //.2
    }
    a*=16.;
    return vec2(a,full);
            
}

void main()
{
    vec4 fragColor=vec4(1.,1.,1.,1.);
    float AR = iResolution.x / iResolution.y;
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = fragCoord/iResolution.xy;
    
    
            
    
    //======START tv to glass curvature===
    const float corner_harshness=1.2;
    const float corner_ease=4.;
    
    float vv=2.-min(mod(iTime,10.),2.);
    //float vv=cos(iTime*1.)*2.;
    float fade=max(pow(vv,16.),1.);
    fade=1.;
    
    float xx=(abs(uv.x -0.5)*corner_harshness);
    float yy=(abs(uv.y -0.5)*corner_harshness);
    float rr=(1.+pow((xx*xx+yy*yy),corner_ease));
    vec2 tuv=(uv-0.5)*rr+0.5;
    tuv=clamp(tuv,0.,1.);
    uv=tuv;
    //========END=========================
    
    if(  uv.x>0. && uv.x<1. && uv.y>0. && uv.y<1.){
    
        
    
        //===== START additional curvature for glass to allow fade in out but keep glass background
        yy=(abs(uv.y -0.5)*corner_harshness)*fade;
        rr=(1.+pow((xx*xx+yy*yy),corner_ease));
        tuv=(uv-0.5)*rr+0.5;
        tuv=clamp(tuv,0.,1.);
        uv=tuv;
        //===END==========================
        
        if(  uv.x>0. && uv.x<1. && uv.y>0. && uv.y<1.){

            
            //flicker
            uv+=sin(min(mod(iTime,1.),2.)*2000.)/10000.;

            //resolution factor
            float res=min(RESOLUTION,iResolution.x);
            vec2 res2=vec2(res*.5,res/AR);
            vec2 res3=res2;
            vec2 res4=res2;
            
            
            
             float shift=1./res;


            //vec2 pre_i=floor(uv*res2);
            //vec2 i=pre_i/res2;

            vec2 i=floor(uv*res2)/res2;
            vec4 tex = vec4(1.,1.,1.,1.);//texture(iChannel0, i);
            float lum=(0.2126*tex.r + 0.7152*tex.g + 0.0722*tex.b);
            float value=smoothstep(LOW_RANGE,HIGH_RANGE,1.-lum);
            float v=min(value,1.);


            //first factor to determine how much rgb pixels split up
            //1. is complete seperation, 0. is merged
            float split=max(.33,v);
            
            ////===== START scan lines
            float L=0.01*cos(uv.x*1.2+iTime*20.);
            float wave=cos(6.28*smoothstep(i.y,L,L+0.05))/5.;
            
           
            float scanny=cos(1.57+3.14*(.2-wave));
            float vvv=2.*scanny*cos(uv.x*16.+iTime*16.)/res;
            //========== END

           vec2 r=path(uv,res2,vec3(1.,0.,0.),vec2(0.,vvv)); //(uv.y*(uv.x+iTime/2.)*2.)*4.
           float red=r.y;
           float ar=r.x;

            vec2 uv2=uv;//+vec2(-split*0.8/res,0.);
            vec2 uv3=uv;//+vec2(-split*1.6/res,0.);
            
            //vec2 pushy=vec2(0.,cos((uv.x+iTime)*10.)*cos(uv.x*64. +iTime*32.)/2.);
            
            vec2 g=path(uv2,res3,vec3(0.,1.,0.),vec2(0.,0.));
            vec2 b=path(uv3,res4,vec3(0.,0.,1.),vec2(0.,-vvv));
            
            float ag=g.x;
            float green=g.y;
            float ab=b.x;
            float blue=b.y;
             
            // Time varying pixel color
            vec3 col = vec3(red*ar,green*ag,blue*ab); //vec3(r);//

            fragColor = vec4(col,1.);//vec4(cos(i2.x*100.),1.0,1.,1.0);
          
        }else{
            fragColor=vec4(0.,0.,0.,1.);
        }
    }else{
        float vx=abs(uv.x-0.5);
        float vy=abs(uv.y-0.5);
        float r=min(abs(vx-vy),0.25);//log(1./sqrt(vx*vx+vy*vy));
        fragColor=vec4(r,r,r,1.);
    }
}