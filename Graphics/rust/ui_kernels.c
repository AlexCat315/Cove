#include <stddef.h>
#include <stdint.h>
#include <string.h>
void cove_ui_pad_rgba(const uint8_t *source,size_t width,size_t height,uint8_t *destination){
    size_t out_width=width+2;
    for(size_t y=0;y<height+2;y++){size_t sy=y==0?0:y>height?height-1:y-1;uint8_t *row=destination+y*out_width*4;const uint8_t *src=source+sy*width*4;memcpy(row,src,4);memcpy(row+4,src,width*4);memcpy(row+(width+1)*4,src+(width-1)*4,4);}
}
#include <math.h>
typedef struct {float x,y,u,v;uint32_t color;} CoveUIVertex;
int cove_ui_validate_geometry(const CoveUIVertex *vertices,size_t vertex_count,const uint32_t *indices,size_t index_count){for(size_t i=0;i<vertex_count;i++){CoveUIVertex v=vertices[i];if(!isfinite(v.x)||!isfinite(v.y)||!isfinite(v.u)||!isfinite(v.v))return 0;}for(size_t i=0;i<index_count;i++)if(indices[i]>=vertex_count)return 0;return 1;}
int cove_ui_relies_on_clip(const CoveUIVertex *vertices,const uint32_t *indices,size_t count,float x,float y,float r,float b){for(size_t i=0;i<count;i++){CoveUIVertex v=vertices[indices[i]];if(v.x<x||v.y<y||v.x>r||v.y>b)return 1;}return 0;}
void cove_ui_remap_uvs(const CoveUIVertex *source,CoveUIVertex *out,const uint32_t *indices,size_t count,float x,float y,float width,float height,float size){for(size_t i=0;i<count;i++){uint32_t index=indices[i];CoveUIVertex v=source[index];float base=v.u>=15?20:v.u>=5?10:-1;if(base<0)continue;v.u=base+(x+(v.u-base)*width)/size;v.v=(y+v.v*height)/size;out[index]=v;}}
