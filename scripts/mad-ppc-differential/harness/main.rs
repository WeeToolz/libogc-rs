#![no_std]
#![no_main]
extern crate alloc;
use alloc::vec::Vec;
use core::{alloc::{GlobalAlloc,Layout}, panic::PanicInfo, sync::atomic::{AtomicUsize,Ordering}};
use libmad_rs::{Error,MAD_BUFFER_GUARD,Stream,frame::{FLAG_INCOMPLETE,Frame,Header,Layer},layer::decode::Layer3Decoder,synth::Synth};
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
 let mut stream=Stream::new();stream.buffer(&bytes);let mut frame=Frame::default();let mut decoder=Layer3Decoder::new();let mut synth=Synth::new();let mut c=[0i32;6+2304+2304];
 loop{
  let cr=unsafe{c_next(c.as_mut_ptr())};
  let rr=loop{let result=Header::decode(&mut stream).and_then(|header|{frame.header=header;frame.header.flags&=!FLAG_INCOMPLETE;frame.options=stream.options;if frame.header.layer!=Layer::III{return Err(Error::BadLayer)}decoder.decode(&mut stream,&mut frame)});
   match result{Ok(())=>break 1,Err(Error::BufferLength)=>break 0,Err(e) if e.is_recoverable()=>r[9]+=1,Err(_)=>break -1}
  };
  if cr==1{r[0]+=1}if rr==1{r[1]+=1}
  if cr!=rr||cr<0{r[11]=1;break}if cr==0{break}
  let pcm=synth.frame(&frame);
  let h=[frame.header.layer as i32,frame.header.mode as i32,frame.header.samplerate as i32,frame.header.bitrate as i32,pcm.channels as i32,pcm.length as i32];
  for(i,v)in h.iter().enumerate(){r[2]+=1;r[3]+=u32::from(*v!=c[i])}
  if h[4]!=c[4]||h[5]!=c[5]{r[11]=2;break}
  let mut k=6;
  for ch in &frame.sbsample{for slot in ch{for v in slot{r[4]+=1;r[5]+=u32::from(*v!=c[k]);k+=1}}}
  for ch in &pcm.samples[..pcm.channels as usize]{for v in &ch[..pcm.length as usize]{r[6]+=1;r[7]+=u32::from(*v!=c[k]);k+=1}}
 }
 r[8]=unsafe{c_errors()};unsafe{c_end()};r[10]=NEXT.load(Ordering::Relaxed) as u32;
}
#[unsafe(no_mangle)] pub extern "C" fn run(){let out=core::ptr::addr_of_mut!(RESULTS).cast::<u32>();for(i,data)in [include_bytes!("../../../libogc-rs-fixtures/layer3-stereo.mp3").as_slice(),include_bytes!("../../../libogc-rs-fixtures/libmad-stereo-mpeg1.mp3").as_slice()].iter().enumerate(){let mut r=[0;12];compare(data,&mut r);for(j,v)in r.iter().enumerate(){unsafe{out.add(1+12*i+j).write_volatile(*v)}}NEXT.store(0,Ordering::Relaxed);}unsafe{out.write_volatile(0x4d414432)}}
