# Rust Multi-Host Plugin Skeleton

A Rust workspace skeleton for building cross-host video effect plugins. Extracted from [ntsc-rs](https://github.com/valadaptive/ntsc-rs).

## Structure

```
plugin_example/
├── crates/
│   ├── macros/              # Proc macro: `#[derive(FullSettings)]`
│   ├── example-effect/      # Shared effect library (parameters + render)
│   ├── ae-plugin/           # Adobe After Effects / Premiere Pro plugin (cdylib)
│   └── openfx-plugin/       # OpenFX plugin (cdylib, DaVinci Resolve / Natron)
└── xtask/                   # Build & bundle helper (cargo xtask)
```

## Supported Hosts

| Host | Crate | Build Command |
|------|-------|---------------|
| After Effects / Premiere Pro | `ae-plugin` | `cargo build -p example-ae-plugin` |
| OpenFX (Resolve, Natron, etc.) | `openfx-plugin` | `cargo xtask build-ofx-plugin` |

## Quick Start

### 1. Check compilation

```bash
cargo check --workspace
```

### 2. Run tests

```bash
cargo test --workspace
```

This runs:
- **Settings tests**: JSON round-trip, get/set fields, descriptor iteration
- **Effect tests**: passthrough correctness with various dimensions
- **Plugin compile tests**: verify cdylibs build successfully

### 3. Build the OpenFX plugin (Windows)

```bash
# Initialize the OpenFX SDK submodule
git submodule update --init

# Build and bundle (output in crates/openfx-plugin/build/)
cargo xtask build-ofx-plugin

# For release mode:
cargo xtask build-ofx-plugin --release
```

### 4. Build the AE plugin

```bash
# Windows: build the .aex DLL
cargo build --package example-ae-plugin --lib

# macOS: build and bundle into .plugin
cargo xtask macos-ae-plugin
```

## How to Write a Plugin

### 1. Define your effect parameters in `crates/example-effect/src/settings/standard.rs`

```rust
#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ExampleEffect {
    pub brightness: f32,
    pub invert_colors: bool,
    // ...
}
```

### 2. Implement `Settings` trait with `setting_descriptors()`

This provides the introspectable parameter descriptions used by both plugin hosts.

### 3. Write the effect render function in `crates/example-effect/src/effect.rs`

```rust
impl ExampleEffect {
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        // Your effect logic here
    }
}
```

### 4. The plugins automatically map parameters

Both `ae-plugin` and `openfx-plugin` use the generic `SettingsList` to:
- Generate host-specific UI controls (sliders, checkboxes, dropdowns)
- Read parameter values back during render
- Support preset load/save (JSON)

## Testing with Real Hosts

### OpenFX

Copy `crates/openfx-plugin/build/ExampleEffect.ofx.bundle/` to your OFX host's plugins directory:
- **DaVinci Resolve**: `C:\ProgramData\Blackmagic Design\DaVinci Resolve\Support\OFXPlugins\`
- **Natron**: `C:\Program Files\Common Files\OFX\Plugins\`

### After Effects / Premiere Pro

Copy the built `.aex` to:
- `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\`

The plugin appears as **"Example Effect"** under the **"Example"** category.

## Writing Tests

### Testing the effect library (pure Rust)

```rust
// tests/effect_tests.rs
#[test]
fn my_effect_works() {
    let effect = ExampleEffect::default();
    let src = create_test_image(1920, 1080);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, 1920, 1080);
    // assert on expected output...
}
```

### Testing plugin builds (compile-check)

The `ae-plugin/tests/` and `openfx-plugin/tests/` verify that cdylibs compile.
Full integration testing requires loading the plugin in the actual host application.

## zzzFX Effects

The `zzzfx-*` crates provide a family of custom video effects:

| Effect | Crate | Description |
|--------|-------|-------------|
| zzzFX Stroke | `zzzfx-core` | Alpha-channel stroke with distance transform |
| zzzFX Repeater | `zzzfx-core` | Keyframe-driven time-offset compositor |
| zzzFX Sprite Sheet | `zzzfx-core` | Grid-based sprite sheet reader with animation |
| zzzFX ASS Subtitle | `zzzfx-core` | ASS/SSA subtitle renderer |
| zzzFX ASCII Art | `zzzfx-core` | Luminance-to-character-glyph mapping |
| zzzFX Pixel Art Style | `zzzfx-core` | Color quantization in pixel blocks with dithering + grid |

### Building zzzFX

```bash
# OpenFX plugin (all 6 effects in one .ofx bundle)
cargo xtask build-zzzfx-ofx-plugin

# AE plugin (build one effect at a time via feature flag)
cargo build -p zzzfx-ae-plugin --features effect-pixel-art
```

### GPU Acceleration (Pixel Art Style)

The Pixel Art Style effect supports GPU-accelerated rendering via wgpu compute shaders.
GPU is used automatically when available; CPU fallback is transparent on failure.

**Feature flags:**

| Flag | Default | Effect |
|------|---------|--------|
| `gpu` | on | Enable GPU compute path (wgpu) |

**Disable GPU** (pure CPU build):

```bash
cargo build -p zzzfx-ae-plugin --features effect-pixel-art --no-default-features
```

**System requirements for GPU path:**
- Any GPU with Vulkan, Metal, DX12, or WebGPU support
- Tested on NVIDIA (Vulkan/DX12), AMD (Vulkan), Intel Arc (Vulkan)

The GPU path caches buffers and pipelines across frames (no per-frame allocation),
and falls back to CPU if:
- Floyd-Steinberg dithering is selected (serial algorithm)
- GPU adapter is unavailable
- GPU device is lost at runtime

## Dependencies

- **AE Plugin**: Requires the `after-effects` and `pipl` crates (git dependency)
- **OFX Plugin**: Requires `bindgen` + `libclang` for C header generation, and the OpenFX SDK (git submodule)
- Both plugins depend on `example-effect` (shared parameter + render library)

## License

MIT OR ISC OR Apache-2.0
