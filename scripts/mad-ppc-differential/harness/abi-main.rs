#![no_std]
#![no_main]
extern crate alloc;
use alloc::vec::Vec;
use core::{alloc::{GlobalAlloc,Layout}, panic::PanicInfo, sync::atomic::{AtomicUsize,Ordering}};
use mad_sys::*;
const CAPACITY:usize=8*1024*1024;
#[repr(align(64))] struct Arena([u8;CAPACITY]);
static mut HEAP:Arena=Arena([0;CAPACITY]);
static NEXT:AtomicUsize=AtomicUsize::new(0);
struct Bump;
#[global_allocator] static ALLOC:Bump=Bump;
unsafe impl GlobalAlloc for Bump {
 unsafe fn alloc(&self,l:Layout)->*mut u8 {
  let base=core::ptr::addr_of_mut!(HEAP).cast::<u8>();
  let mut old=NEXT.load(Ordering::Relaxed);
  loop {let aligned=((base as usize+old+l.align()-1)&!(l.align()-1))-base as usize;
   let Some(end)=aligned.checked_add(l.size())else{return core::ptr::null_mut()};
   if end>CAPACITY{return core::ptr::null_mut()}
   match NEXT.compare_exchange_weak(old,end,Ordering::Relaxed,Ordering::Relaxed){Ok(_)=>return unsafe{base.add(aligned)},Err(n)=>old=n}
  }
 }
 // Test-only bounded arena, reset after all objects for a fixture are dropped.
 unsafe fn dealloc(&self,_p:*mut u8,_l:Layout){}
}
#[unsafe(no_mangle)] pub unsafe extern "C" fn malloc(n:usize)->*mut u8{unsafe{ALLOC.alloc(Layout::from_size_align(n.max(1),16).unwrap())}}
#[unsafe(no_mangle)] pub unsafe extern "C" fn calloc(n:usize,s:usize)->*mut u8{let Some(len)=n.checked_mul(s)else{return core::ptr::null_mut()};let p=unsafe{malloc(len)};if !p.is_null(){unsafe{core::ptr::write_bytes(p,0,len)}}p}
#[unsafe(no_mangle)] pub extern "C" fn free(_p:*mut u8){} // Same test-only arena policy.
#[unsafe(no_mangle)] pub extern "C" fn abort()->!{loop{core::hint::spin_loop()}}
#[panic_handler] fn panic(_: &PanicInfo)->!{abort()}
unsafe extern "C"{fn c_begin(input:*const u8,len:u32);fn c_next(record:*mut i32)->i32;fn c_errors()->u32;fn c_end();}
#[unsafe(no_mangle)] static mut RESULTS:[u32;25]=[0;25];
// C frames, Rust frames, header fields, header mismatches, subband values,
// subband mismatches, PCM values, PCM mismatches, C and Rust recoverable errors,
// arena use, terminal status.
fn compare(input:&[u8],r:&mut[u32;12]){
 let mut bytes=Vec::from(input);bytes.resize(bytes.len()+MAD_BUFFER_GUARD,0);
 unsafe{c_begin(bytes.as_ptr(),bytes.len() as u32)};
 let mut stream=MadStream::default();let mut frame=MadFrame::default();let mut synth=MadSynth::default();unsafe {stream_init(&mut stream);frame_init(&mut frame);synth_init(&mut synth);stream_buffer(&mut stream,bytes.as_ptr(),bytes.len() as u32);}let mut c=[0i32;6+2304+2304];
 loop{
  let cr=unsafe{c_next(c.as_mut_ptr())};
  let rr=loop{let result=unsafe{frame_decode(&mut frame,&mut stream)};
   if result==0{break 1}if stream.error==MAD_ERROR_BUFLEN{break 0}if stream.error&0xff00!=0{r[9]+=1}else{break -1}
  };
  if cr==1{r[0]+=1}if rr==1{r[1]+=1}
  if cr!=rr||cr<0{r[11]=1;break}if cr==0{break}
  unsafe{synth_frame(&mut synth,&frame)};let pcm=&synth.pcm;
  let h=[frame.header.layer as i32,frame.header.mode as i32,frame.header.samplerate as i32,frame.header.bitrate as i32,pcm.channels as i32,pcm.length as i32];
  for(i,v)in h.iter().enumerate(){r[2]+=1;r[3]+=u32::from(*v!=c[i])}
  if h[4]!=c[4]||h[5]!=c[5]{r[11]=2;break}
  let mut k=6;
  for ch in &frame.sbsample{for slot in ch{for v in slot{r[4]+=1;r[5]+=u32::from(*v!=c[k]);k+=1}}}
  for ch in &pcm.samples[..pcm.channels as usize]{for v in &ch[..pcm.length as usize]{r[6]+=1;r[7]+=u32::from(*v!=c[k]);k+=1}}
 }
 r[8]=unsafe{c_errors()};unsafe{c_end();frame_finish(&mut frame);stream_finish(&mut stream);};r[10]=NEXT.load(Ordering::Relaxed) as u32;
}
#[unsafe(no_mangle)] pub extern "C" fn run(){let out=core::ptr::addr_of_mut!(RESULTS).cast::<u32>();for(i,data)in [include_bytes!("../../../libogc-rs-fixtures/layer3-stereo.mp3").as_slice(),include_bytes!("../../../libogc-rs-fixtures/libmad-stereo-mpeg1.mp3").as_slice()].iter().enumerate(){let mut r=[0;12];compare(data,&mut r);for(j,v)in r.iter().enumerate(){unsafe{out.add(1+12*i+j).write_volatile(*v)}}NEXT.store(0,Ordering::Relaxed);}unsafe{out.write_volatile(0x4d414432)}}
