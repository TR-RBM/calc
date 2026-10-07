# calc-exec-wgpu

## What it does

The backend for a hardware GPU device over `wgpu`. Nothing here is registered in a shipped build.

- `WgpuDevice::new(options)` asks `wgpu` for a Vulkan adapter with high performance preference, refuses a CPU adapter unless `WgpuDeviceOptions::allow_cpu_adapter` is set, and requests a device with no extra features and default limits. It returns `DeviceCreationError` naming why it failed. `description()` gives the adapter name, backend, driver and whether it is a CPU adapter, for the record's backend members.
- On a target where `wgpu` has no Vulkan backend, macOS among them, `WgpuDevice::new` answers `DeviceCreationError::NoVulkanBackend` without creating an instance, so a session there runs without a GPU instead of stopping.
- `WgpuDevice` implements `GpuDevice` of `calc-exec-gpu`. Buffers are storage buffers of little-endian `f32` or `u32` entries. `compile` emits the map and reduce WGSL of the kernel and builds one compute pipeline each. `dispatch` binds the pass's three buffers and a uniform with the element count and invocations, and dispatches workgroups of 64 over one or two dimensions. `read_buffer` copies into a staging buffer and maps it. Every `wgpu` call runs inside error scopes, so an out of memory, validation or internal error becomes a `DeviceError` instead of a panic.
- `block_on` waits on `wgpu`'s futures with the standard library's `Waker` and a park of the calling thread. The crate starts no thread and keeps `unsafe_code` at deny.
- `validate_shader(text)` validates WGSL with `wgpu::naga`, without an adapter.

No `wgpu` type appears in the public API.

## How to test

`cargo test -p calc-exec-wgpu` runs the unit tests on any machine. They cover workgroup counts, the byte round trip of special values, and shader validation. The device creation test accepts a named hardware adapter or a creation error with a reason, so it passes on this machine without a GPU.

`host_conformance` is a test target with its own `main` that is not run by `cargo test`. Build it with `cargo test -p calc-exec-wgpu --test host_conformance --no-run --release` and run the printed binary on a machine with a GPU. It checks every `calc-conformance` case the GPU backend supports on `WgpuDevice` against `calc-exec-cpu`: the operation cases, the contraction and constant chain cases, all reduce cases, the iteration cases, the escape-time equivalences at 8 and 256 iterations, and the escape-time expected counts. It also times a 1920x1080 escape-time picture at 256 and 4096 iterations on the GPU and on one CPU thread, and compares the two bit for bit. It prints one line per case and a summary. It exits 0 when nothing failed, 1 on any failure, and 2 when no hardware device exists, never with a pass.
