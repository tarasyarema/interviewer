use std::{
    collections::hash_map::Entry,
    collections::HashMap,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::spawn,
};

use log::*;
use serde::{Deserialize, Serialize};
use tungstenite::{
    accept, handshake::HandshakeRole, protocol::Role, Error, HandshakeError, Message, Result,
    WebSocket,
};

type Db = Arc<Mutex<HashMap<String, Vec<TcpStream>>>>;

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

fn must_not_block<Role: HandshakeRole>(err: HandshakeError<Role>) -> Error {
    match err {
        HandshakeError::Interrupted(_) => panic!("Bug: blocking socket would block"),
        HandshakeError::Failure(f) => f,
    }
}

const ID: &str = "id";

impl App {
    fn handle_client(&self, stream: &TcpStream) -> Result<()> {
        let mut socket = accept(stream).map_err(must_not_block)?;
        let addr = stream.peer_addr().unwrap();

        {
            let mut db = self.db.lock().unwrap();

            match db.entry(ID.to_string()) {
                Entry::Vacant(ele) => {
                    ele.insert(vec![stream.try_clone().unwrap()]);
                }
                Entry::Occupied(mut ele) => {
                    ele.get_mut().push(stream.try_clone().unwrap());
                }
            }

            println!(
                "Client {} inserted into db with len {}",
                addr,
                db.get(ID).unwrap().len()
            )
        }

        loop {
            match socket.read_message()? {
                msg @ Message::Text(_) | msg @ Message::Binary(_) => {
                    let data: EditorEvent = serde_json::from_str(&msg.to_string()).unwrap();
                    println!("{}: ({} bytes) {:?}", addr, msg.len(), data);

                    for other_stream in self.db.lock().unwrap().get(ID).unwrap() {
                        let mut other = WebSocket::from_raw_socket(
                            other_stream.try_clone().unwrap(),
                            Role::Server,
                            None,
                        );

                        if other.can_write() {
                            match other_stream.peer_addr() {
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
                }

                // Skip all non text nor binary messages
                Message::Ping(_) | Message::Pong(_) | Message::Close(_) => {}
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
    let server_addr = "127.0.0.1:1337";

    let server = TcpListener::bind(server_addr).unwrap();
    println!("Server listening on ws://{}", server_addr);

    let app = App::new();

    for stream in server.incoming() {
        let moved_app = App::clone(&app);

        spawn(move || match stream {
            Ok(stream) => {
                if let Err(err) = moved_app.handle_client(&stream) {
                    match err {
                        Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                        e => error!("test: {}", e),
                    }
                }
            }
            Err(e) => error!("Error accepting stream: {}", e),
        });
    }
}
