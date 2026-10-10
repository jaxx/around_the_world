//! Which page a URL path shows: `/` is home, `/r/{code}` a room's invite link.

use protocol::RoomCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Room(RoomCode),
}

/// The page for `path`. Room codes are made upper-case; anything that is
/// not a plausible room link goes home.
pub fn parse(path: &str) -> Route {
    let path = path.strip_suffix('/').unwrap_or(path);
    match path.strip_prefix("/r/") {
        Some(code) if is_code(code) => Route::Room(RoomCode(code.to_ascii_uppercase())),
        _ => Route::Home,
    }
}

/// Letters and digits only, at most 16 of them.
fn is_code(code: &str) -> bool {
    (1..=16).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The invite link path for a room.
pub fn room_path(code: &RoomCode) -> String {
    format!("/r/{}", code.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(code: &str) -> Route {
        Route::Room(RoomCode(code.into()))
    }

    #[test]
    fn the_root_is_home() {
        assert_eq!(parse("/"), Route::Home);
        assert_eq!(parse(""), Route::Home);
    }

    #[test]
    fn invite_links_name_a_room_in_upper_case() {
        assert_eq!(parse("/r/K7MXQ2"), room("K7MXQ2"));
        assert_eq!(parse("/r/k7mxq2"), room("K7MXQ2"));
        assert_eq!(parse("/r/K7MXQ2/"), room("K7MXQ2"));
    }

    #[test]
    fn anything_else_goes_home() {
        assert_eq!(parse("/r/"), Route::Home);
        assert_eq!(parse("/r/K7-MX"), Route::Home);
        assert_eq!(parse("/r/ABCDEFGHIJKLMNOPQ"), Route::Home, "too long");
        assert_eq!(parse("/elsewhere"), Route::Home);
        assert_eq!(parse("/r/K7MXQ2/extra"), Route::Home);
    }

    #[test]
    fn room_paths_round_trip() {
        let code = RoomCode("K7MXQ2".into());
        assert_eq!(room_path(&code), "/r/K7MXQ2");
        assert_eq!(parse(&room_path(&code)), Route::Room(code));
    }
}
