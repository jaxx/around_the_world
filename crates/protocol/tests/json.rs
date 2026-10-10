//! The exact JSON both sides speak. Changing a shape here breaks the client.

use protocol::{
    ApiError, ClientMsg, CreateRoom, ErrorCode, JoinRoom, Joined, RoomCode, RoomView, SeatId,
    SeatToken, SeatView, ServerMsg, Stage,
};
use rules::{Color, Game, PieceId, RuleError};
use serde_json::{Value, json};

fn client(json: Value) -> Result<ClientMsg, serde_json::Error> {
    serde_json::from_value(json)
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn client_messages_are_tagged_by_type() {
    assert_eq!(
        client(json!({"type": "hello", "token": "abc"})).unwrap(),
        ClientMsg::Hello {
            token: SeatToken("abc".into())
        }
    );
    assert_eq!(
        client(json!({"type": "choose_color", "color": "Green"})).unwrap(),
        ClientMsg::ChooseColor {
            color: Color::Green
        }
    );
    assert_eq!(client(json!({"type": "start"})).unwrap(), ClientMsg::Start);
    assert_eq!(client(json!({"type": "roll"})).unwrap(), ClientMsg::Roll);
    assert_eq!(
        client(json!({"type": "move", "piece": 2})).unwrap(),
        ClientMsg::Move { piece: PieceId(2) }
    );
    assert_eq!(client(json!({"type": "leave"})).unwrap(), ClientMsg::Leave);
    assert_eq!(client(json!({"type": "ping"})).unwrap(), ClientMsg::Ping);
    assert_eq!(
        client(json!({"type": "rematch"})).unwrap(),
        ClientMsg::Rematch
    );
}

#[test]
fn a_client_cannot_choose_its_die_value() {
    // `Roll` has nowhere to carry a die: an extra field is dropped, not used.
    assert_eq!(
        client(json!({"type": "roll", "die": 6})).unwrap(),
        ClientMsg::Roll
    );
}

#[test]
fn unknown_client_messages_are_rejected() {
    assert!(client(json!({"type": "cheat"})).is_err());
    assert!(client(json!({"token": "abc"})).is_err());
}

#[test]
fn errors_are_snake_case_codes_with_rule_errors_nested() {
    assert_eq!(
        to_json(&ServerMsg::Error {
            code: ErrorCode::NotHost
        }),
        json!({"type": "error", "code": "not_host"})
    );
    assert_eq!(
        to_json(&ServerMsg::Error {
            code: ErrorCode::Rule(RuleError::NotYourTurn)
        }),
        json!({"type": "error", "code": {"rule": "NotYourTurn"}})
    );
}

#[test]
fn welcome_tells_the_client_its_seat() {
    assert_eq!(
        to_json(&ServerMsg::Welcome { seat: SeatId(1) }),
        json!({"type": "welcome", "seat": 1})
    );
}

fn sample_room(stage: Stage) -> RoomView {
    RoomView {
        code: RoomCode("K7MXQ2".into()),
        host: SeatId(0),
        seats: vec![SeatView {
            id: SeatId(0),
            nickname: "Mari".into(),
            color: Color::Red,
            connected: true,
            away: false,
        }],
        stage,
        turn_ms_left: Some(30_000),
    }
}

#[test]
fn a_lobby_snapshot_has_a_flat_shape() {
    assert_eq!(
        to_json(&ServerMsg::Snapshot {
            room: sample_room(Stage::Lobby)
        }),
        json!({
            "type": "snapshot",
            "room": {
                "code": "K7MXQ2",
                "host": 0,
                "seats": [{"id": 0, "nickname": "Mari", "color": "Red", "connected": true, "away": false}],
                "stage": {"stage": "lobby"},
                "turn_ms_left": 30000
            }
        })
    );
}

#[test]
fn a_game_snapshot_round_trips() {
    let game = Game::new(&[Color::Red, Color::Blue]).unwrap();
    let msg = ServerMsg::Snapshot {
        room: sample_room(Stage::Game { game }),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(serde_json::from_str::<ServerMsg>(&json).unwrap(), msg);
}

#[test]
fn http_bodies() {
    assert_eq!(
        serde_json::from_value::<CreateRoom>(json!({"nickname": "Mari"})).unwrap(),
        CreateRoom {
            nickname: "Mari".into()
        }
    );
    assert_eq!(
        serde_json::from_value::<JoinRoom>(json!({"nickname": "Jüri"})).unwrap(),
        JoinRoom {
            nickname: "Jüri".into()
        }
    );
    assert_eq!(
        to_json(&Joined {
            code: RoomCode("K7MXQ2".into()),
            token: SeatToken("t".into()),
            seat: SeatId(1)
        }),
        json!({"code": "K7MXQ2", "token": "t", "seat": 1})
    );
    assert_eq!(
        to_json(&ApiError {
            code: ErrorCode::RoomFull
        }),
        json!({"code": "room_full"})
    );
}
