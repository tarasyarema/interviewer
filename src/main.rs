use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::hash_map::Entry,
    collections::HashMap,
    env,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, StreamExt};

use futures_util::TryStreamExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_hdr_async_with_config, tungstenite::Error};
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::{Message, WebSocketConfig};

const MAX_STRING_LEN: usize = 24;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct User {
    username: String,
    socket: SocketAddr,
}

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type Db = Arc<Mutex<HashMap<String, Vec<User>>>>;

#[derive(Serialize, Deserialize, Debug)]
struct Event {
    username: String,
    event: String,
    data: String,
    ts: u128,
}

#[derive(Serialize, Deserialize, Debug)]
struct LoginCommand {
    username: String,
    session_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Range {
    row: u64,
    column: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChangeCommand {
    id: Option<u64>,
    action: String,
    start: Range,
    end: Range,
    lines: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ValueCommand {
    target: String,
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ErrorCommand {
    msg: String,
}

#[derive(Serialize, Deserialize, Debug)]
enum Command {
    Login { data: LoginCommand },
    Change { data: ChangeCommand },
    SetValue { data: String },
}

struct App {
    db: Db,
    peers: PeerMap,
    session: Mutex<String>,
    username: Mutex<String>,
}

impl App {
    fn new() -> Self {
        App {
            db: Arc::new(Mutex::new(HashMap::new())),
            peers: PeerMap::new(Mutex::new(HashMap::new())),
            session: Mutex::new("".to_string()),
            username: Mutex::new("".to_string()),
        }
    }

    fn add_user(&self, addr: SocketAddr, username: String, session: String) -> Vec<User> {
        match self.db.lock() {
            Ok(mut data) => match data.entry(session.to_string()) {
                Entry::Vacant(ele) => {
                    ele.insert(vec![User {
                        username: username,
                        socket: addr,
                    }]);
                    vec![]
                }
                Entry::Occupied(mut ele) => {
                    let list = ele.get_mut();
                    let ret = list.clone();
                    list.push(User {
                        username: username,
                        socket: addr,
                    });
                    ret
                }
            },
            Err(mut err) => {
                let data = err.get_mut();
                match data.entry(session) {
                    Entry::Vacant(ele) => {
                        ele.insert(vec![User {
                            username: username,
                            socket: addr,
                        }]);
                        vec![]
                    }
                    Entry::Occupied(mut ele) => {
                        let list = ele.get_mut();
                        let ret = list.clone();
                        list.push(User {
                            username: username,
                            socket: addr,
                        });
                        ret
                    }
                }
            }
        }
    }

    fn get_session(&self) -> String {
        self.session.lock().unwrap().to_string()
    }

    fn get_username(&self) -> String {
        self.username.lock().unwrap().to_string()
    }

    fn get_others(&self, addr: SocketAddr) -> Vec<User> {
        let session = self.get_session();

        match self.db.lock() {
            Ok(data) => {
                let mut others = data.get(&session).unwrap().clone();
                others.retain(|other| other.socket != addr);
                others.clone()
            }
            Err(err) => {
                let data = err.get_ref();
                let mut others = data.get(&session).unwrap().clone();
                others.retain(|other| other.socket != addr);
                others.clone()
            }
        }
    }

    fn login(&self, addr: SocketAddr, cmd: LoginCommand) {
        let session = cmd.session_id.to_string();
        let username = cmd.username.to_string();

        // Validation error on username and session strings
        if session.len() == 0
            || session.len() > MAX_STRING_LEN
            || username.len() > MAX_STRING_LEN
            || username.len() == 0
        {
            println!("{}: Got validation error", addr);

            let cmd_error: String = serde_json::to_string(&Event {
                event: "error".to_string(),
                username: cmd.username.to_string(),
                data: serde_json::to_string(&ErrorCommand {
                    msg: "Values received exceed max length or empty".to_string(),
                })
                .unwrap(),
                ts: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            })
            .unwrap();

            match self.peers.lock() {
                Ok(p) => p,
                Err(e) => e.into_inner(),
            }
            .get(&addr)
            .unwrap()
            .unbounded_send(Message::Text(cmd_error.to_string()))
            .unwrap_or_default();

            return;
        }

        // Set the session of the current app instance
        {
            let mut self_session = self.session.lock().unwrap();
            *self_session = session.to_string();
        }
        // Set the usernale of the current app instance
        {
            let mut self_username = self.username.lock().unwrap();
            *self_username = username.to_string();
        }

        println!("{}: logged in {:?}", addr, cmd);

        // Add the current user to the session list and
        // return the list of all other participants of it
        let others = self.add_user(addr, username.to_string(), session.to_string());

        {
            let peers = match self.peers.lock() {
                Ok(p) => p,
                Err(e) => e.into_inner(),
            };

            // Create the `add_user` event so that the other
            // users in the session add this one to their list
            let cmd_string: String = serde_json::to_string(&Event {
                event: "add_user".to_string(),
                username: cmd.username.to_string(),
                data: serde_json::to_string(&LoginCommand {
                    username: cmd.username.to_string(),
                    session_id: session.to_string(),
                })
                .unwrap(),
                ts: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            })
            .unwrap();

            println!("{}: others -> {:?}", addr, others);

            for other in others {
                peers
                    .get(&other.socket)
                    .unwrap()
                    .unbounded_send(Message::Text(cmd_string.to_string()))
                    .unwrap_or_default();

                // Now send a message to self with the other user
                let self_cmd_string: String = serde_json::to_string(&Event {
                    event: "add_user".to_string(),
                    username: cmd.username.to_string(),
                    data: serde_json::to_string(&LoginCommand {
                        username: other.username.to_string(),
                        session_id: session.to_string(),
                    })
                    .unwrap(),
                    ts: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                })
                .unwrap();

                peers
                    .get(&addr)
                    .unwrap()
                    .unbounded_send(Message::Text(self_cmd_string.to_string()))
                    .unwrap_or_default();
            }
        }

        println!("{}: added to {}", addr, session);

        // Now trigger the petition for value
        self.petition_for_value(addr)
    }

    fn petition_for_value(&self, addr: SocketAddr) {
        let username = self.get_username();

        let peers = match self.peers.lock() {
            Ok(p) => p,
            Err(e) => e.into_inner(),
        };

        for other in self.get_others(addr) {
            let cmd_string: String = serde_json::to_string(&Event {
                event: "send_value".to_string(),
                username: username.to_string(),
                data: "".to_string(),
                ts: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            })
            .unwrap();

            peers
                .get(&other.socket)
                .unwrap()
                .unbounded_send(Message::Text(cmd_string.to_string()))
                .unwrap_or_default();

            println!("{}: sent value petition to {:?}", addr, other);
            break;
        }
    }

    fn forward_value(&self, addr: SocketAddr, cmd: ValueCommand) {
        let peers = match self.peers.lock() {
            Ok(p) => p,
            Err(e) => e.into_inner(),
        };

        // Create the `set_value` event
        let cmd_string: String = serde_json::to_string(&Event {
            event: "set_value".to_string(),
            username: self.get_username().to_string(),
            data: serde_json::to_string(&cmd).unwrap(),
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        })
        .unwrap();

        for other in self
            .get_others(addr)
            .into_iter()
            .filter(|a| a.username == cmd.target)
        {
            peers
                .get(&other.socket)
                .unwrap()
                .unbounded_send(Message::Text(cmd_string.to_string()))
                .unwrap_or_default();
        }
    }

    fn remove(&self, addr: SocketAddr, session: String) {
        // Remove the current user from its session
        let others = match self.db.lock() {
            Ok(mut data) => {
                let others = data.get_mut(&session).unwrap();
                others.retain(|other| other.socket != addr);
                others.clone()
            }
            Err(mut err) => {
                let data = err.get_mut();
                let others = data.get_mut(&session).unwrap();
                others.retain(|other| other.socket != addr);
                others.clone()
            }
        };

        // Get the current username
        let username = { self.username.lock().unwrap().to_string() };

        // Create the `remove_user` event
        let cmd_string: String = serde_json::to_string(&Event {
            event: "remove_user".to_string(),
            username: username.to_string(),
            data: serde_json::to_string(&LoginCommand {
                username: username.to_string(),
                session_id: session.to_string(),
            })
            .unwrap(),
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        })
        .unwrap();

        {
            let peers = match self.peers.lock() {
                Ok(p) => p,
                Err(e) => e.into_inner(),
            };

            for other in others {
                peers
                    .get(&other.socket)
                    .unwrap()
                    .unbounded_send(Message::Text(cmd_string.to_string()))
                    .unwrap_or_default();
            }
        }
    }

    fn change(&self, addr: SocketAddr, cmd: ChangeCommand) {
        let cmd_string: String = serde_json::to_string(&Event {
            event: "change".to_string(),
            username: self.username.lock().unwrap().to_string(),
            data: serde_json::to_string(&cmd).unwrap(),
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        })
        .unwrap();

        let peers = match self.peers.lock() {
            Ok(p) => p,
            Err(e) => e.into_inner(),
        };

        let others = self.get_others(addr);
        let others_len = others.len();

        for other in others {
            peers
                .get(&other.socket)
                .unwrap()
                .unbounded_send(Message::Text(cmd_string.to_string()))
                .unwrap_or_default();

            println!(
                "{}: sent change ({} bytes) to {:?}",
                addr,
                cmd_string.len(),
                other
            );
        }

        println!("{}: sent change to {} users", addr, others_len)
    }
}

async fn accept_connection(db: Db, peers: PeerMap, stream: TcpStream, addr: SocketAddr) {
    if let Err(e) = handle_connection(db, peers, stream, addr).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => println!("error: processing connection => {}", err),
        }
    }
}

async fn handle_connection(
    db: Db,
    peers: PeerMap,
    raw_stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Error> {
    let app = App {
        db: db,
        peers: peers,
        session: Mutex::new("".to_string()),
        username: Mutex::new("".to_string()),
    };

    let callback = |_req: &Request, mut response: Response| {
        let headers = response.headers_mut();

        // Add the `interviewer` protocol to the headers
        headers.append("Sec-WebSocket-Protocol", "interviewer".parse().unwrap());

        Ok(response)
    };

    let ws_stream = accept_hdr_async_with_config(
        raw_stream,
        callback,
        Some(WebSocketConfig {
            max_send_queue: None,
            max_message_size: None,
            max_frame_size: None,
            // This setting allows to accept client frames which are not masked
            // This is not in compliance with RFC 6455 but might be handy in some
            // rare cases where it is necessary to integrate with existing/legacy
            // clients which are sending unmasked frames
            accept_unmasked_frames: true,
        }),
    )
    .await
    .expect(
        format!(
            "error: error during the websocket handshake occurred for addr {:?}",
            addr
        )
        .as_ref(),
    );

    println!("{}: WS connection established", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();

    match app.peers.lock() {
        Ok(mut data) => data.insert(addr, tx),
        Err(mut err) => {
            let data = err.get_mut();
            data.insert(addr, tx)
        }
    };

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        let data: Event = match serde_json::from_str(&msg.to_string()) {
            Ok(data) => data,
            Err(_) => Event {
                // Empty event
                event: "".to_string(),
                username: "".to_string(),
                data: "".to_string(),
                ts: 1,
            },
        };

        println!(
            "{}: Received {} event with {} bytes of data",
            addr,
            data.event,
            data.data.len(),
        );

        match data.event.as_ref() {
            "login" => {
                let cmd: LoginCommand = serde_json::from_str(data.data.as_ref()).unwrap();
                app.login(addr, cmd);
            }
            "change" => {
                let cmd: ChangeCommand = serde_json::from_str(data.data.as_ref()).unwrap();
                app.change(addr, cmd)
            }
            "set_value" => {
                let cmd: ValueCommand = serde_json::from_str(data.data.as_ref()).unwrap();
                app.forward_value(addr, cmd)
            }
            _ => (),
        };

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    // Remove the address from the peers
    match app.peers.lock() {
        Ok(mut data) => data.remove(&addr),
        Err(mut err) => {
            let data = err.get_mut();
            data.remove(&addr)
        }
    };

    let session = app.get_session();

    // If the session is empty it means that the user
    // had an error on login
    if session == "" {
        return Ok(());
    }

    app.remove(addr, session.to_string());

    println!(
        "{}: disconnected from session {} succesfully",
        &addr, session
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let port = env::var("PORT").unwrap_or("1337".to_string());
    let addr = format!("0.0.0.0:{}", port).to_string();

    // Create the mother app
    let app = App::new();

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;

    let listener = try_socket.expect("Failed to bind");
    println!("Listening on: {}", addr);

    // Aawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(accept_connection(
            app.db.clone(),
            app.peers.clone(),
            stream,
            addr,
        ));
    }

    Ok(())
}
