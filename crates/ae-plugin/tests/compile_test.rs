//! Verify the AE plugin cdylib builds successfully.
//!
//! This is a compile-time test only. The actual plugin requires After Effects or
//! Premiere Pro to load and exercise. To test with a real host:
//!
//! 1. Build: `cargo build --package example-ae-plugin --lib`
//! 2. Locate the `.aex` (Windows) or `.plugin` bundle (macOS) in `target/`
//! 3. Copy to your AE/Premiere plugins directory
//! 4. Apply the "Example Effect" to a layer and verify:
//!    - All parameters appear in the Effect Controls panel
//!    - The Load/Save Preset buttons work
//!    - The image passes through unchanged
//!    - The "Advanced" group expands/collapses correctly

#[test]
fn plugin_crate_compiles() {
    // This test exists solely to verify the crate compiles.
    // If we get here, compilation succeeded.
    assert!(true);
}
