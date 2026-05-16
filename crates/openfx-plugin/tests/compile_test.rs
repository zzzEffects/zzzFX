//! Verify the OFX plugin cdylib builds successfully.
//!
//! This is a compile-time test only. The actual plugin requires an OFX host
//! (DaVinci Resolve, Natron, etc.) to load and exercise. To test with a real host:
//!
//! 1. Initialize the OFX submodule:
//!    `git submodule update --init`
//!
//! 2. Build with xtask (bundles into .ofx.bundle):
//!    `cargo xtask build-ofx-plugin`
//!
//! 3. Install the bundle from `crates/openfx-plugin/build/` into your host's
//!    OFX plugins directory.
//!
//! 4. Apply the "Example Effect" (category: "Example") and verify:
//!    - All parameters appear in the effect panel
//!    - The Load/Save Preset buttons work
//!    - The image passes through unchanged
//!    - Parameters can be adjusted (they don't affect the passthrough)

#[test]
fn plugin_crate_compiles() {
    // This test exists solely to verify the crate compiles.
    // If we get here, compilation succeeded.
    assert!(true);
}
