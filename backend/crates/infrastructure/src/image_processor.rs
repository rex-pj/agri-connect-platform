//! `ImageProcessor` with the `image` crate. Every upload is decoded and
//! re-encoded as JPEG, so what we serve is always our own encoder's output:
//! no EXIF (phones embed the farm's GPS position there), no polyglot files,
//! no type confusion.

use std::io::Cursor;

use application::ports::{EncodedImage, ImageProcessor, ProcessedImage};
use async_trait::async_trait;
use domain::{Error, ErrorCode, Result};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ImageDecoder, ImageFormat,
    ImageReader, Limits, RgbImage,
};

/// What we accept and what we produce; the values live in
/// `application::constants` and are passed in by `startup.rs`.
#[derive(Debug, Clone, Copy)]
pub struct ImageRules {
    pub max_pixels: u64,
    pub main_max_side: u32,
    pub thumb_max_side: u32,
    pub jpeg_quality: u8,
}

pub struct RasterImageProcessor {
    rules: ImageRules,
}

impl RasterImageProcessor {
    pub fn new(rules: ImageRules) -> Self {
        Self { rules }
    }
}

const OUTPUT_CONTENT_TYPE: &str = "image/jpeg";

/// Worst case per pixel while decoding: 16-bit RGBA.
const MAX_BYTES_PER_PIXEL: u64 = 8;

#[async_trait]
impl ImageProcessor for RasterImageProcessor {
    async fn process(&self, upload: Vec<u8>) -> Result<ProcessedImage> {
        let rules = self.rules;
        // Decoding a 12 MP photo takes long enough to stall every request
        // sharing this worker thread.
        tokio::task::spawn_blocking(move || process(&upload, rules))
            .await
            .map_err(|e| Error::internal(format!("image task: {e}")))?
    }
}

fn invalid(e: image::ImageError) -> Error {
    Error::new(ErrorCode::InvalidImage).with_detail(e.to_string())
}

fn process(bytes: &[u8], rules: ImageRules) -> Result<ProcessedImage> {
    // The magic bytes decide; the client's Content-Type and file name are
    // never consulted.
    let format = image::guess_format(bytes)
        .ok()
        .filter(|f| matches!(f, ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP))
        .ok_or_else(|| Error::new(ErrorCode::UnsupportedImageType))?;

    let reader = || {
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        let mut limits = Limits::default();
        limits.max_alloc = Some(rules.max_pixels.saturating_mul(MAX_BYTES_PER_PIXEL));
        reader.limits(limits);
        reader
    };

    // Header only: a decompression bomb is refused before any pixel is decoded.
    let (width, height) = reader().into_dimensions().map_err(invalid)?;
    if u64::from(width) * u64::from(height) > rules.max_pixels {
        return Err(Error::new(ErrorCode::ImageTooManyPixels)
            .with_param("max_megapixels", rules.max_pixels / 1_000_000));
    }

    let mut decoder = reader().into_decoder().map_err(invalid)?;
    // Phones store "rotate 90°" in EXIF instead of rotating the pixels. EXIF is
    // dropped below, so apply it now or portrait photos come out sideways.
    let orientation = decoder.orientation().map_err(invalid)?;
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(invalid)?;
    decoded.apply_orientation(orientation);

    let main = fit(&decoded, rules.main_max_side);
    // From the already-reduced image: same result, a fraction of the work.
    let thumb = fit(&main, rules.thumb_max_side);
    Ok(ProcessedImage {
        main: encode(main, rules.jpeg_quality)?,
        thumb: encode(thumb, rules.jpeg_quality)?,
        content_type: OUTPUT_CONTENT_TYPE,
    })
}

/// Scales down so the longest side is at most `max_side`; never scales up.
fn fit(img: &DynamicImage, max_side: u32) -> DynamicImage {
    if img.width() <= max_side && img.height() <= max_side {
        img.clone()
    } else {
        img.resize(max_side, max_side, FilterType::CatmullRom)
    }
}

/// JPEG has no alpha channel: transparent areas become white, the colour a
/// product photo on a white page would show anyway.
fn flatten(img: DynamicImage) -> Result<RgbImage> {
    if !img.color().has_alpha() {
        return Ok(img.into_rgb8());
    }
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let blend = |c: u8, a: u8| {
        let (c, a) = (u16::from(c), u16::from(a));
        // (c·a + 255·(255−a)) / 255 never exceeds 255.
        u8::try_from((c * a + 255 * (255 - a)) / 255).unwrap_or(u8::MAX)
    };
    let pixels = rgba
        .pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            [blend(r, a), blend(g, a), blend(b, a)]
        })
        .collect();
    RgbImage::from_vec(w, h, pixels)
        .ok_or_else(|| Error::internal("flattened image has the wrong size"))
}

