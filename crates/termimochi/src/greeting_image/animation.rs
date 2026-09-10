//! Bounded GIF compositing. The image decoder handles offsets and disposal;
//! every retained frame has the same full canvas and premultiplied alpha.
use super::*;
use image::{
    AnimationDecoder,
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
};
use std::time::{Duration, Instant};

pub(crate) const MAX_FRAMES: usize = 120;
const MAX_FRAME_PIXELS: u64 = 1024 * 1024;
const MAX_TOTAL_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_DURATION_MS: u64 = 60_000;
static WORKER: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Scan the container before decompression, rejecting frame rectangles/budgets
// and truncated sub-blocks without allocating any pixel buffers.
fn inspect(bytes: &[u8]) -> Result<(), String> {
    struct Reader<'a> {
        bytes: &'a [u8],
        offset: usize,
    }
    impl<'a> Reader<'a> {
        fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
            let end = self
                .offset
                .checked_add(count)
                .ok_or("GIF block overflow.")?;
            let part = self
                .bytes
                .get(self.offset..end)
                .ok_or("Truncated GIF block.")?;
            self.offset = end;
            Ok(part)
        }
        fn blocks(&mut self) -> Result<(), String> {
            loop {
                let size = self.take(1)?[0] as usize;
                if size == 0 {
                    return Ok(());
                }
                self.take(size)?;
            }
        }
    }
    let mut input = Reader { bytes, offset: 0 };
    let header = input.take(13)?;
    if !matches!(&header[..6], b"GIF87a" | b"GIF89a") {
        return Err("Invalid GIF signature.".into());
    }
    let word = |data: &[u8]| u32::from(u16::from_le_bytes([data[0], data[1]]));
    let (w, h) = (word(&header[6..]), word(&header[8..]));
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return Err("GIF canvas is limited to 1024 × 1024 pixels.".into());
    }
    if header[10] & 0x80 != 0 {
        input.take(3 * (1usize << ((header[10] & 7) + 1)))?;
    }
    let mut count = 0;
    loop {
        match input.take(1)?[0] {
            0x21 => {
                input.take(1)?;
                input.blocks()?;
            }
            0x2c => {
                let descriptor = input.take(9)?;
                let (left, top, width, height) = (
                    word(descriptor),
                    word(&descriptor[2..]),
                    word(&descriptor[4..]),
                    word(&descriptor[6..]),
                );
                if width == 0 || height == 0 || left + width > w || top + height > h {
                    return Err("GIF frame is outside its logical canvas.".into());
                }
                count += 1;
                if count > MAX_FRAMES
                    || u64::from(w) * u64::from(h) * count as u64 > MAX_TOTAL_PIXELS
                {
                    return Err("GIF exceeds 120 frames or 64 MiB of decoded frame pixels. Reduce its size or duration.".into());
                }
                if descriptor[8] & 0x80 != 0 {
                    input.take(3 * (1usize << ((descriptor[8] & 7) + 1)))?;
                }
                let code_size = input.take(1)?[0];
                if !(2..=8).contains(&code_size) {
                    return Err("Invalid GIF LZW code size.".into());
                }
                input.blocks()?;
            }
            0x3b if count > 0 && input.offset == bytes.len() => return Ok(()),
            _ => return Err("Invalid GIF blocks or trailing data.".into()),
        }
    }
}

pub(crate) struct Frame {
    pub pixels: RgbaImage,
    pub delay_ms: u32,
}

