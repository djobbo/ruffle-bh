// This lint is helpful, but right now we have too many instances of it.
// TODO: Remove this once all instances are fixed.
#![allow(clippy::needless_pass_by_ref_mut)]
// This lint is good in theory, but in AVMs we often need to do `let x = args.get(0); let y = args.get(1);` etc.
// It'd make those much less readable and consistent.
#![allow(clippy::get_first)]

#[macro_use]
mod display_object;
pub use display_object::{StageAlign, StageDisplayState, StageScaleMode};

#[macro_use]
extern crate num_derive;

#[macro_use]
mod avm1;
mod avm2;
mod avm_rng;
mod binary_data;
pub mod bitmap;
mod character;
pub mod context;
pub mod context_menu;
mod drawing;
mod ecma_conversions;
pub mod events;
pub mod focus_tracker;
pub mod font;
mod frame_lifecycle;
pub mod fte;
mod html;
mod input;
mod library;
pub mod limits;
pub mod loader;
mod local_connection;
mod net_connection;
mod orphan_manager;
pub mod pixel_bender;
mod player;
mod prelude;
pub mod socket;
pub mod swz;
mod streams;pub mod string;
mod system_properties;
pub mod tag_utils;
mod tessellation_cache;
pub mod timer;
mod types;
mod vminterface;

pub mod backend;
pub mod compatibility_rules;
pub mod config;
#[cfg(feature = "egui")]
pub mod debug_ui;
pub mod external;
pub mod i18n;
pub mod stub;

pub use context_menu::ContextMenuItem;
pub use events::PlayerEvent;
pub use indexmap;
pub use loader::LoadBehavior;
pub use player::{Player, PlayerBuilder, PlayerMode, PlayerRuntime, StaticCallstack};
pub use ruffle_common::duration::FloatDuration;
pub use ruffle_render::backend::ViewportDimensions;
pub use swf;
pub use swf::Color;
pub use ttf_parser;

/// Monotonic frame counter for the headless replay exporter. The native
/// streamer increments this once per `Player::run_frame`; patched debug draws
/// include it in their dumps.
pub static DUMP_FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn advance_dump_frame() -> u64 {
    DUMP_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub fn reset_dump_frame() {
    DUMP_FRAME.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Deterministic `Math.random()` stream for the replay oracle.
///
/// `RUFFLE_FIXED_RANDOM_SEQUENCE=1` anchors the stream to a call counter that
/// the replay loader resets, so load-time UI draws cannot shift it.
pub static FIXED_RANDOM_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn reset_fixed_random() {
    FIXED_RANDOM_COUNTER.store(0, std::sync::atomic::Ordering::Relaxed);
}

pub fn next_fixed_random() -> f64 {
    let n = FIXED_RANDOM_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mixed = n
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((mixed >> 11) as f64) / ((1u64 << 53) as f64)
}

#[cfg(target_family = "wasm")]
mod wasm_replay {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    pub static HEADLESS: AtomicBool = AtomicBool::new(false);
    pub static REPLAY: Mutex<Option<(String, Vec<u8>)>> = Mutex::new(None);
    pub static STATES: Mutex<Vec<String>> = Mutex::new(Vec::new());
    pub static BOXES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    pub fn take_replay() -> Option<(String, Vec<u8>)> {
        REPLAY.lock().ok().and_then(|mut replay| replay.take())
    }

    pub fn counts() -> (usize, usize) {
        let states = STATES.lock().map(|lines| lines.len()).unwrap_or(0);
        let boxes = BOXES.lock().map(|lines| lines.len()).unwrap_or(0);
        (states, boxes)
    }

    pub fn take_dump() -> (Vec<String>, Vec<String>) {
        let states = STATES
            .lock()
            .map(|mut lines| std::mem::take(&mut *lines))
            .unwrap_or_default();
        let boxes = BOXES
            .lock()
            .map(|mut lines| std::mem::take(&mut *lines))
            .unwrap_or_default();
        (states, boxes)
    }

    pub fn headless() -> bool {
        HEADLESS.load(Ordering::Relaxed)
    }
}

#[cfg(target_family = "wasm")]
pub fn set_headless_mode(enabled: bool) {
    wasm_replay::HEADLESS.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_family = "wasm")]
pub fn headless_mode() -> bool {
    wasm_replay::headless()
}

#[cfg(target_family = "wasm")]
pub fn set_replay_bytes(name: String, bytes: Vec<u8>) {
    if let Ok(mut replay) = wasm_replay::REPLAY.lock() {
        *replay = Some((name, bytes));
    }
}

#[cfg(target_family = "wasm")]
pub fn has_replay_bytes() -> bool {
    wasm_replay::REPLAY
        .lock()
        .map(|replay| replay.is_some())
        .unwrap_or(false)
}

#[cfg(target_family = "wasm")]
pub fn take_replay_bytes() -> Option<(String, Vec<u8>)> {
    wasm_replay::take_replay()
}

#[cfg(target_family = "wasm")]
pub fn push_state_line(line: String) {
    if let Ok(mut lines) = wasm_replay::STATES.lock() {
        lines.push(line);
    }
}

#[cfg(target_family = "wasm")]
pub fn push_box_line(line: String) {
    if let Ok(mut lines) = wasm_replay::BOXES.lock() {
        lines.push(line);
    }
}

#[cfg(target_family = "wasm")]
pub fn replay_dump_counts() -> (usize, usize) {
    wasm_replay::counts()
}

#[cfg(target_family = "wasm")]
pub fn take_replay_dump() -> (Vec<String>, Vec<String>) {
    wasm_replay::take_dump()
}

/// The newest Flash Player version known to Ruffle.
pub const NEWEST_PLAYER_VERSION: u8 = 51;

/// The default Flash Player version that Ruffle will emulate.
pub const DEFAULT_PLAYER_VERSION: u8 = 32;

/// Path to the raw playerglobal ABC, for use as an `asc.jar` import library.
pub const PLAYERGLOBAL_ABC_PATH: &str = env!("RUFFLE_PLAYERGLOBAL_ABC_PATH");
