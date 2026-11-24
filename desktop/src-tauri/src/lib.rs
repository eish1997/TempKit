//! Versioned domain contracts. Desktop command wiring follows in R1–R4.
pub mod contracts;
pub mod store;
pub mod resources;
pub mod decode_worker;
pub mod migration;
pub mod tasks;
pub mod scenes;
pub mod recovery_export;
pub mod notes;
pub mod note_document;
pub mod ink_session;

#[cfg(windows)]
pub mod native_file_drag;

pub mod drag_exports;

pub mod clipboard_pixels;

#[cfg(windows)]
pub mod native_clipboard;

pub mod gateway_profiles;

#[cfg(windows)]
pub mod credential_vault;
#[cfg(windows)]
pub mod gateway_connections;

#[cfg(windows)]
pub mod gateway_discovery;

pub mod image_protocol;
#[cfg(windows)]
pub mod image_bindings;
#[cfg(windows)]
pub mod image_execution;
#[cfg(windows)]
pub mod image_budget;

#[cfg(all(windows,feature="desktop-diagnostics"))]
pub mod generation_fixture;

#[cfg(windows)]
pub mod image_storage;
#[cfg(windows)]
mod image_receipt;
#[cfg(windows)]
mod drag_archive;

#[cfg(test)]
mod image_batch_tests;

#[cfg(test)] mod image_cost_scope_tests;
#[cfg(windows)]
pub mod image_cost_rules;

#[cfg(windows)]
pub mod overlay_window;
