# Pure `rules` crate shared by server and client; server is the only authority

The game logic lives in a `rules` crate with no I/O, randomness or clock: it takes a game state plus an action (including the die value) and returns the new state or a rejection. The axum server owns the random number generator, timers and rooms, and is the sole authority on game state. The Leptos/WASM client uses the same crate only to highlight legal moves and preview positions, and never decides an outcome. We chose this over a TypeScript client with a separate rules implementation, so that there is one implementation of a rule set with many subtle cases (bounce back onto the track, stacks, three attempts, fill-from-end home lane), and so that the rules are cheap to test and bots can be added later.

## Consequences

- The client and server must be deployed from the same version of `rules`; a mismatch can only cause wrong highlights, never a wrong game, because the server re-validates every action.
- Game state is in memory only in v1, so the server must run as a single long-lived instance.
