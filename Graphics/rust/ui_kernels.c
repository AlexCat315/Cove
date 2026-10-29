#include <stddef.h>
#include <stdint.h>
#include <string.h>
void cove_ui_pad_rgba(const uint8_t *source,size_t width,size_t height,uint8_t *destination){
    size_t out_width=width+2;
    for(size_t y=0;y<height+2;y++){size_t sy=y==0?0:y>height?height-1:y-1;uint8_t *row=destination+y*out_width*4;const uint8_t *src=source+sy*width*4;memcpy(row,src,4);memcpy(row+4,src,width*4);memcpy(row+(width+1)*4,src+(width-1)*4,4);}
}