fn encode(img: DynamicImage, quality: u8) -> Result<EncodedImage> {
    let rgb = flatten(img)?;
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(&rgb)
        .map_err(|e| Error::internal(format!("jpeg encode: {e}")))?;
    Ok(EncodedImage {
        bytes,
        width: rgb.width(),
        height: rgb.height(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use image::{codecs::png::PngEncoder, ImageEncoder, Rgb, RgbaImage};

    use super::*;

    const RULES: ImageRules = ImageRules {
        max_pixels: 40_000_000,
        main_max_side: 1600,
        thumb_max_side: 400,
        jpeg_quality: 80,
    };

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 90)
            .encode_image(&img)
            .unwrap();
        out
    }

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(
                img,
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        out
    }

    /// An APP1 segment with a big-endian TIFF header and one IFD entry:
    /// Orientation (0x0112) = 6, "rotate 90° clockwise to display".
    fn exif_rotate_90() -> Vec<u8> {
        let mut tiff = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
        tiff.extend_from_slice(&[0x00, 0x01]); // one entry
        tiff.extend_from_slice(&[0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01]);
        tiff.extend_from_slice(&[0x00, 0x06, 0x00, 0x00]); // SHORT 6, padded
        tiff.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // no next IFD
        let mut payload = b"Exif\x00\x00".to_vec();
        payload.extend_from_slice(&tiff);
        let len = u16::try_from(payload.len() + 2).unwrap();
        let mut segment = vec![0xFF, 0xE1];
        segment.extend_from_slice(&len.to_be_bytes());
        segment.extend_from_slice(&payload);
        segment
    }

    fn with_exif(jpeg: &[u8]) -> Vec<u8> {
        // Right after SOI (FF D8), where cameras put it.
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&exif_rotate_90());
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    fn code(r: Result<ProcessedImage>) -> Option<ErrorCode> {
        r.err().map(|e| e.code())
    }

    #[test]
    fn exif_is_stripped_after_its_rotation_is_applied() {
        let upload = with_exif(&jpeg(40, 20));
        assert!(
            contains(&upload, b"Exif\x00\x00"),
            "fixture must carry EXIF"
        );

        let out = process(&upload, RULES).unwrap();
        // GPS lives in the same EXIF block; none of it may survive.
        assert!(!contains(&out.main.bytes, b"Exif"));
        assert!(!contains(&out.thumb.bytes, b"Exif"));
        // Rotated: a 40×20 landscape stored as "rotate 90°" is 20×40.
        assert_eq!((out.main.width, out.main.height), (20, 40));
        assert_eq!(out.content_type, "image/jpeg");
    }

    #[test]
    fn large_photos_are_reduced_and_small_ones_are_not_enlarged() {
        let out = process(&jpeg(2000, 1000), RULES).unwrap();
        assert_eq!((out.main.width, out.main.height), (1600, 800));
        assert_eq!((out.thumb.width, out.thumb.height), (400, 200));

        let out = process(&jpeg(300, 200), RULES).unwrap();
        assert_eq!((out.main.width, out.main.height), (300, 200));
        assert_eq!((out.thumb.width, out.thumb.height), (300, 200));
    }

    #[test]
    fn transparent_png_becomes_a_white_backed_jpeg() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 0]));
        let out = process(&png(&img), RULES).unwrap();
        assert!(out.main.bytes.starts_with(&[0xFF, 0xD8]), "output is JPEG");
        let back = image::load_from_memory(&out.main.bytes)
            .unwrap()
            .into_rgb8();
        assert!(back.pixels().all(|p| p.0.iter().all(|&c| c > 240)));
    }

    #[test]
    fn webp_is_accepted() {
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([10, 200, 10, 255]));
        let mut webp = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut webp)
            .write_image(&img, 8, 8, image::ExtendedColorType::Rgba8)
            .unwrap();
        assert!(process(&webp, RULES).is_ok());
    }

    #[test]
    fn only_jpeg_png_and_webp_are_accepted_whatever_the_name_says() {
        assert_eq!(
            code(process(b"GIF89a\x01\x00\x01\x00", RULES)),
            Some(ErrorCode::UnsupportedImageType)
        );
        assert_eq!(
            code(process(b"<svg xmlns='http://www.w3.org/2000/svg'/>", RULES)),
            Some(ErrorCode::UnsupportedImageType)
        );
        assert_eq!(
            code(process(b"", RULES)),
            Some(ErrorCode::UnsupportedImageType)
        );
    }

    #[test]
    fn a_truncated_photo_is_invalid_not_a_crash() {
        let full = jpeg(64, 64);
        let cut = &full[..full.len() / 2];
        assert_eq!(code(process(cut, RULES)), Some(ErrorCode::InvalidImage));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn a_decompression_bomb_is_refused_from_its_header() {
        // A tiny PNG whose header claims 50 000 × 50 000 pixels (10 GB as RGBA).
        let mut bomb = png(&RgbaImage::new(1, 1));
        // IHDR data starts at byte 16: 8 signature + 4 length + 4 type.
        bomb[16..20].copy_from_slice(&50_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&50_000u32.to_be_bytes());
        let crc = crc32(&bomb[12..29]); // type + 13 data bytes
        bomb[29..33].copy_from_slice(&crc.to_be_bytes());

        let err = process(&bomb, RULES).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ImageTooManyPixels);
        assert_eq!(err.params(), [("max_megapixels", "40".to_owned())]);
    }
}
