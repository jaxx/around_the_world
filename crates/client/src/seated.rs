//! A room seen from a seat this browser holds: the live connection, the
//! lobby (ootesaal) and the game. Screens are swapped only when the stage
//! changes; inside them, memos keep updates to what actually changed.

use std::collections::VecDeque;

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use protocol::{ClientMsg, RoomCode, RoomView, SeatId, SeatView, Stage};
use rules::{Cell, Color, Die, Game, Phase, PieceId};

use crate::anim::{Beat, Timed, plan};
use crate::board::{Board, color_class};
use crate::browser;
use crate::game_view::{
    can_roll, marks_with, my_targets, piece_marks, pips, seconds_left, standings,
};
use crate::lobby::{can_start, choosable_colors, invite_url};
use crate::session::{Session, Status};
use crate::socket::{self, Outbox};
use crate::store::SavedSeat;
use crate::text;

/// Connects `saved` to its room. `lost` runs if the server no longer knows
/// the seat; `left` after leaving the lobby on purpose.
#[component]
pub fn Seated(
    code: RoomCode,
    saved: SavedSeat,
    lost: Callback<()>,
    left: Callback<()>,
) -> impl IntoView {
    let session = RwSignal::new(Session::default());
    let forget_code = code.clone();
    let outbox = socket::connect(code.clone(), saved.token, session, move || {
        browser::store().forget_seat(&forget_code);
        lost.run(());
    });

    let room = Memo::new(move |_| session.with(|s| s.room.clone()));
    let me = Memo::new(move |_| session.with(|s| s.me));
    let in_game = Memo::new(move |_| {
        room.with(|r| r.as_ref().map(|r| matches!(r.stage, Stage::Game { .. })))
    });

    let status = move || match session.with(|s| s.status.clone()) {
        Status::Connecting => Some(text::CONNECTING),
        Status::Retrying { .. } => Some(text::RECONNECTING),
        Status::Live | Status::Refused(_) => None,
    };
    let problem = move || session.with(|s| s.problem.map(text::error_text));

    let leave = {
        let outbox = outbox.clone();
        Callback::new(move |()| {
            outbox.send(ClientMsg::Leave);
            browser::store().forget_seat(&code);
            left.run(());
        })
    };

    view! {
        {move || status().map(|t| view! { <p class="status">{t}</p> })}
        {move || problem().map(|t| view! { <p class="problem" role="alert">{t}</p> })}
        {move || match in_game.get() {
            None => ().into_any(),
            Some(false) => view! { <Lobby room me outbox=outbox.clone() leave /> }.into_any(),
            Some(true) => view! { <GameScreen room me session outbox=outbox.clone() /> }.into_any(),
        }}
    }
}

/// The seat's own colour, once both the room and the seat are known.
fn my_color(room: &Option<RoomView>, me: Option<SeatId>) -> Option<Color> {
    let me = me?;
    room.as_ref()?
        .seats
        .iter()
        .find(|s| s.id == me)
        .map(|s| s.color)
}

#[component]
fn Lobby(
    room: Memo<Option<RoomView>>,
    me: Memo<Option<SeatId>>,
    outbox: Outbox,
    leave: Callback<()>,
) -> impl IntoView {
    let choosable = Memo::new(move |_| match (room.get(), me.get()) {
        (Some(room), Some(me)) => choosable_colors(&room, me),
        _ => Vec::new(),
    });
    let mine = Memo::new(move |_| my_color(&room.get(), me.get()));
    let hosting = Memo::new(move |_| room.with(|r| r.as_ref().map(|r| r.host)) == me.get());
    let startable = Memo::new(move |_| match (room.get(), me.get()) {
        (Some(room), Some(me)) => can_start(&room, me),
        _ => false,
    });
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    let link = room.with_untracked(|r| {
        r.as_ref()
            .map(|r| invite_url(&origin, &r.code))
            .unwrap_or_default()
    });

    let seats = move || {
        let Some(room) = room.get() else {
            return Vec::new();
        };
        let me = me.get();
        room.seats
            .iter()
            .map(|seat| {
                let tags = seat_tags(seat, &room, me, None, None);
                seat_row(seat, tags)
            })
            .collect::<Vec<_>>()
    };

    let colors = Color::ALL
        .into_iter()
        .map(|color| {
            let outbox = outbox.clone();
            let class = format!("color {}", color_class(color));
            view! {
                <button
                    type="button"
                    class=class
                    aria-pressed=move || (mine.get() == Some(color)).to_string()
                    disabled=move || !choosable.get().contains(&color)
                    on:click=move |_| outbox.send(ClientMsg::ChooseColor { color })
                >
                    {text::color_name(color)}
                </button>
            }
        })
        .collect_view();

    let copied = RwSignal::new(false);
    let copy_link = link.clone();
    let copy = move |_| {
        let link = copy_link.clone();
        spawn_local(async move {
            if browser::copy_text(&link).await {
                copied.set(true);
            }
        });
    };

    let start_outbox = outbox.clone();
    let start_area = move || {
        if hosting.get() {
            let outbox = start_outbox.clone();
            view! {
                <button
                    type="button"
                    class="start"
                    disabled=move || !startable.get()
                    on:click=move |_| outbox.send(ClientMsg::Start)
                >
                    {text::START}
                </button>
                {move || (!startable.get()).then(|| view! { <p class="hint">{text::NEED_PLAYERS}</p> })}
            }
            .into_any()
        } else {
            view! { <p class="hint">{text::WAITING_FOR_HOST}</p> }.into_any()
        }
    };

    view! {
        <section class="card lobby">
            <h2>{text::LOBBY}</h2>
            <h3>{text::PLAYERS}</h3>
            <ul class="seats">{seats}</ul>
            <h3>{text::YOUR_COLOR}</h3>
            <div class="colors">{colors}</div>
            <h3>{text::INVITE}</h3>
            <div class="invite">
                <input type="text" readonly value=link />
                <button type="button" on:click=copy>
                    {move || if copied.get() { text::COPIED } else { text::COPY }}
                </button>
            </div>
            {start_area}
            <button type="button" class="leave" on:click=move |_| leave.run(())>
                {text::LEAVE}
            </button>
        </section>
    }
}

