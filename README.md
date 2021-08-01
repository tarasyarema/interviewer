# interviewer


> In-memory, non stateful and session based code sharing application.

Test it here: [interviewer.taras.lol](https://interviewer.taras.lol/)

Note: it's deployed to render automatically in every master commit.

## Useful information

1. Sessions should be passed as query param: `?session_id={some_id}`.
2. Once no active connection is left in a session its state is lost forever.
3. It aims to be fast and simple, but it may happen that the global state differs between clients in some scenarios.
    For those cases you should be able to refresh the page and be fine. 

## Project structure

1. Rust API that handles the websocket connections and sessions in a simple mutex locked `HashMap<String, Vec<Client>>`. See the `src` folder.
2. Vanilla JS client that uses [ace](https://ace.c9.io/). See the `static` folder.

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

- [ ] Testing, stress-testing and benchmarking.
- [ ] Refactor Rust code to methods fo `App` as it's hard to read right now.
- [ ] Add current user list of the session to FE.
- [ ] Adding some sort of state (?).
