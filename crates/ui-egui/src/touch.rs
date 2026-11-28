//! The iPad layout (`Prefs::touch_layout`, Settings › Layout, Home's rail, `view.touch_layout`):
//! controls sized and spaced for fingers and the Apple Pencil. The interface is drawn a little
//! larger (on top of the UI scale), and a tap within a few points of a control reaches it, as egui
//! hit-tests a touch against the widgets near it (`interact_radius`). Off, both are as before.

/// How much larger the iPad layout draws the interface: 28 pt controls become 32 pt.
pub(crate) const SCALE: f32 = 1.15;

/// How far from a control a tap still reaches it, in points, in the iPad layout.
pub(crate) const REACH: f32 = 12.0;

/// egui's own reach, outside the iPad layout.
const DESKTOP_REACH: f32 = 5.0;

/// The zoom the iPad layout adds to the UI scale.
pub(crate) fn scale(on: bool) -> f32 {
    if on { SCALE } else { 1.0 }
}

/// Give egui the reach of the layout, when it differs from what it has.
pub(crate) fn apply(ctx: &egui::Context, on: bool) {
    let reach = if on { REACH } else { DESKTOP_REACH };
    if ctx.global_style().interaction.interact_radius != reach {
        ctx.all_styles_mut(|style| style.interaction.interact_radius = reach);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ipad_layout_is_larger_and_reaches_further_and_off_is_as_before() {
        let ctx = egui::Context::default();
        apply(&ctx, true);
        assert_eq!(ctx.global_style().interaction.interact_radius, REACH);
        assert!(scale(true) > 1.0);
        apply(&ctx, false);
        assert_eq!(ctx.global_style().interaction.interact_radius, DESKTOP_REACH);
        assert_eq!(scale(false), 1.0);
    }
}