#[component]
fn GameScreen(
    room: Memo<Option<RoomView>>,
    me: Memo<Option<SeatId>>,
    session: RwSignal<Session>,
    outbox: Outbox,
) -> impl IntoView {
    let game = Memo::new(move |_| {
        room.with(|r| match r.as_ref().map(|r| &r.stage) {
            Some(Stage::Game { game }) => Some(game.clone()),
            _ => None,
        })
    });
    let mine = Memo::new(move |_| my_color(&room.get(), me.get()));
    let show = animation(room, session);
    let animating = Memo::new(move |_| show.base.with(Option::is_some));

    // While an animation plays, the board shows the game it started from
    // with the moving pieces where the animation has them.
    let marks = Signal::derive(move || match show.base.get() {
        Some(base) => show.overrides.with(|o| marks_with(&base, o)),
        None => game.with(|g| g.as_ref().map(piece_marks).unwrap_or_default()),
    });
    let targets = Signal::derive(move || match (game.get(), mine.get(), animating.get()) {
        (Some(game), Some(color), false) => my_targets(&game, color),
        _ => Vec::new(),
    });
    let rollable = Memo::new(move |_| match (game.get(), mine.get(), animating.get()) {
        (Some(game), Some(color), false) => can_roll(&game, color),
        _ => false,
    });

    let roll_outbox = outbox.clone();
    let roll = move || {
        if rollable.get_untracked() {
            roll_outbox.send(ClientMsg::Roll);
        }
    };
    let roll_key = roll.clone();
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        let typing = ev
            .target()
            .and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(t).ok())
            .is_some();
        if ev.code() == "Space" && !typing && !ev.repeat() {
            ev.prevent_default();
            roll_key();
        }
    });
    on_cleanup(move || keys.remove());

    let pick_outbox = outbox.clone();
    let on_pick = Callback::new(move |piece| pick_outbox.send(ClientMsg::Move { piece }));

    let countdown = countdown(room);

    // Game over is shown once the last move has been played out.
    let over = Memo::new(move |_| {
        !animating.get() && game.with(|g| g.as_ref().is_some_and(|g| *g.phase() == Phase::GameOver))
    });
    let hosting = Memo::new(move |_| room.with(|r| r.as_ref().map(|r| r.host)) == me.get());
    let rematch_outbox = outbox.clone();
    let results = move || {
        let (Some(room), Some(game)) = (room.get(), game.get()) else {
            return Vec::new();
        };
        standings(&room, &game)
            .into_iter()
            .map(|(place, name, color)| {
                let class = format!("seat {}", color_class(color));
                view! {
                    <li class=class>
                        <span class="dot" aria-hidden="true"></span>
                        <span class="name">{format!("{place}. {name}")}</span>
                    </li>
                }
            })
            .collect::<Vec<_>>()
    };
    let finale = move || {
        let next = if hosting.get() {
            let outbox = rematch_outbox.clone();
            view! {
                <button type="button" class="rematch" on:click=move |_| outbox.send(ClientMsg::Rematch)>
                    {text::REMATCH}
                </button>
            }
            .into_any()
        } else {
            view! { <p class="hint">{text::WAITING_FOR_REMATCH}</p> }.into_any()
        };
        view! {
            <h3>{text::RESULTS}</h3>
            <ol class="results">{results}</ol>
            {next}
        }
    };

    let turn_line = move || {
        let (Some(room), Some(game)) = (room.get(), game.get()) else {
            return String::new();
        };
        if over.get() {
            return text::GAME_OVER.to_string();
        }
        let current = game.current();
        if mine.get() == Some(current) {
            return match game.phase() {
                Phase::ChoosingStarter { .. } => text::STARTER_ROLL.to_string(),
                Phase::AwaitingMove { .. } => text::YOUR_TURN_MOVE.to_string(),
                _ => text::YOUR_TURN_ROLL.to_string(),
            };
        }
        let name = room
            .seats
            .iter()
            .find(|s| s.color == current)
            .map(|s| s.nickname.clone())
            .unwrap_or_default();
        text::turn_of(&name)
    };

    let die = move || {
        let (color, die) = show.roll.get()?;
        let dots = pips(die.value())
            .into_iter()
            .map(|(c, r)| {
                let (x, y) = (14.0 + 18.0 * c as f32, 14.0 + 18.0 * r as f32);
                view! { <circle cx=x cy=y r="5" /> }
            })
            .collect_view();
        let rolling = if show.rolling.get() { " rolling" } else { "" };
        let class = format!("die {}{rolling}", color_class(color));
        Some(view! {
            <svg class=class viewBox="0 0 64 64" role="img" aria-label=format!("{} {}", text::DIE, die.value())>
                <rect x="2" y="2" width="60" height="60" rx="10" />
                {dots}
            </svg>
        })
    };

    let seats = move || {
        let (Some(room), Some(game)) = (room.get(), game.get()) else {
            return Vec::new();
        };
        let me = me.get();
        room.seats
            .iter()
            .map(|seat| {
                let tags = seat_tags(seat, &room, me, Some(&game), Some(game.current()));
                seat_row(seat, tags)
            })
            .collect::<Vec<_>>()
    };

    view! {
        <section class="game">
            <div class="board-wrap">
                <Board marks targets mine=mine.into() on_pick />
            </div>
            <aside class="panel card">
                <p class="turn" aria-live="polite">{turn_line}</p>
                <Show
                    when=move || !over.get()
                    fallback=finale
                >
                    {move || countdown.get().map(|s| view! { <p class="countdown">{s} " " {text::SECONDS}</p> })}
                    <div class="dice-row">
                        {die}
                        <button
                            type="button"
                            class="roll"
                            disabled=move || !rollable.get()
                            on:click={
                                let roll = roll.clone();
                                move |_| roll()
                            }
                        >
                            {text::ROLL}
                        </button>
                    </div>
                </Show>
                <h3>{text::PLAYERS}</h3>
                <ul class="seats">{seats}</ul>
                <h3>{text::LOG}</h3>
                <ol class="log" aria-live="polite">
                    {move || {
                        show.log.get().into_iter().map(|line| view! { <li>{line}</li> }).collect_view()
                    }}
                </ol>
            </aside>
        </section>
    }
}

