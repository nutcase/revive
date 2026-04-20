use std::io::BufWriter;

fn write_framebuffer_ppm(
    path: &std::path::Path,
    fb: &[u32],
    width: usize,
    height: usize,
) -> Result<(), String> {
    let mut ppm = Vec::with_capacity(width * height * 3 + 32);
    ppm.extend_from_slice(format!("P6\n{} {}\n255\n", width, height).as_bytes());
    for &px in fb.iter().take(width * height) {
        let r = ((px >> 16) & 0xFF) as u8;
        let g = ((px >> 8) & 0xFF) as u8;
        let b = (px & 0xFF) as u8;
        ppm.extend_from_slice(&[r, g, b]);
    }
    std::fs::write(path, &ppm).map_err(|e| e.to_string())
}

pub(crate) fn write_framebuffer_png(
    path: &std::path::Path,
    fb: &[u32],
    width: usize,
    height: usize,
) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;

    let mut rgba = Vec::with_capacity(width * height * 4);
    for &px in fb.iter().take(width * height) {
        let r = ((px >> 16) & 0xFF) as u8;
        let g = ((px >> 8) & 0xFF) as u8;
        let b = (px & 0xFF) as u8;
        let a = ((px >> 24) & 0xFF) as u8;
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    Ok(())
}

pub(in crate::emulator) fn write_framebuffer_image(
    path: &std::path::Path,
    fb: &[u32],
    width: usize,
    height: usize,
) -> Result<(), String> {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("ppm") => {
            write_framebuffer_ppm(path, fb, width, height).map_err(|e| e.to_string())
        }
        Some(ext) if ext.eq_ignore_ascii_case("png") => {
            write_framebuffer_png(path, fb, width, height)
        }
        _ => write_framebuffer_png(path, fb, width, height),
    }
}
