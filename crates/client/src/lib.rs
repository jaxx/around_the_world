//! Browser client for «Reis ümber maailma». Pure, natively tested modules
//! (layout, route, store, text) sit beside thin Leptos components and
//! browser glue (api, browser).

pub mod anim;
pub mod api;
pub mod app;
pub mod board;
pub mod browser;
pub mod game_view;

pub mod layout;
pub mod lobby;
pub mod pages;
pub mod route;
pub mod seated;
pub mod session;
pub mod socket;
pub mod store;
pub mod text;
