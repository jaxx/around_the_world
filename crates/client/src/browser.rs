//! Thin wrappers over browser APIs. Only call these in the browser.

use crate::store::{KeyValue, Store};

/// `window.localStorage`; quietly does nothing where storage is unavailable
/// (e.g. blocked in private browsing).
pub struct LocalStorage;

impl KeyValue for LocalStorage {
    fn get(&self, key: &str) -> Option<String> {
        storage()?.get_item(key).ok().flatten()
    }

    fn set(&self, key: &str, value: &str) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(key, value);
        }
    }

    fn remove(&self, key: &str) {
        if let Some(storage) = storage() {
            let _ = storage.remove_item(key);
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn store() -> Store<LocalStorage> {
    Store(LocalStorage)
}

/// The path of the current page, e.g. `/r/K7MXQ2`.
pub fn current_path() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

/// Changes the address bar without reloading the page.
pub fn push_path(path: &str) {
    if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
        let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
    }
}

/// Puts `text` on the clipboard; `false` if the browser refused.
pub async fn copy_text(text: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let promise = window.navigator().clipboard().write_text(text);
    wasm_bindgen_futures::JsFuture::from(promise).await.is_ok()
}

/// Whether the player asked their system for less motion.
pub fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .is_some_and(|query| query.matches())
}
