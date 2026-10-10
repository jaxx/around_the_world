//! Whole games over real WebSockets: every seat plays random legal moves as
//! soon as it is its turn, until the game is over.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::Ws;
use protocol::{ClientMsg, Joined, ServerMsg, Stage};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rules::{Color, Event, Game, Phase};
use server::registry::Limits;

/// A served room with `names.len()` seats, each connected and welcomed.
async fn table(names: &[&str]) -> (Vec<(Ws, Color)>, Joined) {
    let registry = common::registry(Limits::default());
    let host = registry.create(names[0], [10, 0, 0, 1].into()).unwrap();
    let mut seats = vec![host.clone()];
    for name in &names[1..] {
        seats.push(registry.join(&host.code.0, name).await.unwrap());
    }
    let addr: SocketAddr = common::serve(registry).await;

    let mut clients = Vec::new();
    for seat in &seats {
        let mut ws = Ws::open(addr, &seat.code.0).await;
        ws.send(&ClientMsg::Hello {
            token: seat.token.clone(),
        })
        .await;
        assert_eq!(
            ws.recv().await,
            Some(ServerMsg::Welcome { seat: seat.seat })
        );
        let color = ws
            .snapshot()
            .await
            .seats
            .iter()
            .find(|s| s.id == seat.seat)
            .unwrap()
            .color;
        clients.push((ws, color));
    }
    (clients, host)
}

/// What one seat saw by the end of the game.
struct Outcome {
    ws: Ws,
    /// Placements in the final snapshot.
    placements: Vec<Color>,
    /// Placements in the `GameOver` event.
    announced: Vec<Color>,
    /// `Moved` events seen, all colours.
    moves: usize,
}

/// Plays `me` until game over, acting on every snapshot where it is its turn.
async fn play(mut ws: Ws, me: Color, seed: u64) -> Outcome {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut announced = Vec::new();
    let mut moves = 0;
    loop {
        match ws
            .recv()
            .await
            .expect("the socket stays open during the game")
        {
            ServerMsg::Snapshot { room } => {
                let Stage::Game { game } = room.stage else {
                    continue;
                };
                if *game.phase() == Phase::GameOver {
                    let placements = game.placements().to_vec();
                    return Outcome {
                        ws,
                        placements,
                        announced,
                        moves,
                    };
                }
                if let Some(msg) = my_action(&game, me, &mut rng) {
                    ws.send(&msg).await;
                }
            }
            ServerMsg::Events { events } => {
                for event in events {
                    match event {
                        Event::GameOver { placements } => announced = placements,
                        Event::Moved { .. } => moves += 1,
                        _ => {}
                    }
                }
            }
            // A stale duplicate action; the next snapshot is already on its way.
            ServerMsg::Error { .. } | ServerMsg::Welcome { .. } => {}
        }
    }
}

fn my_action(game: &Game, me: Color, rng: &mut StdRng) -> Option<ClientMsg> {
    if game.current() != me {
        return None;
    }
    match game.phase() {
        Phase::ChoosingStarter { .. } | Phase::AwaitingRoll { .. } => Some(ClientMsg::Roll),
        Phase::AwaitingMove { .. } => {
            let moves = game.legal_moves();
            let choice = moves[rng.random_range(0..moves.len())];
            Some(ClientMsg::Move {
                piece: choice.piece,
            })
        }
        Phase::GameOver => None,
    }
}

/// Starts the game from the host's socket and plays it out on all seats.
async fn play_out(mut clients: Vec<(Ws, Color)>) -> Vec<(Outcome, Color)> {
    clients[0].0.send(&ClientMsg::Start).await;
    let tasks: Vec<_> = clients
        .into_iter()
        .enumerate()
        .map(|(i, (ws, color))| {
            tokio::spawn(async move { (play(ws, color, i as u64).await, color) })
        })
        .collect();
    let mut outcomes = Vec::new();
    for task in tasks {
        let outcome = tokio::time::timeout(Duration::from_secs(60), task)
            .await
            .expect("the game ends within a minute")
            .unwrap();
        outcomes.push(outcome);
    }
    outcomes
}

fn assert_consistent(outcomes: &[(Outcome, Color)]) {
    // The winner alone needs 4 pieces x 70 steps at no more than 6 a move.
    let at_least = 4 * 70 / 6;
    let placements = &outcomes[0].0.placements;
    let mut colors: Vec<Color> = outcomes.iter().map(|(_, c)| *c).collect();
    let mut placed = placements.clone();
    colors.sort();
    placed.sort();
    assert_eq!(placed, colors, "every colour is placed exactly once");
    for (outcome, color) in outcomes {
        assert_eq!(
            &outcome.placements, placements,
            "{color:?} saw other placements"
        );
        assert_eq!(
            &outcome.announced, placements,
            "{color:?} saw another GameOver event"
        );
        // Everyone but the last place finished all four pieces.
        assert!(
            outcome.moves >= at_least * (outcomes.len() - 1),
            "{color:?} saw only {} moves",
            outcome.moves
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_players_finish_a_game_and_start_a_rematch() {
    let (clients, _) = table(&["Mari", "Jüri"]).await;
    let mut outcomes = play_out(clients).await;
    assert_consistent(&outcomes);

    outcomes[0].0.ws.send(&ClientMsg::Rematch).await;
    for (outcome, _) in &mut outcomes {
        loop {
            match outcome.ws.recv().await.expect("still open") {
                ServerMsg::Snapshot { room } if room.stage == Stage::Lobby => {
                    assert_eq!(room.seats.len(), 2);
                    break;
                }
                _ => continue,
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn four_players_finish_a_game() {
    let (clients, _) = table(&["Mari", "Jüri", "Kati", "Mati"]).await;
    let outcomes = play_out(clients).await;
    assert_consistent(&outcomes);
    assert_eq!(outcomes[0].0.placements.len(), 4);
}
