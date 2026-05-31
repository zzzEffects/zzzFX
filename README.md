# zzzFX — Open-Source Video Effect Plugins

zzzFX is an open-source and free series of video effect plugins for After Effects, Premiere and OpenFX hosts (such as VEGAS Pro, DaVinci Resolve, etc.).

The plugin framework is based on [VideoFX-rs](https://github.com/zzzEffect/VideoFX-rs) (MIT licensed), with modifications and extensions for zzzFX's effect system.

## License

zzzFX is licensed under the [GNU General Public License v3.0 or later](https://www.gnu.org/licenses/gpl-3.0.html) (GPL-3.0-or-later).

The upstream VideoFX-rs plugin framework is available under the MIT license.

## Structure

```
zzzFX/
├── crates/
│   ├── zzzfx/                  # Shared effect library (core)
│   ├── macros/                 # Proc macro: #[derive(FullSettings)]
│   ├── ae-plugin/              # After Effects / Premiere plugin (cdylib)
│   └── openfx-plugin/          # OpenFX plugin (cdylib)
│       └── vendor/
│           └── openfx/         # OpenFX SDK (git submodule)
└── xtask/                      # Build & bundle helper (cargo xtask)
```

## Supported Hosts

| Host | Crate | Build Command |
|------|-------|---------------|
| After Effects / Premiere | `ae-plugin` | `cargo build -p zzzfx-ae-plugin --release` |
| OpenFX (Resolve, Natron, VEGAS, etc.) | `openfx-plugin` | `cargo xtask build-ofx-plugin --release` |

## Effects

All effects are implemented in `zzzfx` and bundled into both the AE and OFX plugins:

| Effect | Description |
|--------|-------------|
| Ambient Light | Edge-based ambient light with separated local/global blur for depth-aware glow |
| ASCII Art | Luminance-to-character-glyph mapping with configurable charset and color modes |
| ASS Subtitle | ASS/SSA subtitle renderer with style parsing and overlay compositing |
| Cast Shadow | Directional drop shadow with blur, scale, and color controls, GPU-accelerated |
| Chroma Key | BT.601 YCbCr-based color keying with edge blur for clean matte extraction |
| Long Shadow | Extended directional shadow with configurable length, angle, opacity, and color |
| MIDI Display | MIDI file visualization as piano-roll note blocks with track filtering |
| Pixel Art Style | Color quantization in pixel blocks with ordered/Floyd-Steinberg dithering + grid |
| Repeater | Keyframe-driven time-offset layer compositor with blend modes |
| Sprite Sheet | Grid-based sprite sheet reader with animation, scaling, and rotation |
| Stroke | Alpha-channel stroke with distance transform, fill modes, and blend modes |
| SVG Display | SVG file renderer with system font loading, scaling, and cache |

## Building from Source

### Install Rust

The first step is to install the latest version of [Rust](https://www.rust-lang.org/). Even if you're using Linux and Rust is available in your package manager, the version may be too outdated to build zzzFX.

To obtain the latest stable version of Rust, install [rustup](https://rustup.rs/) and then run:

```bash
rustup install stable
```

You may need to close and reopen your terminal after this.

### Install rust-bindgen's requirements (OpenFX only)

If you want to build the OpenFX plugin, you'll need to install some dependencies for the `rust-bindgen` tool to work. On Windows, this means having [LLVM/clang](https://releases.llvm.org/) installed. On Linux, install `libclang-dev` via your package manager.

If you're not building the OpenFX plugin, you can ignore this part.

### Clone the repository

Make sure to include submodules when cloning the repository if you want the OpenFX plugin to build properly:

```bash
git clone --recurse-submodules https://github.com/zzzEffect/zzzFX.git
cd zzzFX
```

If you've already cloned the repository without submodules, you can initialize them via:

```bash
git submodule update --init --recursive
```

### Platform-specific instructions

After installing Rust and cloning the repository, the steps are platform-specific:

#### Windows

```bash
# Build the OpenFX plugin (output in crates/openfx-plugin/build/)
cargo xtask build-ofx-plugin --release

# Build the After Effects / Premiere plugin (output: target/release/zzzfx_ae_plugin.dll)
# Copy and rename to: C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\zzzfx-ae.aex
cargo build -p zzzfx-ae-plugin --release
```

#### macOS

```bash
# Build the OpenFX plugin (output in crates/openfx-plugin/build/)
cargo xtask build-ofx-plugin --macos-universal --release

# Build and bundle the After Effects plugin (output in the build/ folder)
cargo xtask macos-ae-plugin --macos-universal --release
```

#### Linux

```bash
# Build the OpenFX plugin (output in crates/openfx-plugin/build/)
cargo xtask build-ofx-plugin --release
```

## Testing with Real Hosts

### OpenFX

Copy the built `.ofx.bundle/` from `crates/openfx-plugin/build/` to your OFX host's plugins directory:

- **DaVinci Resolve**: `C:\ProgramData\Blackmagic Design\DaVinci Resolve\Support\OFXPlugins\`
- **Natron**: `C:\Program Files\Common Files\OFX\Plugins\`
- **VEGAS Pro**: `C:\Program Files\VEGAS\VEGAS Pro\OFX Video Plug-Ins\`

### After Effects / Premiere

Copy the built `.aex` (Windows) or `.plugin` bundle (macOS) to:

- **Windows**: `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\`
- **macOS**: `/Library/Application Support/Adobe/Common/Plug-ins/7.0/MediaCore/`

## Running Tests

```bash
cargo test --workspace
```

This runs:
- Settings tests: JSON round-trip, get/set fields, descriptor iteration
- Effect tests: passthrough correctness with various dimensions
- Plugin compile tests: verify cdylibs build successfully
- i18n tests: verify all translation keys have non-empty EN and zh_CN text

## GPU Acceleration

Several effects support GPU-accelerated rendering via wgpu compute shaders. GPU is used automatically when available; CPU fallback is transparent on failure.

**Feature flags:**

| Flag | Default | Effect |
|------|---------|--------|
| `gpu` | on | Enable GPU compute path (wgpu) |

**Disable GPU** (pure CPU build):

```bash
cargo build -p zzzfx-ae-plugin --release --no-default-features
```

**System requirements for GPU path:**
- Any GPU with Vulkan, Metal, DX12, or WebGPU support
- Tested on NVIDIA (Vulkan/DX12), AMD (Vulkan), Intel Arc (Vulkan)

The GPU path caches buffers and pipelines across frames (no per-frame allocation),
and falls back to CPU if:
- Floyd-Steinberg dithering is selected (serial algorithm)
- GPU adapter is unavailable
- GPU device is lost at runtime
