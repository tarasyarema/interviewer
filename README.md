# interviewer

> In-memory, non stateful and session based code sharing application.

Test it here: [interviewer.taras.lol](https://interviewer.taras.lol/)

Note: it's deployed to render automatically in every master commit.

## Useful information

1. Sessions should be passed as query param: `?session_id={some_id}`.
2. Usernames can be passed as query params (to skip the prompt): `?username={your_username}`
3. Once no active connection is left in a session its state is lost forever.
4. It aims to be fast and simple, but it may happen that the global state differs between clients in some scenarios.
    For those cases you should be able to refresh the page and be fine. 

## Project structure

1. Rust API that handles the websocket connections and sessions via tokio channel communications. This means that
    for every new ws connection a new task is spawned and it channels are stored in a global state that stores
    each socket address (IP) and its channels. Hence the actual communication between users in the server side is
    done using those channels and not an actual WS communication.
    For more details, see the `src` folder.
3. Vanilla JS client that uses [ace](https://ace.c9.io/). See the `static` folder.

## Run locally

### Server

```bash
cargo run
```

#### With Docker

```bash
$ docker build -t app .
$ docker run -p 1337:1337 --name app-ws app
```

### Client

Just open the `static/index.html` in a browser or use `sfz` (for example):

```bash
sfz -r -L -C static
```

## TODO

- [X] (**feature**) Add current user list of the session to FE
- [X] (**refactor**) Refactor Rust code to methods fo `App` as it's hard to read right now
- [X] (**bug**) Sometimes it keeps sending the `set_value` event
- [X] (**security**) Limit the user input that is tored without checks: usernames and session ids (those are store in raw rust `String`s)
- [X] (**feature**) Handle the ws url in a better way, xd
- [ ] (**feature**) Make the design better (I suck at it xd :/)
- [ ] (**testing**) Testing, stress-testing and benchmarking
- [ ] (**feature**) Adding some sort of state (this may be an optional feature via a REDIS connection)
- [ ] (**feature**) Add theme and language change

## License

**GNU General Public License v3.0**
