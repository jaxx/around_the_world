//! Every piece of Estonian text the client shows, in one place.

use protocol::ErrorCode;
use rules::RuleError;

pub const TITLE: &str = "Reis ümber maailma";
/// Accessible name of the board drawing.
pub const BOARD: &str = "Mängulaud";

pub const NICKNAME: &str = "Sinu nimi";
pub const CREATE_ROOM: &str = "Loo uus tuba";
pub const CREATE: &str = "Loo tuba";
pub const JOIN_WITH_CODE: &str = "Liitu koodiga";
pub const JOIN_ROOM: &str = "Liitu tuppa";
pub const ROOM_CODE: &str = "Toa kood";
pub const JOIN: &str = "Liitu";
pub const ROOM: &str = "Tuba";
pub const CONNECTING: &str = "Ühendan…";
pub const NETWORK_ERROR: &str = "Serveriga ei saanud ühendust. Proovi uuesti.";
pub const RECONNECTING: &str = "Ühendus katkes. Proovin uuesti…";

pub const LOBBY: &str = "Ootesaal";
pub const PLAYERS: &str = "Mängijad";
pub const YOU: &str = "sina";
pub const HOST: &str = "võõrustaja";
pub const DISCONNECTED: &str = "ühendus katkenud";
pub const YOUR_COLOR: &str = "Sinu värv";
pub const INVITE: &str = "Kutselink";
pub const COPY: &str = "Kopeeri";
pub const COPIED: &str = "Kopeeritud!";
pub const START: &str = "Alusta mängu";
pub const WAITING_FOR_HOST: &str = "Ootame, kuni võõrustaja mängu alustab.";
pub const NEED_PLAYERS: &str = "Mängu alustamiseks on vaja vähemalt kahte mängijat.";
pub const LEAVE: &str = "Lahku";
pub const ROLL: &str = "Viska";
pub const DIE: &str = "Täring";
pub const STARTER_ROLL: &str = "Kes alustab? Viska täringut!";
pub const YOUR_TURN_ROLL: &str = "Sinu kord: viska täringut!";
pub const YOUR_TURN_MOVE: &str = "Vali nupp, millega käia.";
pub const TURN: &str = "kord";
pub const AWAY: &str = "eemal";
pub const SECONDS: &str = "s";

pub const GAME_OVER: &str = "Mäng läbi!";
pub const RESULTS: &str = "Tulemused";
pub const REMATCH: &str = "Uus mäng";
pub const WAITING_FOR_REMATCH: &str = "Ootame, kas võõrustaja alustab uue mängu.";
pub const LOG: &str = "Sündmused";

/// A colour in the genitive, as in "punase nupu".
pub fn color_genitive(color: rules::Color) -> &'static str {
    match color {
        rules::Color::Red => "punase",
        rules::Color::Yellow => "kollase",
        rules::Color::Green => "rohelise",
        rules::Color::Blue => "sinise",
    }
}

/// A colour's name, as in "punane nupp".
pub fn color_name(color: rules::Color) -> &'static str {
    match color {
        rules::Color::Red => "punane",
        rules::Color::Yellow => "kollane",
        rules::Color::Green => "roheline",
        rules::Color::Blue => "sinine",
    }
}

/// What to tell the player when the server refuses something.
pub fn error_text(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::BadMessage => "Server ei saanud sõnumist aru.",
        ErrorCode::ExpectedHello => "Ühendus ei alanud õigesti. Laadi leht uuesti.",
        ErrorCode::UnknownToken => "Sinu kohta selles toas enam ei ole.",
        ErrorCode::RoomNotFound => "Sellist tuba ei ole.",
        ErrorCode::RoomFull => "Tuba on täis.",
        ErrorCode::TooManyRooms => "Server on praegu täis. Proovi hiljem uuesti.",
        ErrorCode::RateLimited => "Liiga palju uusi tube. Oota natuke.",
        ErrorCode::BadNickname => "Nimi peab olema 1–20 märki.",
        ErrorCode::NotHost => "Seda saab teha ainult võõrustaja.",
        ErrorCode::NotInLobby => "Mäng on juba alanud.",
        ErrorCode::NotInGame => "Mäng pole veel alanud.",
        ErrorCode::ColorTaken => "See värv on juba võetud.",
        ErrorCode::NotEnoughPlayers => "Mängimiseks on vaja vähemalt kahte mängijat.",
        ErrorCode::GameNotOver => "Mäng ei ole veel läbi.",
        ErrorCode::Rule(RuleError::NotYourTurn) => "Praegu ei ole sinu kord.",
        ErrorCode::Rule(RuleError::IllegalMove) => "Selle nupuga ei saa praegu käia.",
        ErrorCode::Rule(RuleError::WrongPhase) => "Seda ei saa praegu teha.",
        ErrorCode::Rule(RuleError::InvalidDie | RuleError::InvalidPlayers) => {
            "Midagi läks valesti."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_has_its_own_estonian_text() {
        let codes = [
            ErrorCode::BadMessage,
            ErrorCode::ExpectedHello,
            ErrorCode::UnknownToken,
            ErrorCode::RoomNotFound,
            ErrorCode::RoomFull,
            ErrorCode::TooManyRooms,
            ErrorCode::RateLimited,
            ErrorCode::BadNickname,
            ErrorCode::NotHost,
            ErrorCode::NotInLobby,
            ErrorCode::NotInGame,
            ErrorCode::ColorTaken,
            ErrorCode::NotEnoughPlayers,
            ErrorCode::GameNotOver,
            ErrorCode::Rule(RuleError::InvalidPlayers),
            ErrorCode::Rule(RuleError::InvalidDie),
            ErrorCode::Rule(RuleError::NotYourTurn),
            ErrorCode::Rule(RuleError::WrongPhase),
            ErrorCode::Rule(RuleError::IllegalMove),
        ];
        let texts: Vec<&str> = codes.iter().map(|&c| error_text(c)).collect();
        for (code, text) in codes.iter().zip(&texts) {
            assert!(!text.is_empty(), "{code:?}");
        }
        assert_eq!(error_text(ErrorCode::RoomNotFound), "Sellist tuba ei ole.");
        assert_eq!(
            error_text(ErrorCode::BadNickname),
            "Nimi peab olema 1–20 märki."
        );
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;
    use rules::Color;

    #[test]
    fn colours_have_estonian_names() {
        let names: Vec<&str> = Color::ALL.into_iter().map(color_name).collect();
        assert_eq!(names, vec!["punane", "kollane", "roheline", "sinine"]);
    }
}

/// Whose turn it is, e.g. "Mari kord".
pub fn turn_of(nickname: &str) -> String {
    format!("{nickname} kord")
}

/// A finishing place, e.g. "1. koht".
pub fn place(n: usize) -> String {
    format!("{n}. koht")
}

#[cfg(test)]
mod phrase_tests {
    use super::*;

    #[test]
    fn turns_and_places_read_naturally() {
        assert_eq!(turn_of("Mari"), "Mari kord");
        assert_eq!(place(1), "1. koht");
    }
}
