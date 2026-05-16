use crate::settings::standard::ExampleEffect;

impl ExampleEffect {
    /// Apply the example effect to an RGBA buffer.
    ///
    /// This is a **passthrough** — the input is copied unchanged to the output.
    /// In a real effect, you would use `self.brightness`, `self.invert_colors`,
    /// `self.color_preset`, etc. to transform the pixel data.
    ///
    /// Each pixel is 4 bytes (R, G, B, A). The buffer length must be
    /// `width * height * 4`.
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        assert!(src.len() >= len, "source buffer too small");
        assert!(dst.len() >= len, "destination buffer too small");

        // Passthrough: copy src to dst unchanged.
        dst[..len].copy_from_slice(&src[..len]);

        // Parameters are accessible for demonstration:
        let _brightness = self.brightness;
        let _invert = self.invert_colors;
        let _preset = self.color_preset;

        // Example of checking whether a settings group is enabled:
        if let Some(adv) = &self.advanced {
            let _contrast = adv.contrast;
            let _saturation = adv.saturation;
        }
    }
}
