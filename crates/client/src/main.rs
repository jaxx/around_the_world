//! Entry point: Trunk builds this into WebAssembly and mounts the app.

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(client::app::App);
}
