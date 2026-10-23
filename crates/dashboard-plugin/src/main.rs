#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod entrypoints;
#[cfg(any(target_arch = "wasm32", test))]
mod events;
#[cfg(target_arch = "wasm32")]
mod plugin;
#[cfg(any(target_arch = "wasm32", test))]
mod render;

#[cfg(target_arch = "wasm32")]
fn main() {
    entrypoints::initialize();
}
