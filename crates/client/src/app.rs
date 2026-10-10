//! The root component: picks the page from the address and keeps the two
//! in step, including the browser's back button.

use leptos::prelude::*;

use crate::browser;
use crate::pages::{Home, Room};
use crate::route::{self, Route};
use crate::text;

#[component]
pub fn App() -> impl IntoView {
    let route = RwSignal::new(route::parse(&browser::current_path()));
    let navigate = Callback::new(move |to: Route| {
        let path = match &to {
            Route::Home => "/".to_string(),
            Route::Room(code) => route::room_path(code),
        };
        browser::push_path(&path);
        route.set(to);
    });
    let back = window_event_listener(leptos::ev::popstate, move |_| {
        route.set(route::parse(&browser::current_path()));
    });
    on_cleanup(move || back.remove());

    let go_home = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();
        navigate.run(Route::Home);
    };
    view! {
        <main>
            <h1><a href="/" on:click=go_home>{text::TITLE}</a></h1>
            {move || match route.get() {
                Route::Home => view! { <Home navigate /> }.into_any(),
                Route::Room(code) => view! { <Room code navigate /> }.into_any(),
            }}
        </main>
    }
}
