#![recursion_limit = "1024"]
pub mod app;
pub mod app_state;
pub mod components;
pub mod config;
pub mod config_handlers;
pub mod db;
pub mod features;
pub mod pages;
pub mod validation;

#[cfg(feature = "ssr")]
pub mod session_store;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
