use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::hash_map::Entry,
    collections::HashMap,
    env,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::spawn,
};

use serde::{Deserialize, Serialize};
use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    protocol::Role,
    Error, Message, Result, WebSocket,
};

#[derive(Debug)]
struct Client {
    username: String,
    stream: TcpStream,
}

type Db = Arc<Mutex<HashMap<String, Vec<Client>>>>;

struct App {
    db: Db,
}

#[derive(Serialize, Deserialize, Debug)]
struct Range {
    row: u64,
    column: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct EditorEvent {
    id: Option<u64>,
    action: String,
    start: Range,
    end: Range,
    lines: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct UserEvent {
    username: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ValueEvent {
    target: String,
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Event {
    session: String,
    username: String,
    event: String,
    data: String,
    ts: u128,
}

const SESSION_ID: &str = "id";

impl App {
    fn handle_client(&self, stream: &TcpStream) -> Result<()> {
        let addr = stream.peer_addr().unwrap();

        let mut socket = accept_hdr(stream, |req: &Request, mut response: Response| {
            println!(
                "info: {} received new WS handskahe with path {}",
                addr,
                req.uri().path()
            );

            let headers = response.headers_mut();
            headers.append("X_INTERVIEWER_OK", ":3".parse().unwrap());

            Ok(response)
        })
        .unwrap();

        let is_first = {
            let mut db = self.db.lock().unwrap();

            match db.entry(SESSION_ID.to_string()) {
                Entry::Vacant(ele) => {
                    ele.insert(vec![Client {
                        username: addr.to_string(),
                        stream: stream.try_clone().unwrap(),
                    }]);
                }
                Entry::Occupied(mut ele) => {
                    ele.get_mut().push(Client {
                        username: addr.to_string(),
                        stream: stream.try_clone().unwrap(),
                    });
                }
            }

            let session_len = db.get(SESSION_ID).unwrap().len();

            println!("{}: inserted into db with len {}", addr, session_len,);

            if session_len == 1 {
                true
            } else {
                false
            }
        };

        loop {
            match socket.read_message()? {
                msg @ Message::Text(_) | msg @ Message::Binary(_) => {
                    let data: Event = serde_json::from_str(&msg.to_string()).unwrap();

                    println!(
                        "{}: [{}] sent {} bytes {:?}",
                        addr,
                        data.event,
                        data.event.len(),
                        data.data
                    );

                    match data.event.as_ref() {
                        "get_value" => match is_first {
                            true => {
                                let response = serde_json::to_string(&Event {
                                    session: data.session,
                                    username: data.username,
                                    event: "set_value".to_string(),
                                    // If the current client is the only one in the
                                    // DB we just send an empty string
                                    data: serde_json::to_string(&ValueEvent {
                                        target: addr.to_string(),
                                        text: "".to_string(),
                                    })
                                    .unwrap(),
                                    ts: SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis(),
                                })
                                .unwrap();

                                println!("{}: sent `set_value`", addr);
                                socket.write_message(Message::Text(response))?
                            }
                            false => {
                                for other_stream in self.db.lock().unwrap().get(SESSION_ID).unwrap()
                                {
                                    let mut other = WebSocket::from_raw_socket(
                                        other_stream.stream.try_clone().unwrap(),
                                        Role::Server,
                                        None,
                                    );

                                    if !other.can_write() {
                                        continue;
                                    }

                                    match other_stream.stream.peer_addr() {
                                        Ok(other_addr) => {
                                            // Skip the same client address
                                            if other_addr == addr {
                                                continue;
                                            }

                                            // This is a petition event to an other client
                                            // so that it sends back the
                                            let response = serde_json::to_string(&Event {
                                                session: data.session,
                                                username: data.username,
                                                event: "send_value".to_string(),
                                                data: "".to_string(),
                                                ts: SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_millis(),
                                            })
                                            .unwrap();

                                            println!(
                                                "{}: sending value petition of {} bytes to {}",
                                                addr,
                                                response.len(),
                                                other_addr
                                            );

                                            other.write_message(Message::Text(response))?;
                                        }
                                        Err(e) => {
                                            println!("{:?}", e);
                                            continue;
                                        }
                                    }

                                    // If everything went ok we exit
                                    break;
                                }
                            }
                        },
                        "set_value" => {
                            let e: ValueEvent = serde_json::from_str(&data.data).unwrap();
                            println!(
                                "{}: sending {} bytes of value data to {}",
                                addr,
                                e.text.len(),
                                e.target
                            );

                            for other_stream in self.db.lock().unwrap().get_mut(SESSION_ID).unwrap()
                            {
                                if other_stream.username == e.target {
                                    let mut other = WebSocket::from_raw_socket(
                                        other_stream.stream.try_clone().unwrap(),
                                        Role::Server,
                                        None,
                                    );

                                    if !other.can_write() {
                                        continue;
                                    }

                                    other.write_message(Message::Text(
                                        serde_json::to_string(&Event {
                                            session: data.session,
                                            username: data.username,
                                            event: "set_value".to_string(),
                                            data: serde_json::to_string(&ValueEvent {
                                                target: addr.to_string(),
                                                text: e.text,
                                            })
                                            .unwrap(),
                                            ts: SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis(),
                                        })
                                        .unwrap(),
                                    ))?;

                                    break;
                                }
                            }
                        }
                        "login" => {
                            let e: UserEvent = serde_json::from_str(&data.data).unwrap();
                            println!("{}: setting username to be {:?}", addr, e);

                            for other_stream in self.db.lock().unwrap().get_mut(SESSION_ID).unwrap()
                            {
                                if other_stream.stream.peer_addr().unwrap() == addr {
                                    other_stream.username = e.username;
                                    break;
                                }
                            }
                        }
                        "change" => {
                            for other_stream in self.db.lock().unwrap().get(SESSION_ID).unwrap() {
                                let mut other = WebSocket::from_raw_socket(
                                    other_stream.stream.try_clone().unwrap(),
                                    Role::Server,
                                    None,
                                );

                                if !other.can_write() {
                                    continue;
                                }

                                match other_stream.stream.peer_addr() {
                                    Ok(other_addr) => {
                                        if other_addr != addr {
                                            // Now broadcast the received event to all other
                                            // editors of the current document
                                            let response = serde_json::to_string(&data).unwrap();

                                            println!(
                                                "{}: sending {} bytes to {}",
                                                addr,
                                                response.len(),
                                                other_addr
                                            );
                                            other.write_message(Message::Text(response))?;
                                        }
                                    }
                                    Err(e) => {
                                        println!("{:?}", e)
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }

                // Handle a client closure message
                Message::Close(_) => {
                    let mut db = self.db.lock().unwrap();

                    db.get_mut(SESSION_ID)
                        .unwrap()
                        .retain(|other| other.stream.peer_addr().unwrap() != addr);

                    println!("{}: Removed from DB", addr)
                }
                // Skip all non text nor binary messages
                Message::Ping(_) | Message::Pong(_) => {}
            }
        }
    }

    fn clone(app: &App) -> App {
        App { db: app.db.clone() }
    }

    fn new() -> App {
        App {
            db: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn main() {
    let port = env::var("PORT").unwrap_or("1337".to_string());
    let server_addr = format!("0.0.0.0:{}", port);

    let server = TcpListener::bind(&server_addr).unwrap();
    println!(
        "info: server listening on port {}",
        server.local_addr().unwrap().port()
    );
    let app = App::new();

    for stream in server.incoming() {
        let moved_app = App::clone(&app);

        spawn(move || match stream {
            Ok(stream) => {
                println!(
                    "info: incoming connection by {}",
                    stream.peer_addr().unwrap()
                );

                if let Err(err) = moved_app.handle_client(&stream) {
                    match err {
                        Error::Protocol(e) => println!("error: protocol {:?}", e),
                        Error::ConnectionClosed | Error::Utf8 => {}
                        e => println!("error: could not move client {:?}", e),
                    }
                }
            }
            Err(e) => println!("error: could not accept stream {:?}", e),
        });
    }
}
