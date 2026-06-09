use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Shared GPU device — one wgpu instance per process, shared by all effects.
// This avoids crashes from creating multiple GPU backends inside plugin hosts
// (e.g. VEGAS Pro) that already manage their own GPU contexts.
// ---------------------------------------------------------------------------

pub(crate) static SHARED_GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);
static SHARED_DEVICE: OnceLock<wgpu::Device> = OnceLock::new();
static SHARED_QUEUE: OnceLock<wgpu::Queue> = OnceLock::new();

pub fn get_or_init_shared_device() -> Result<(&'static wgpu::Device, &'static wgpu::Queue), String> {
    if let (Some(d), Some(q)) = (SHARED_DEVICE.get(), SHARED_QUEUE.get()) {
        return Ok((d, q));
    }

    if !SHARED_GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Err("GPU unavailable".to_string());
    }

    static INIT_LOCK: Mutex<()> = Mutex::new(());
    let _guard = INIT_LOCK.lock().map_err(|_| "init lock poisoned".to_string())?;

    if let (Some(d), Some(q)) = (SHARED_DEVICE.get(), SHARED_QUEUE.get()) {
        return Ok((d, q));
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .map_err(|e| {
        SHARED_GPU_AVAILABLE.store(false, Ordering::Relaxed);
        format!("adapter request failed: {e}")
    })?;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("zzzfx shared GPU"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: Default::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: Default::default(),
        },
    ))
    .map_err(|e| {
        SHARED_GPU_AVAILABLE.store(false, Ordering::Relaxed);
        format!("failed to create GPU device: {e}")
    })?;

    let _ = SHARED_DEVICE.set(device);
    let _ = SHARED_QUEUE.set(queue);

    Ok((
        SHARED_DEVICE
            .get()
            .ok_or_else(|| "device init race".to_string())?,
        SHARED_QUEUE
            .get()
            .ok_or_else(|| "queue init race".to_string())?,
    ))
}

/// Check if the shared GPU device is already initialized WITHOUT attempting to create one.
/// Safe to call from any context (including VEGAS Pro plugin host) — does not block,
/// does not create resources. Returns false if GPU init was never triggered.
pub fn is_shared_device_ready() -> bool {
    SHARED_GPU_AVAILABLE.load(Ordering::Relaxed)
        && SHARED_DEVICE.get().is_some()
        && SHARED_QUEUE.get().is_some()
}

// ---------------------------------------------------------------------------
// Shared GPU readback helper
// ---------------------------------------------------------------------------

/// Blocking GPU staging-buffer readback with timeout.
/// Copies `image_size` bytes from `staging_buf` into `dst`.
pub fn blocking_readback(
    device: &wgpu::Device,
    staging_buf: &wgpu::Buffer,
    image_size: u64,
    dst: &mut [u8],
) -> Result<(), String> {
    let staging_slice = staging_buf.slice(..image_size);
    let (tx, rx) = std::sync::mpsc::channel();
    staging_slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_millis(100)),
    });
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => {
            let mapped = staging_slice.get_mapped_range();
            let len = (image_size as usize).min(dst.len());
            dst[..len].copy_from_slice(&mapped[..len]);
            drop(mapped);
            staging_buf.unmap();
            Ok(())
        }
        _ => {
            staging_buf.unmap();
            Err("staging map failed".to_string())
        }
    }
}