pub(crate) struct Animation {
    pub frames: Vec<Frame>,
    pub dimensions: (u32, u32),
    repeat: Repeat,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Animation, String> {
    let _guard = WORKER
        .lock()
        .map_err(|_| "GIF worker stopped unexpectedly.")?;
    decode_inner(bytes)
}

fn decode_inner(bytes: &[u8]) -> Result<Animation, String> {
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("GIF exceeds 16 MiB.".into());
    }
    inspect(bytes)?;
    let mut decoder =
        GifDecoder::new(Cursor::new(bytes)).map_err(|e| format!("Invalid GIF: {e}"))?;
    let dimensions = decoder.dimensions();
    let pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > 1024
        || dimensions.1 > 1024
        || pixels > MAX_FRAME_PIXELS
    {
        return Err("GIF canvas is limited to 1024 × 1024 pixels.".into());
    }
    let mut bounds = limits();
    bounds.max_image_width = Some(1024);
    bounds.max_image_height = Some(1024);
    bounds.max_alloc = Some(16 * 1024 * 1024);
    decoder.set_limits(bounds).map_err(|e| e.to_string())?;
    let repeat = match decoder.loop_count() {
        image::metadata::LoopCount::Infinite => Repeat::Infinite,
        image::metadata::LoopCount::Finite(n) => {
            Repeat::Finite(n.get().min(u32::from(u16::MAX)) as u16)
        }
    };
    let mut frames = Vec::new();
    let mut duration = 0u64;
    let started = Instant::now();
    // Never collect an unbounded iterator. At most one extra frame is decoded
    // to detect excess; each decoder allocation also has an independent limit.
    for frame in decoder.into_frames() {
        if frames.len() >= MAX_FRAMES || pixels * (frames.len() as u64 + 1) > MAX_TOTAL_PIXELS {
            return Err("GIF exceeds 120 frames or 64 MiB of decoded frame pixels. Reduce its size or duration.".into());
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err("GIF decoding took too long.".into());
        }
        let frame = frame.map_err(|e| format!("Invalid GIF frame: {e}"))?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay_ms = (numerator / denominator.max(1)).max(20);
        duration += u64::from(delay_ms);
        if duration > MAX_DURATION_MS {
            return Err("GIF is limited to 60 seconds per cycle.".into());
        }
        let mut image = frame.into_buffer();
        if image.dimensions() != dimensions {
            return Err("GIF frame canvas does not match its logical screen.".into());
        }
        for pixel in image.pixels_mut() {
            for c in 0..3 {
                pixel[c] = ((u16::from(pixel[c]) * u16::from(pixel[3]) + 127) / 255) as u8;
            }
        }
        frames.push(Frame {
            pixels: image,
            delay_ms,
        });
    }
    if frames.is_empty() {
        return Err("GIF contains no frames.".into());
    }
    Ok(Animation {
        frames,
        dimensions,
        repeat,
    })
}

// A bounded writer prevents the GIF encoder from growing an unbounded Vec.
struct Encoded(Vec<u8>);
impl std::io::Write for Encoded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > FILE_LIMIT as usize {
            return Err(std::io::Error::other("Processed GIF exceeds 16 MiB."));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) struct Prepared {
    // Re-decoded exported GIF: preview shows the actual quantized colors/alpha.
    pub frames: Vec<Frame>, // straight alpha after decoding the export
    pub gif: Vec<u8>,
}

impl Prepared {
    pub fn frame_after(&self, mut index: usize, elapsed: Duration) -> (usize, Duration) {
        let cycle: u128 = self.frames.iter().map(|f| u128::from(f.delay_ms)).sum();
        let mut remaining = elapsed.as_millis() % cycle;
        while remaining >= u128::from(self.frames[index].delay_ms) {
            remaining -= u128::from(self.frames[index].delay_ms);
            index = (index + 1) % self.frames.len();
        }
        (index, Duration::from_millis(remaining as u64))
    }

