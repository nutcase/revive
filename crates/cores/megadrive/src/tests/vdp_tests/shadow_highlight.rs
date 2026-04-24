#[test]
fn shadow_highlight_dac_accuracy() {
    // Verify shadow_channel and highlight_channel match the 4-bit DAC model.
    // Normal level L maps to L*36 (0-252).
    // Shadow = channel >> 1 (4-bit DAC output L vs normal 2L).
    // Highlight = channel + 18, clamped to 255 (4-bit DAC output 2L+1 vs normal 2L).
    use crate::vdp::{highlight_channel, shadow_channel};
    for level in 0..=7u8 {
        let normal = level as u16 * 36;
        let expected_shadow = normal / 2;
        let expected_highlight = (normal + 18).min(255);
        assert_eq!(
            shadow_channel(normal as u8) as u16,
            expected_shadow,
            "shadow of level {} (normal={})",
            level,
            normal
        );
        assert_eq!(
            highlight_channel(normal as u8) as u16,
            expected_highlight,
            "highlight of level {} (normal={})",
            level,
            normal
        );
    }
}
