#include "config.h"
#include "global.h"
#include "stream.h"
#include "frame.h"
#include "synth.h"
#include <stdint.h>
static struct mad_stream stream;
static struct mad_frame frame;
static struct mad_synth synth;
static uint32_t errors;
void c_begin(const unsigned char *data, uint32_t size) {
  errors=0; mad_stream_init(&stream); mad_frame_init(&frame); mad_synth_init(&synth);
  mad_stream_buffer(&stream,data,size);
}
int32_t c_next(int32_t *out) {
  while (mad_frame_decode(&frame,&stream)) {
    if(stream.error==MAD_ERROR_BUFLEN)return 0;
    if(MAD_RECOVERABLE(stream.error)){errors++;continue;}
    return -(int32_t)stream.error;
  }
  if(frame.header.layer!=MAD_LAYER_III)return -10000;
  mad_synth_frame(&synth,&frame);
  *out++=frame.header.layer;*out++=frame.header.mode;*out++=frame.header.samplerate;
  *out++=frame.header.bitrate;*out++=synth.pcm.channels;*out++=synth.pcm.length;
  for(unsigned c=0;c<2;c++)for(unsigned j=0;j<36;j++)for(unsigned k=0;k<32;k++)*out++=frame.sbsample[c][j][k];
  for(unsigned c=0;c<synth.pcm.channels;c++)for(unsigned i=0;i<synth.pcm.length;i++)*out++=synth.pcm.samples[c][i];
  return 1;
}
uint32_t c_errors(void) { return errors; }
void c_end(void) { mad_frame_finish(&frame);mad_stream_finish(&stream); }
