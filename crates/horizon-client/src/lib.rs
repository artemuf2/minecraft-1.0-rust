//! Horizon-Lite: клиент Minecraft 1.0 на glow + glutin.
#![allow(unsafe_code)]

pub use horizon_client_core::Result;

// Наш glow-путь
pub mod assets;
pub mod gfx;
pub mod mesher;

// Модули Horizon 26.3 без wgpu, нужные для мира
pub mod biome;
pub mod camera;
pub mod light;
pub use horizon_items::blocks;
pub mod chunk;
pub mod chunk_map;
pub mod fast_hash;
pub mod spans;