/// Lines kept in the event log.
const LOG_LINES: usize = 5;

/// What the game screen shows while catching up with events.
#[derive(Clone, Copy)]
struct Show {
    /// The game the running animation started from; `None` when idle.
    base: RwSignal<Option<Game>>,
    /// Where moving pieces are, mid-animation.
    overrides: RwSignal<Vec<(Color, PieceId, Cell)>>,
    /// The die as last shown, and whether it is rolling right now.
    roll: RwSignal<Option<(Color, Die)>>,
    rolling: RwSignal<bool>,
    /// The latest event lines, newest first.
    log: RwSignal<Vec<String>>,
}

/// Plays every event batch the session receives as beats: the die, hops,
/// flights home and log lines, one after another.
fn animation(room: Memo<Option<RoomView>>, session: RwSignal<Session>) -> Show {
    let show = Show {
        base: RwSignal::new(None),
        overrides: RwSignal::new(Vec::new()),
        roll: RwSignal::new(session.with_untracked(|s| s.last_roll)),
        rolling: RwSignal::new(false),
        log: RwSignal::new(Vec::new()),
    };
    let queue = StoredValue::new(VecDeque::<Timed>::new());
    let running = StoredValue::new(false);

    Effect::new(move |_| {
        session.track();
        // Taking batches must not notify, or this effect would wake itself.
        let batches = session
            .try_update_untracked(Session::take_batches)
            .unwrap_or_default();
        let names: Vec<(Color, String)> = room.with_untracked(|r| {
            r.as_ref()
                .map(|r| {
                    r.seats
                        .iter()
                        .map(|s| (s.color, s.nickname.clone()))
                        .collect()
                })
                .unwrap_or_default()
        });
        let name = |color: Color| {
            names
                .iter()
                .find(|(c, _)| *c == color)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| text::color_name(color).to_string())
        };
        for batch in batches {
            let Some(before) = batch.before else {
                continue;
            };
            let beats = plan(&before, &batch.events, &name);
            if beats.is_empty() {
                continue;
            }
            if show.base.get_untracked().is_none() {
                show.base.set(Some(before));
            }
            queue.update_value(|q| q.extend(beats));
        }
        if !running.get_value() && queue.with_value(|q| !q.is_empty()) {
            running.set_value(true);
            spawn_local(play(show, queue, running));
        }
    });
    show
}

