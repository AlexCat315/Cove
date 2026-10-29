/* Shared geometry implementation translated from DrawList.swift and AnalyticStroke.swift. */
#include "CRenderKernels.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>

typedef struct { float x, y; } CovePoint;
typedef struct { CoveRenderVertex *vertices; size_t count; int failed; } CoveClipResult;
static const float pi = 3.14159265358979323846f;
static CoveRenderVertex vertex(float x, float y, uint32_t color) { CoveRenderVertex v = { x, y, -1, 0, color }; return v; }
static CovePoint unit(CovePoint p) { float d = fmaxf(0.00001f, sqrtf(p.x*p.x + p.y*p.y)); CovePoint r = {p.x/d, p.y/d}; return r; }
static void connect(uint32_t *indices, size_t *at, uint32_t a, uint32_t b) {
    for (uint32_t band = 0; band < 3; ++band) {
        indices[(*at)++] = a + band; indices[(*at)++] = b + band; indices[(*at)++] = a + band + 1;
        indices[(*at)++] = a + band + 1; indices[(*at)++] = b + band; indices[(*at)++] = b + band + 1;
    }
}
void cove_shape_path(const CovePoint *path, size_t count, float width, float scale, uint32_t opaque, int closed,
    CoveRenderVertex *output, uint32_t *indices) {
    const float feather = 0.5f/fmaxf(1, scale), half = width*0.5f;
    const uint32_t clear = opaque & 0x00ffffffu;
    for (size_t index = 0; index < count; ++index) {
        const size_t previous = index ? index-1 : closed ? count-1 : 0;
        const size_t next = index+1 == count ? closed ? 0 : index : index+1;
        CovePoint incoming = unit((CovePoint){path[index].x-path[previous].x, path[index].y-path[previous].y});
        CovePoint outgoing = unit((CovePoint){path[next].x-path[index].x, path[next].y-path[index].y});
        CovePoint direction = !closed && !index ? outgoing : !closed && index+1 == count ? incoming : unit((CovePoint){incoming.x+outgoing.x, incoming.y+outgoing.y});
        CovePoint miter = {-direction.y, direction.x};
        CovePoint edge = !closed && index+1 == count ? (CovePoint){-incoming.y,incoming.x} : (CovePoint){-outgoing.y,outgoing.x};
        const float divisor = fmaxf(0.5f, fabsf(miter.x*edge.x + miter.y*edge.y));
        miter.x /= divisor; miter.y /= divisor;
        CovePoint position = path[index];
        if (!closed && !index) { position.x += outgoing.x*fminf(feather,half); position.y += outgoing.y*fminf(feather,half); }
        if (!closed && index+1 == count) { position.x -= incoming.x*fminf(feather,half); position.y -= incoming.y*fminf(feather,half); }
        const float distances[4] = {half+feather, fmaxf(0,half-feather), -fmaxf(0,half-feather), -half-feather};
        for (size_t band = 0; band < 4; ++band) output[index*4+band] = vertex(position.x+miter.x*distances[band], position.y+miter.y*distances[band], band==0 || band==3 ? clear : opaque);
    }
    size_t at = 0;
    for (size_t i = 0; i < (closed ? count : count-1); ++i) connect(indices,&at,(uint32_t)(i*4),(uint32_t)(((i+1)%count)*4));
    if (!closed) {
        for (size_t cap = 0; cap < 2; ++cap) {
            const size_t index = cap ? count-1 : 0;
            CovePoint direction = unit(cap ? (CovePoint){path[index].x-path[index-1].x,path[index].y-path[index-1].y}
                : (CovePoint){path[1].x-path[0].x,path[1].y-path[0].y});
            const float sign = cap ? 1.0f : -1.0f, travel = fminf(2*feather,width);
            const uint32_t base = (uint32_t)(count*4 + cap*4);
            for (size_t band=0; band<4; ++band) { CoveRenderVertex v = output[index*4+band]; v.x += direction.x*sign*travel; v.y += direction.y*sign*travel; v.color=clear; output[base+band]=v; }
            connect(indices,&at,(uint32_t)(index*4),base);
        }
    }
}
void cove_rounded_outline(float x, float y, float width, float height, float radius, size_t segments, CovePoint *output) {
    const CovePoint centers[4] = {{x+width-radius,y+radius},{x+width-radius,y+height-radius},{x+radius,y+height-radius},{x+radius,y+radius}};
    for (size_t corner=0; corner<4; ++corner) {
        const float start=-pi/2+(float)corner*pi/2;
        for (size_t step=0; step<=segments; ++step) { float angle=start+(float)step/(float)segments*pi/2;
            output[corner*(segments+1)+step]=(CovePoint){centers[corner].x+cosf(angle)*radius,centers[corner].y+sinf(angle)*radius}; }
    }
}
void cove_shape_rounded(float x, float y, float width, float height, float radius, float scale, size_t segments,
    uint32_t color, CoveRenderVertex *output, uint32_t *indices) {
    const float half=fminf(0.5f/fmaxf(1,scale),fminf(width,height)*0.5f);
    const float inner_radius=fmaxf(0,radius-half), outer_radius=radius+half;
    output[0]=vertex(x+width*0.5f,y+height*0.5f,color);
    for (size_t corner=0; corner<4; ++corner) {
        const int right=corner==1 || corner==2, bottom=corner>=2;
        const float ix=right ? x+width-half-inner_radius : x+half+inner_radius;
        const float iy=bottom ? y+height-half-inner_radius : y+half+inner_radius;
        const float ox=right ? x+width+half-outer_radius : x-half+outer_radius;
        const float oy=bottom ? y+height+half-outer_radius : y-half+outer_radius;
        const float start=(float)corner*pi/2+pi;
        for (size_t step=0; step<=segments; ++step) {
            const float angle=start+(float)step/(float)segments*pi/2;
            const size_t pair=corner*(segments+1)+step;
            output[1+pair*2]=vertex(ix+cosf(angle)*inner_radius,iy+sinf(angle)*inner_radius,color);
            output[2+pair*2]=vertex(ox+cosf(angle)*outer_radius,oy+sinf(angle)*outer_radius,color & 0x00ffffffu);
        }
    }
    const uint32_t pairs=(uint32_t)(4*(segments+1));
    for (uint32_t pair=0; pair<pairs; ++pair) {
        const uint32_t a=1+pair*2,b=1+((pair+1)%pairs)*2;
        const uint32_t values[9]={0,a,b,a,a+1,b,b,a+1,b+1}; memcpy(indices+pair*9,values,sizeof(values));
    }
}
static float side(CoveRenderVertex v,CovePoint a,CovePoint b) { return (b.x-a.x)*(v.y-a.y)-(b.y-a.y)*(v.x-a.x); }
static CoveRenderVertex interpolate(CoveRenderVertex a,CoveRenderVertex b,float t) {
    uint32_t color=0;
    for (unsigned shift=0; shift<32; shift+=8) { float x=(float)((a.color>>shift)&255),y=(float)((b.color>>shift)&255);
        color |= (uint32_t)fmaxf(0,fminf(255,roundf(x+(y-x)*t)))<<shift; }
    CoveRenderVertex v={a.x+(b.x-a.x)*t,a.y+(b.y-a.y)*t,a.u+(b.u-a.u)*t,a.v+(b.v-a.v)*t,color}; return v;
}
CoveClipResult cove_clip_polygon(const CoveRenderVertex *input,size_t count,const CovePoint *outline,size_t edges) {
    CoveClipResult result={NULL,0,0};
    if (count<3 || edges<3) return result;
    if (count > SIZE_MAX/sizeof(CoveRenderVertex)-2 || edges > SIZE_MAX/sizeof(CoveRenderVertex)-count-2) { result.failed=1; return result; }
    const size_t capacity=count+edges+2;
    CoveRenderVertex *polygon=malloc(capacity*sizeof(CoveRenderVertex)),*next=malloc(capacity*sizeof(CoveRenderVertex));
    if (!polygon || !next) { free(polygon); free(next); result.failed=1; return result; }
    memcpy(polygon,input,count*sizeof(CoveRenderVertex));
    for (size_t edge=0; edge<edges; ++edge) {
        const CovePoint a=outline[edge],b=outline[(edge+1)%edges];
        float minimum=INFINITY,maximum=-INFINITY;
        for (size_t i=0; i<count; ++i) { const float d=side(polygon[i],a,b); minimum=fminf(minimum,d); maximum=fmaxf(maximum,d); }
        if (minimum>=0) continue;
        if (maximum<0) { count=0; break; }
        size_t used=0;
        for (size_t i=0; i<count; ++i) {
            const CoveRenderVertex current=polygon[i],previous=polygon[(i+count-1)%count];
            const float cd=side(current,a,b),pd=side(previous,a,b);
            if ((cd>=0 && pd<0) || (cd<0 && pd>=0)) {
                if (used==capacity) { result.failed=1; goto cleanup; }
                next[used++]=interpolate(previous,current,pd/(pd-cd));
            }
            if (cd>=0) { if (used==capacity) { result.failed=1; goto cleanup; } next[used++]=current; }
        }
        CoveRenderVertex *saved=polygon; polygon=next; next=saved; count=used;
        if (count<3) { count=0; break; }
    }
    result.vertices=polygon; result.count=count; free(next); return result;
cleanup:
    free(polygon); free(next); return result;
}
void cove_clip_free(CoveRenderVertex *vertices) { free(vertices); }
static float rounded_distance(CoveRenderVertex vertex,float x,float y,float width,float height,float radius) {
    const float dx=fabsf(vertex.x-x-width/2)-width/2+radius,dy=fabsf(vertex.y-y-height/2)-height/2+radius;
    return sqrtf(fmaxf(dx,0)*fmaxf(dx,0)+fmaxf(dy,0)*fmaxf(dy,0))+fminf(fmaxf(dx,dy),0)-radius;
}
void cove_clip_feather(CoveRenderVertex *vertices,size_t count,float x,float y,float width,float height,float radius,float scale) {
    for (size_t i=0; i<count; ++i) {
        const float coverage=fmaxf(0,fminf(1,0.5f-rounded_distance(vertices[i],x,y,width,height,radius)*scale));
        const uint32_t alpha=(uint32_t)roundf((float)(vertices[i].color>>24)*coverage);
        vertices[i].color=(vertices[i].color & 0x00ffffffu) | (alpha<<24);
    }
}
static void line_quad(CoveRenderVertex *vertices,uint32_t *indices,size_t *vc,size_t *ic,
    CoveRenderVertex a,CoveRenderVertex b,CoveRenderVertex c,CoveRenderVertex d) {
    const uint32_t base=(uint32_t)*vc; vertices[(*vc)++]=a; vertices[(*vc)++]=b; vertices[(*vc)++]=c; vertices[(*vc)++]=d;
    const uint32_t data[6]={base,base+1,base+2,base,base+2,base+3}; memcpy(indices+*ic,data,sizeof(data)); *ic+=6;
}
void cove_shape_line(float x0,float y0,float x1,float y1,float thickness,float scale,uint32_t opaque,
    CoveRenderVertex *vertices,uint32_t *indices,size_t *vertex_count,size_t *index_count) {
    const float dx=x1-x0,dy=y1-y0,length=sqrtf(dx*dx+dy*dy),nx=-dy/length,ny=dx/length;
    const float half=fmaxf(thickness,0.5f)*0.5f,feather=fminf(0.5f/fmaxf(1,scale),half*0.5f);
    const float inner=fmaxf(0,half-feather),outer=half+feather; const uint32_t clear=opaque & 0x00ffffffu;
    size_t vc=0,ic=0;
    if (inner>0) line_quad(vertices,indices,&vc,&ic,vertex(x0+nx*inner,y0+ny*inner,opaque),vertex(x1+nx*inner,y1+ny*inner,opaque),
        vertex(x1-nx*inner,y1-ny*inner,opaque),vertex(x0-nx*inner,y0-ny*inner,opaque));
    line_quad(vertices,indices,&vc,&ic,vertex(x0+nx*inner,y0+ny*inner,opaque),vertex(x1+nx*inner,y1+ny*inner,opaque),
        vertex(x1+nx*outer,y1+ny*outer,clear),vertex(x0+nx*outer,y0+ny*outer,clear));
    line_quad(vertices,indices,&vc,&ic,vertex(x0-nx*inner,y0-ny*inner,opaque),vertex(x1-nx*inner,y1-ny*inner,opaque),
        vertex(x1-nx*outer,y1-ny*outer,clear),vertex(x0-nx*outer,y0-ny*outer,clear));
    for (size_t cap=0; cap<2; ++cap) {
        const float cx=cap ? x1 : x0,cy=cap ? y1 : y0;
        const uint32_t center=(uint32_t)vc; vertices[vc++]=vertex(cx,cy,opaque); const uint32_t ring=(uint32_t)vc;
        for (size_t step=0; step<=8; ++step) { float angle=(float)step/8*2*pi,ox=cosf(angle),oy=sinf(angle);
            vertices[vc++]=vertex(cx+ox*inner,cy+oy*inner,opaque); vertices[vc++]=vertex(cx+ox*outer,cy+oy*outer,clear); }
        for (uint32_t step=0; step<8; ++step) { const uint32_t a=ring+step*2,b=ring+(step+1)*2;
            const uint32_t values[9]={center,a,b,a,a+1,b+1,a,b+1,b}; memcpy(indices+ic,values,sizeof(values)); ic+=9; }
    }
    *vertex_count=vc; *index_count=ic;
}
static CoveRenderVertex image_vertex(float x,float y,float sx,float sy,float width,float height,uint32_t color,
    float u0,float v0,float u1,float v1) {
    const float us=width>0 ? (x-sx)/width : 0,vs=height>0 ? (y-sy)/height : 0;
    CoveRenderVertex v={x,y,u0+(u1-u0)*us+10,v0+(v1-v0)*vs,color}; return v;
}
void cove_shape_image_corner(float cx,float cy,float radius,float start,float x,float y,float width,float height,
    uint32_t color,float u0,float v0,float u1,float v1,size_t segments,CoveRenderVertex *vertices,uint32_t *indices) {
    vertices[0]=image_vertex(cx,cy,x,y,width,height,color,u0,v0,u1,v1);
    for (size_t i=0; i<=segments; ++i) { const float angle=start+(float)i/(float)segments*(pi/2);
        vertices[1+i]=image_vertex(cx+cosf(angle)*radius,cy+sinf(angle)*radius,x,y,width,height,color,u0,v0,u1,v1); }
    for (uint32_t i=0; i<(uint32_t)segments; ++i) { indices[i*3]=0; indices[i*3+1]=i+1; indices[i*3+2]=i+2; }
}