    /// Full-frame replacement avoids icat versions that alpha-blend cleared
    /// GIF pixels onto earlier frames. Image numbers allocate a fresh image on
    /// every run. Only integer controls and locally encoded PNG data are emitted.
    pub fn kitty_stream(&self, columns: u32, rows: u32) -> Result<Vec<u8>, String> {
        use std::hash::BuildHasher;
        if !(1..=120).contains(&columns) || !(1..=64).contains(&rows) || self.frames.is_empty() {
            return Err("Invalid animation placement.".into());
        }
        let number =
            (std::collections::hash_map::RandomState::new().hash_one(&self.gif) as u32).max(1);
        let mut stream = String::new();
        let started = Instant::now();
        for (index, frame) in self.frames.iter().enumerate() {
            if started.elapsed() > Duration::from_secs(12) {
                return Err("Animation export took too long. Reduce the GIF size.".into());
            }
            let mut png = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(frame.pixels.clone())
                .write_to(&mut png, ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let payload = gtk::glib::base64_encode(png.get_ref());
            if stream.len() + payload.len() + 4096 > 32 * 1024 * 1024 {
                return Err(
                    "Animation protocol data exceeds 32 MiB. Reduce the GIF size or frame count."
                        .into(),
                );
            }
            let header = if index == 0 {
                format!("a=T,f=100,I={number},q=2,c={columns},r={rows},C=1")
            } else {
                format!("a=f,f=100,I={number},q=2,X=1,z={}", frame.delay_ms)
            };
            let chunks = payload.as_bytes().chunks(4096);
            let count = chunks.len();
            for (part, bytes) in chunks.enumerate() {
                let prefix = if part == 0 { header.as_str() } else { "q=2" };
                write!(
                    stream,
                    "\x1b_G{prefix},m={};{}\x1b\\",
                    u8::from(part + 1 < count),
                    std::str::from_utf8(bytes).unwrap()
                )
                .unwrap();
            }
        }
        write!(
            stream,
            "\x1b_Ga=a,I={number},q=2,r=1,z={}\x1b\\\x1b_Ga=a,I={number},q=2,s=3,v=1\x1b\\",
            self.frames[0].delay_ms
        )
        .unwrap();
        Ok(stream.into_bytes())
    }
}

pub(crate) fn prepare(source: &source::EditableArtwork) -> Result<Prepared, String> {
    let _guard = WORKER
        .lock()
        .map_err(|_| "GIF worker stopped unexpectedly.")?;
    let options = source.options()?;
    let mut animation = decode_inner(source.image.bytes())?;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for frame in &animation.frames {
        let (x, y, w, h) = processing::crop_bounds(&frame.pixels, options.edits);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + w);
        bottom = bottom.max(y + h);
    }
    let key = if options.edits.removal.enabled && options.edits.removal.color.is_none() {
        animation
            .frames
            .iter()
            .find_map(|f| background::detect(&f.pixels))
    } else {
        None
    };
    if options.edits.removal.enabled && options.edits.removal.color.is_none() && key.is_none() {
        return Err("Cannot identify a stable GIF background. Pick a custom background color in Edit Artwork.".into());
    }
    let mut writer = Encoded(Vec::new());
    let mut visible = false;
    let started = Instant::now();
    {
        let mut encoder = GifEncoder::new_with_speed(&mut writer, 10);
        encoder
            .set_repeat(animation.repeat)
            .map_err(|e| e.to_string())?;
        // Drain releases original frame buffers as processed frames are encoded.
        for frame in animation.frames.drain(..) {
            if started.elapsed() > Duration::from_secs(12) {
                return Err(
                    "GIF conversion took too long. Reduce the image size or number of frames."
                        .into(),
                );
            }
            let mut pixels =
                image::imageops::crop_imm(&frame.pixels, left, top, right - left, bottom - top)
                    .to_image();
            background::remove(&mut pixels, options.edits.removal, key);
            // An empty frame is valid within an animation; only an entirely
            // erased animation is rejected below. Keep canvas size unchanged.
            processing::adjust(&mut pixels, options.edits);
            if options.edits.smoothing > 0.0 {
                pixels = image::imageops::blur(&pixels, (options.edits.smoothing * 1.2) as f32);
            }
            for pixel in pixels.pixels_mut() {
                // GIF only supports binary transparency. Threshold explicitly
                // instead of letting the encoder silently make soft edges opaque.
                if pixel[3] < 128 {
                    pixel.0 = [0; 4];
                } else {
                    let color = processing::ink_color(straight_color(pixel).unwrap(), options);
                    pixel.0 = [color[0], color[1], color[2], 255];
                    visible = true;
                }
            }
            encoder
                .encode_frame(image::Frame::from_parts(
                    pixels,
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(frame.delay_ms, 1),
                ))
                .map_err(|e| e.to_string())?;
        }
    }
    if !visible {
        return Err("All GIF frames became transparent. Adjust background removal or crop.".into());
    }
    let mut decoded = decode_inner(&writer.0)?;
    // GIF has binary alpha, so straight and premultiplied pixel bytes coincide.
    let frames = std::mem::take(&mut decoded.frames);
    Ok(Prepared {
        frames,
        gif: writer.0,
    })
}

#[cfg(test)]
pub(crate) mod tests;
