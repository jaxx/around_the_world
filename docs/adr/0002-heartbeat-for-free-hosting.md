# Clients send a heartbeat so a free host does not sleep mid-game

The game runs on Render's free web service, which spins down after 15 minutes without an incoming HTTP request or an incoming WebSocket message, losing every room held in memory. Messages the server sends do not count, and players can sit in the lobby or wait for others without sending anything. So the client sends `{"type": "ping"}` every 5 minutes while its socket is open, and the server drops it at the socket without involving the room. We chose this over hosts that stay awake on an open connection (Koyeb, an Oracle VM) because those ask for a credit card, and over Cloudflare Durable Objects because they would mean rewriting the server.

## Consequences

- `ClientMsg::Ping` does nothing on purpose; removing it brings the sleeping back.
- A deploy, a restart by Render, or a quiet spell of 15 minutes with nobody connected still ends any games in progress (ADR 0001 keeps rooms in memory); the first visit after a spin-down waits for the service to start.