async fn play(show: Show, queue: StoredValue<VecDeque<Timed>>, running: StoredValue<bool>) {
    // With reduced motion, beats still happen in order but without pauses.
    let still = browser::reduced_motion();
    while let Some(Timed { beat, ms }) = queue.try_update_value(VecDeque::pop_front).flatten() {
        show.rolling.try_set(false);
        match beat {
            Beat::Roll { color, die } => {
                show.roll.try_set(Some((color, die)));
                show.rolling.try_set(true);
            }
            Beat::Place { color, piece, cell } => {
                show.overrides.try_update(|o| {
                    o.retain(|&(c, p, _)| (c, p) != (color, piece));
                    o.push((color, piece, cell));
                });
            }
            Beat::Log(line) => {
                show.log.try_update(|log| {
                    log.insert(0, line);
                    log.truncate(LOG_LINES);
                });
            }
        }
        if ms > 0 && !still {
            TimeoutFuture::new(ms).await;
        }
    }
    show.rolling.try_set(false);
    show.overrides.try_set(Vec::new());
    show.base.try_set(None);
    running.try_set_value(false);
}

/// Seconds left on the turn timer, counted down in the browser from each
/// snapshot's `turn_ms_left`.
fn countdown(room: Memo<Option<RoomView>>) -> Signal<Option<u32>> {
    let anchor = RwSignal::new((None::<u32>, js_sys::Date::now()));
    Effect::new(move |_| {
        let ms = room.with(|r| r.as_ref().and_then(|r| r.turn_ms_left));
        anchor.set((ms, js_sys::Date::now()));
    });
    let now = RwSignal::new(js_sys::Date::now());
    let ticker = set_interval_with_handle(
        move || now.set(js_sys::Date::now()),
        std::time::Duration::from_millis(250),
    )
    .ok();
    on_cleanup(move || {
        if let Some(ticker) = ticker {
            ticker.clear();
        }
    });
    Signal::derive(move || {
        let (ms, at) = anchor.get();
        let elapsed = (now.get() - at).max(0.0) as u32;
        ms.map(|ms| seconds_left(ms, elapsed))
    })
}

/// The little labels after a player's name.
fn seat_tags(
    seat: &SeatView,
    room: &RoomView,
    me: Option<SeatId>,
    game: Option<&Game>,
    current: Option<Color>,
) -> Vec<(&'static str, String)> {
    let mut tags = Vec::new();
    if Some(seat.id) == me {
        tags.push(("tag", text::YOU.to_string()));
    }
    if game.is_none() && seat.id == room.host {
        tags.push(("tag", text::HOST.to_string()));
    }
    if current == Some(seat.color) && game.is_some_and(|g| *g.phase() != Phase::GameOver) {
        tags.push(("tag turn", text::TURN.to_string()));
    }
    if let Some(n) = game.and_then(|g| g.placements().iter().position(|&c| c == seat.color)) {
        tags.push(("tag place", text::place(n + 1)));
    }
    if seat.away {
        tags.push(("tag gone", text::AWAY.to_string()));
    }
    if !seat.connected {
        tags.push(("tag gone", text::DISCONNECTED.to_string()));
    }
    tags
}

fn seat_row(seat: &SeatView, tags: Vec<(&'static str, String)>) -> impl IntoView + use<> {
    let class = format!("seat {}", color_class(seat.color));
    let tags = tags
        .into_iter()
        .map(|(class, label)| view! { <span class=class>{label}</span> })
        .collect_view();
    view! {
        <li class=class>
            <span class="dot" aria-hidden="true"></span>
            <span class="name">{seat.nickname.clone()}</span>
            {tags}
        </li>
    }
}
