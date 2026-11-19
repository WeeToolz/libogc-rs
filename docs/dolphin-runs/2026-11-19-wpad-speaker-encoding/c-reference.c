#include <stdint.h>
#include <stdio.h>
typedef int32_t s32; typedef uint8_t ubyte;
typedef struct { int32_t predictor; int16_t step_index; int32_t step; int32_t prev_sample; int16_t sample1,sample2; int32_t coeff1,coeff2,idelta; } WENCStatus;
#define WENCMIN(a,b)		((a)>(b)?(b):(a))
#define ABS(x)				((s32)(x)>0?(s32)(x):-((s32)(x)))

static const int yamaha_indexscale[] = {
	230, 230, 230, 230, 307, 409, 512, 614,
	230, 230, 230, 230, 307, 409, 512, 614
};

static const int yamaha_difflookup[] = {
	1, 3, 5, 7, 9, 11, 13, 15,
	-1, -3, -5, -7, -9, -11, -13, -15
};


static __inline__ short wenc_clip_short(int a)
{
	if((a+32768)&~65535) return (a>>31)^32767;
	else return a;
}

static __inline__ int wenc_clip(int a,int amin,int amax)
{
	if(a<amin) return amin;
	else if(a>amax) return amax;
	else return a;
}

ubyte wencdata(WENCStatus *info,short sample)
{
	int nibble,delta;

	if(!info->step) {
		info->predictor = 0;
		info->step = 127;
	}

	delta = sample - info->predictor;
	nibble = WENCMIN(7,(ABS(delta)*4)/info->step) + ((delta<0)*8);

	info->predictor += ((info->step*yamaha_difflookup[nibble])/8);
	info->predictor = wenc_clip_short(info->predictor);
	info->step = (info->step*yamaha_indexscale[nibble])>>8;
	info->step = wenc_clip(info->step,127,24576);

	return nibble;
}
int main(void) { short samples[]={0,1000,-1000,32767,-32768,1234,42,-42}; WENCStatus s={0}; for(int i=0;i<8;i+=2) { int byte = wencdata(&s,samples[i])<<4; byte |= wencdata(&s,samples[i+1]); printf("%02x ",byte); } printf("predictor=%d step=%d\n",s.predictor,s.step); WENCStatus t={0}; short odd[]={0,1000,-1000,0}; for(int i=0;i<4;i+=2) { int byte=wencdata(&t,odd[i])<<4; byte |= wencdata(&t,odd[i+1]); printf("%02x ",byte); } printf("predictor=%d step=%d\n",t.predictor,t.step); return 0; }
