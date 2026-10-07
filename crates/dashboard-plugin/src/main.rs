#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
use zellij_tile::prelude::*;

#[cfg(target_arch = "wasm32")]
mod plugin;

#[cfg(target_arch = "wasm32")]
use plugin::Dashboard;

#[cfg(target_arch = "wasm32")]
register_plugin!(Dashboard);
