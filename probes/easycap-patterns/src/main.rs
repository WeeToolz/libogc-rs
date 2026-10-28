#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

mod identification;
mod patterns;

#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    use ogc_input as input;
    use ogc_system as system;
    use ogc_video as video;
    use patterns::{HEIGHT, NAMES, WIDTH};
    if let Err(error) = system::init_checked() {
        system::report(format_args!(
            "EasyCap patterns: system initialization failed: {error:?}\n"
        ));
        return -4;
    }
    video::init();
    let mut mode = video::sys::TV_NTSC_480_INT_DF;
    // Force NTSC 480i even when the console preference is progressive or PAL.
    // SAFETY: mode remains live for the complete program; allocator owns two XFBs.
    let mut display = unsafe { system::allocate_framebuffer(&mut mode) };
    let mut draw = unsafe { system::allocate_framebuffer(&mut mode) };
    if display.is_null() || draw.is_null() {
        system::report(format_args!("EasyCap patterns: XFB allocation failed\n"));
        return -1;
    }
    unsafe {
        video::clear_frame_buffer(&mut mode, display, patterns::grey(16));
        video::clear_frame_buffer(&mut mode, draw, patterns::grey(16));
        video::configure(&mut mode);
        video::set_next_framebuffer(display);
    }
    video::set_black(false);
    video::flush();
    if !video::wait_vsync_timeout_us(200_000) {
        system::report(format_args!(
            "EasyCap patterns: initial VI retrace timeout\n"
        ));
        return -2;
    }
    if input::init() == 0 {
        system::report(format_args!(
            "EasyCap patterns: PAD initialization failed\n"
        ));
        return -5;
    }
    system::report(format_args!(
        "EasyCap patterns: NTSC 640x480 XFB; PAD initialized\n"
    ));
    let mut page = 0;
    let mut tick = 0u32;
    let mut elapsed = 0u32;
    let mut auto = true;
    let mut hold = false;
    let mut retrace_snapshot = 0;
    loop {
        let connected = input::scan_pads();
        let mut down = 0;
        for channel in 0..4 {
            if connected & (1u32 << channel) != 0 {
                down |= input::buttons_down(channel);
            }
        }
        if down & (input::PAD_BUTTON_A | input::PAD_BUTTON_RIGHT) != 0 {
            page = (page + 1) % NAMES.len();
            auto = false;
            elapsed = 0;
        } else if down & (input::PAD_BUTTON_B | input::PAD_BUTTON_LEFT) != 0 {
            page = (page + NAMES.len() - 1) % NAMES.len();
            auto = false;
            elapsed = 0;
        }
        if down & input::PAD_BUTTON_START != 0 {
            auto = !auto;
            elapsed = 0;
        }
        if down & input::PAD_BUTTON_X != 0 {
            hold = !hold;
        }
        if auto && !hold && elapsed >= 600 {
            page = (page + 1) % NAMES.len();
            elapsed = 0;
        }
        // SAFETY: draw is a live aligned XFB allocation for exactly 640x480 YUYV.
        // K1 bypasses data cache; the previous display buffer is never rendered here.
        let pixels = unsafe {
            core::slice::from_raw_parts_mut(
                ogc_core::mem_k0_to_k1(draw as usize) as *mut u32,
                WIDTH * HEIGHT / 2,
            )
        };
        if !hold {
            retrace_snapshot = video::retrace_count();
        }
        patterns::render(pixels, page, tick, retrace_snapshot, auto, hold);
        unsafe {
            video::set_next_framebuffer(draw);
        }
        video::flush();
        if !video::wait_vsync_timeout_us(200_000) {
            system::report(format_args!(
                "EasyCap patterns: VI retrace timeout on page {page}\n"
            ));
            return -3;
        }
        core::mem::swap(&mut display, &mut draw);
        if !hold {
            tick = tick.wrapping_add(1);
            elapsed += 1;
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    ogc_system::report(format_args!("EasyCap patterns panic: {info}\n"));
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_arch = "powerpc"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use patterns::{HEIGHT, NAMES, WIDTH};
    use std::{io::Write, path::PathBuf};
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--export" {
        return Err("usage: easycap-patterns --export DIRECTORY".into());
    }
    let dir = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&dir)?;
    let mut frame = vec![0; WIDTH * HEIGHT / 2];
    for (page, name) in NAMES.iter().enumerate() {
        patterns::render(&mut frame, page, 0, 0, true, false);
        let stem = format!("{page:02}-{}", name.to_lowercase().replace(' ', "-"));
        let mut raw = std::fs::File::create(dir.join(format!("{stem}.yuyv")))?;
        let mut ppm = std::fs::File::create(dir.join(format!("{stem}.ppm")))?;
        write!(ppm, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
        for word in &frame {
            let [y0, u, y1, v] = word.to_be_bytes();
            raw.write_all(&[y0, u, y1, v])?;
            for y in [y0, y1] {
                let c = i32::from(y) - 16;
                let d = i32::from(u) - 128;
                let e = i32::from(v) - 128;
                let rgb = [
                    (298 * c + 409 * e + 128) >> 8,
                    (298 * c - 100 * d - 208 * e + 128) >> 8,
                    (298 * c + 516 * d + 128) >> 8,
                ]
                .map(|v| v.clamp(0, 255) as u8);
                ppm.write_all(&rgb)?;
            }
        }
    }
    std::fs::write(
        dir.join("identification.json"),
        format!(
            "{{\n  \"version\": 1,\n  \"page\": {},\n  \"page_count\": {},\n  \"width\": {WIDTH},\n  \"height\": {HEIGHT},\n  \"byte_order\": \"YUYV\",\n  \"bit_order\": \"msb-first\",\n  \"bit_x\": {},\n  \"bit_width\": {},\n  \"band_height\": {},\n  \"bands\": {:?},\n  \"words\": [\"presentation_id\", \"inverted_presentation_id\", \"vi_retrace_snapshot\"],\n  \"black_y\": 16,\n  \"white_y\": 235,\n  \"other_parity_y\": 128\n}}\n",
            identification::PAGE,
            NAMES.len(),
            identification::BIT_X,
            identification::BIT_WIDTH,
            identification::BAND_HEIGHT,
            identification::BANDS,
        ),
    )?;
    std::fs::write(
        dir.join("FORMAT.txt"),
        "SOURCE references: 640x480, YUYV422 (Y0 U Y1 V), tick=0, AUTO.\n\
         EasyCap captures are 720x480 UYVY422 (U Y0 V Y1); align active picture before comparison.\n\
         PPM previews use limited-range BT.601 conversion with RGB clipping.\n\
         Luma page top bands x/40: 0 8 16 20 24 32 48 64 96 128 160 192 219 235 240 255.\n\
         Measurement area: avoid labels y=34..59 and y=424..467.\n\
         Flat fields: top-left Y16, top-right Y64, bottom-left Y128, bottom-right Y235; U=V=128.\n\
         Source row parity is not proof of captured field order.\n",
    )?;
    println!(
        "Exported {} source patterns to {}",
        NAMES.len(),
        dir.display()
    );
    Ok(())
}
