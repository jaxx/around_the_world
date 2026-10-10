//! The home page and a room's page.

use leptos::prelude::*;
use leptos::task::spawn_local;
use protocol::{ErrorCode, Joined, RoomCode};

use crate::api::{self, Failure};
use crate::browser;
use crate::route::Route;
use crate::seated::Seated;
use crate::store::SavedSeat;
use crate::text;

/// Create a room, or join one by its code.
#[component]
pub fn Home(navigate: Callback<Route>) -> impl IntoView {
    let nickname = RwSignal::new(browser::store().nickname().unwrap_or_default());
    let code = RwSignal::new(String::new());
    let problem = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);

    let enter = move |room: Option<RoomCode>| {
        busy.set(true);
        problem.set(None);
        let name = nickname.get_untracked();
        spawn_local(async move {
            let result = match &room {
                None => api::create_room(&name).await,
                Some(room) => api::join_room(room, &name).await,
            };
            busy.set(false);
            match result {
                Ok(joined) => {
                    remember(&joined, &name);
                    navigate.run(Route::Room(joined.code));
                }
                Err(failure) => problem.set(Some(failure_text(failure))),
            }
        });
    };
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        enter(None);
    };
    let join = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        enter(Some(RoomCode(
            code.get_untracked().trim().to_ascii_uppercase(),
        )));
    };

    view! {
        <section class="home">
            <label class="field">
                <span>{text::NICKNAME}</span>
                <input type="text" maxlength="20" autocomplete="nickname" bind:value=nickname />
            </label>
            <form class="card" on:submit=create>
                <h2>{text::CREATE_ROOM}</h2>
                <button type="submit" disabled=move || busy.get()>{text::CREATE}</button>
            </form>
            <form class="card" on:submit=join>
                <h2>{text::JOIN_WITH_CODE}</h2>
                <label class="field">
                    <span>{text::ROOM_CODE}</span>
                    <input type="text" maxlength="16" autocapitalize="characters" bind:value=code />
                </label>
                <button type="submit" disabled=move || busy.get()>{text::JOIN}</button>
            </form>
            <Problem problem />
        </section>
    }
}

/// A room's page: join it if this browser has no seat there yet, otherwise
/// connect to it.
#[component]
pub fn Room(code: RoomCode, navigate: Callback<Route>) -> impl IntoView {
    let seat = RwSignal::new(browser::store().seat(&code));
    let nickname = RwSignal::new(browser::store().nickname().unwrap_or_default());
    let problem = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);

    let room = code.clone();
    let join = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        problem.set(None);
        let name = nickname.get_untracked();
        let room = room.clone();
        spawn_local(async move {
            let result = api::join_room(&room, &name).await;
            busy.set(false);
            match result {
                Ok(joined) => {
                    remember(&joined, &name);
                    seat.set(Some(saved(&joined)));
                }
                Err(failure) => problem.set(Some(failure_text(failure))),
            }
        });
    };
    let lost = Callback::new(move |()| {
        seat.set(None);
        problem.set(Some(text::error_text(ErrorCode::UnknownToken)));
    });
    let left = Callback::new(move |()| navigate.run(Route::Home));

    let shown_code = code.0.clone();
    view! {
        <section class="room">
            <p class="room-code">{text::ROOM} " " <strong>{shown_code}</strong></p>
            {move || match seat.get() {
                Some(saved) => view! { <Seated code=code.clone() saved lost left /> }.into_any(),
                None => {
                    let join = join.clone();
                    view! {
                        <form class="card join" on:submit=join>
                            <h2>{text::JOIN_ROOM}</h2>
                            <label class="field">
                                <span>{text::NICKNAME}</span>
                                <input type="text" maxlength="20" autocomplete="nickname" bind:value=nickname />
                            </label>
                            <button type="submit" disabled=move || busy.get()>{text::JOIN}</button>
                        </form>
                        <Problem problem />
                    }
                    .into_any()
                }
            }}
        </section>
    }
}

#[component]
fn Problem(problem: RwSignal<Option<&'static str>>) -> impl IntoView {
    move || {
        problem
            .get()
            .map(|text| view! { <p class="problem" role="alert">{text}</p> })
    }
}

fn remember(joined: &Joined, nickname: &str) {
    let store = browser::store();
    store.save_seat(&joined.code, &saved(joined));
    store.save_nickname(nickname.trim());
}

fn saved(joined: &Joined) -> SavedSeat {
    SavedSeat {
        token: joined.token.clone(),
        seat: joined.seat,
    }
}

fn failure_text(failure: Failure) -> &'static str {
    match failure {
        Failure::Refused(code) => text::error_text(code),
        Failure::Network => text::NETWORK_ERROR,
    }
}
