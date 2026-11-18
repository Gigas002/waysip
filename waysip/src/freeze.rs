//! Captures a screenshot of every output (via `libwayshot`) so it can be
//! used as the static backdrop for the selection overlay, giving the effect
//! of a frozen screen while selecting.

use std::collections::HashMap;

use wayland_client::protocol::wl_output::WlOutput;

/// Takes a screenshot of every output up front and returns a
/// [`libwaysip::BackgroundProvider`]-compatible closure that looks up the
/// pre-captured image for a given output by name.
///
/// Returns `None` (falling back to the normal, non-frozen overlay) if the
/// screenshot backend itself couldn't be initialised.
pub fn capture_backgrounds()
-> Option<impl Fn(&WlOutput, &str) -> Option<cairo::ImageSurface> + 'static> {
    let wayshot_conn = match libwayshot::WayshotConnection::new() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Warning: freeze: failed to initialize screenshot backend: {e}");
            return None;
        }
    };

    let mut backgrounds: HashMap<String, cairo::ImageSurface> = HashMap::new();
    for output_info in wayshot_conn.get_all_outputs() {
        let image = match wayshot_conn.screenshot_single_output(output_info, false) {
            Ok(image) => image,
            Err(e) => {
                eprintln!(
                    "Warning: freeze: failed to capture output {}: {e}",
                    output_info.name
                );
                continue;
            }
        };
        match image_to_argb_surface(image) {
            Some(surface) => {
                backgrounds.insert(output_info.name.clone(), surface);
            }
            None => {
                eprintln!(
                    "Warning: freeze: failed to convert screenshot for output {}",
                    output_info.name
                );
            }
        }
    }

    Some(move |_wl_output: &WlOutput, name: &str| backgrounds.get(name).cloned())
}

/// Converts an [`image::DynamicImage`] into a premultiplied-alpha
/// `cairo::ImageSurface` (`ARgb32`) suitable for use as a paint source.
pub(crate) fn image_to_argb_surface(image: image::DynamicImage) -> Option<cairo::ImageSurface> {
    // `into_rgba8` takes the buffer as is when it already is RGBA8.
    let rgba = image.into_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    if width == 0 || height == 0 {
        return None;
    }
    let stride = cairo::Format::ARgb32.stride_for_width(width as u32).ok()? as usize;
    let mut data = vec![0u8; stride * height];
    for (src_row, dst_row) in rgba
        .as_raw()
        .chunks_exact(width * 4)
        .zip(data.chunks_exact_mut(stride))
    {
        let (src_px, _) = src_row.as_chunks::<4>();
        let (dst_px, _) = dst_row.as_chunks_mut::<4>();
        for (s, d) in src_px.iter().zip(dst_px) {
            let [r, g, b, a] = s.map(u32::from);
            // cairo's ARgb32 stores premultiplied-alpha pixels; screenshots
            // are opaque, which needs no multiplying.
            let (r, g, b) = if a == 255 {
                (r, g, b)
            } else {
                (r * a / 255, g * a / 255, b * a / 255)
            };
            *d = ((a << 24) | (r << 16) | (g << 8) | b).to_ne_bytes();
        }
    }
    cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        width as i32,
        height as i32,
        stride as i32,
    )
    .ok()
}
