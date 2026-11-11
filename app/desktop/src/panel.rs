//! Placing the windows of the menu key's layer: scaled to the monitor like the overlay the first
//! time, then where and how big the player left them.

use std::sync::OnceLock;

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition};

pub struct Place {
    /// At 1080p. The window is scaled to the monitor's height from it.
    pub size: (f64, f64),
    pub min_size: (f64, f64),
    /// Top-left corner as fractions of the monitor.
    pub anchor: Option<(f64, f64)>,
    pub default_anchor: (f64, f64),
}

pub fn place(app: &AppHandle, window: &tauri::WebviewWindow, sized: &OnceLock<()>, p: Place) {
    let Some(m) = app
        .get_webview_window("overlay")
        .and_then(|w| w.current_monitor().ok().flatten())
    else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let zoom = crate::chat::screen_zoom(ms.height as f64 / m.scale_factor());
    let scaled = |(w, h): (f64, f64)| LogicalSize::new(w * zoom, h * zoom);
    let (width, height) = if sized.set(()).is_ok() {
        let _ = window.set_zoom(zoom);
        let _ = window.set_min_size(Some(scaled(p.min_size)));
        let _ = window.set_size(scaled(p.size));
        let px = |v: f64| (v * zoom * m.scale_factor()) as i32;
        (px(p.size.0), px(p.size.1))
    } else {
        let size = window.outer_size().unwrap_or_default();
        (size.width as i32, size.height as i32)
    };
    let max_x = (ms.width as i32 - width).max(0);
    let max_y = (ms.height as i32 - height).max(0);
    let (fx, fy) = p.anchor.unwrap_or(p.default_anchor);
    let (x, y) = (
        (fx * ms.width as f64) as i32,
        (fy * ms.height as f64) as i32,
    );
    let _ = window.set_position(PhysicalPosition::new(
        mp.x + x.clamp(0, max_x),
        mp.y + y.clamp(0, max_y),
    ));
}

pub struct Spot {
    pub anchor: (f64, f64),
    /// At 1080p, like [`Place::size`].
    pub size: (f64, f64),
}

/// Where the window is now, to save before it hides.
pub fn spot(window: &tauri::WebviewWindow) -> Option<Spot> {
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let m = window.current_monitor().ok()??;
    let (mp, ms) = (m.position(), m.size());
    let zoom = crate::chat::screen_zoom(ms.height as f64 / m.scale_factor());
    let fx = (pos.x - mp.x) as f64 / ms.width as f64;
    let fy = (pos.y - mp.y) as f64 / ms.height as f64;
    let at_1080 = |v: u32| (v as f64 / m.scale_factor() / zoom).round();
    Some(Spot {
        anchor: (fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0)),
        size: (at_1080(size.width), at_1080(size.height)),
    })
}
