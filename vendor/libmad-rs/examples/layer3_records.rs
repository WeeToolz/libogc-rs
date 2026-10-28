//! Emit full Layer III decode records in the existing C comparison format.

use std::{
    env, fs,
    io::{self, Write},
};

use libmad_rs::{
    Error, MAD_BUFFER_GUARD, Stream,
    frame::{FLAG_INCOMPLETE, FLAG_LSF_EXT, Frame, Header, Layer, Mode},
    layer::{
        decode::Layer3Decoder,
        layer3::{FLAG_MIXED_BLOCK, parse_side_info},
    },
    synth::Synth,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: layer3_records INPUT.mp3")?;
    let mut input = fs::read(path)?;
    input.resize(input.len() + MAD_BUFFER_GUARD, 0);
    let mut stream = Stream::new();
    stream.buffer(&input);
    let mut frame = Frame::default();
    let mut decoder = Layer3Decoder::new();
    let mut synth = Synth::new();
    let mut output = io::BufWriter::new(io::stdout().lock());
    let mut frames = 0u32;
    let mut recoverable_errors = 0u32;
    let mut block_types = [0u32; 4];
    let mut mixed_blocks = 0u32;
    let mut mode_extensions = [0u32; 4];
    loop {
        let result = Header::decode(&mut stream).and_then(|header| {
            frame.header = header;
            frame.header.flags &= !FLAG_INCOMPLETE;
            frame.options = stream.options;
            if frame.header.layer != Layer::III {
                return Err(Error::BadLayer);
            }
            let mut side_ptr = stream.ptr;
            let side = parse_side_info(
                &mut side_ptr,
                &stream.buffer,
                header.channels(),
                header.flags & FLAG_LSF_EXT != 0,
            )
            .ok();
            decoder.decode(&mut stream, &mut frame)?;
            if let Some(side) = side {
                for granule in &side.granules[..side.granule_count] {
                    for channel in &granule.channels[..side.channel_count] {
                        block_types[usize::from(channel.block_type)] += 1;
                        mixed_blocks += u32::from(channel.flags & FLAG_MIXED_BLOCK != 0);
                    }
                }
            }
            if header.mode == Mode::JointStereo {
                mode_extensions[usize::from(header.mode_extension)] += 1;
            }
            Ok(())
        });
        if let Err(error) = result {
            if error == Error::BufferLength {
                break;
            }
            if error.is_recoverable() {
                recoverable_errors += 1;
                continue;
            }
            return Err(format!("fatal Rust decoder error: {error:?}").into());
        }
        let pcm = synth.frame(&frame);
        let fields = [
            frame.header.layer as u32,
            frame.header.mode as u32,
            frame.header.samplerate,
            frame.header.bitrate,
            u32::from(pcm.channels),
            u32::from(pcm.length),
        ];
        for value in fields {
            output.write_all(&value.to_le_bytes())?;
        }
        for channel in &frame.sbsample {
            for slot in channel {
                for value in slot {
                    output.write_all(&value.to_le_bytes())?;
                }
            }
        }
        for channel in &pcm.samples[..usize::from(pcm.channels)] {
            for value in &channel[..usize::from(pcm.length)] {
                output.write_all(&value.to_le_bytes())?;
            }
        }
        frames += 1;
    }
    output.flush()?;
    eprintln!(
        "Rust frames={frames} recoverable_errors={recoverable_errors} final_error=0x{:04x}",
        stream.error as u16
    );
    eprintln!(
        "Rust coverage={{\"block_types\":{block_types:?},\"mixed_blocks\":{mixed_blocks},\"joint_stereo_mode_extensions\":{mode_extensions:?}}}"
    );
    Ok(())
}
