//! The Little Fighter 2 v2.0a simulation, ported from `lf2.exe`.
//!
//! State is kept in byte images with the original's memory layout (see [`mem`]), so
//! that every tick can be compared byte for byte with the running original.
//! No platform dependencies; all arithmetic is plain IEEE f64 (`+ - * /`), which
//! matches the original's x87 code because it runs with 53-bit precision.

pub mod ai;
pub mod ai_projectile;
pub mod ai_specials;
pub mod battle;
pub mod camera;
pub mod carry;
pub mod catching;
pub mod collisions;
pub mod compare;
pub mod controls;
pub mod draw;
pub mod menu;
pub mod fields;
pub mod hits;
pub mod input;
pub mod mem;
pub mod net;
pub mod movement;
pub mod post_loop;
pub mod summary;
pub mod post_update;
pub mod replay;
pub mod rollback;
pub mod setup;
pub mod stage;
pub mod sound;
pub mod tick;
pub mod trace;
pub mod transform;
pub mod world;

pub use world::World;
