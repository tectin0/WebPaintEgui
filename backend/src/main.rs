use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

use anyhow::{Context, Result};

use http::{
    header::{self, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN},
    HeaderValue,
};
use log::{debug, info, trace};
use shared::{
    config::{Config, CONFIG},
    ChangedLines, ClientID, Flag, Lines, Message, Peer,
};
use simple_logger::SimpleLogger;

struct Client {
    id: ClientID,
}

struct State {
    lines: Lines,
    clients: HashMap<String, Client>,
    clear_sync: Option<HashSet<ClientID>>,
    changed_lines_sync: HashMap<ClientID, ChangedLines>,
}

impl State {
    fn new() -> Self {
        Self {
            lines: Lines::default(),
            clients: HashMap::new(),
            clear_sync: None,
            changed_lines_sync: HashMap::new(),
        }
    }
}

const VALID_POST_PATHS: [&str; 3] = ["/send_lines", "/delete_lines", "/hello"];

fn main() {
    let config = CONFIG.read().unwrap();

    SimpleLogger::new()
        .init()
        .context("Failed to initialize logger")
        .unwrap();

    log::set_max_level(log::LevelFilter::Debug);

    let mut state = State::new();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.host.port)).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        match handle_connection(stream, &mut state, &config).context("Failed to handle connection")
        {
            Ok(_) => (),
            Err(e) => println!("Error: {:?}", e),
        };
    }
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    state: &mut State,
    config: &Config,
) -> Result<()> {
    let buf_reader = BufReader::new(&mut stream);

    let mut empty_line_counter = 0;

    let mut is_post = false;

    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| {
            if line.starts_with("POST") && VALID_POST_PATHS.iter().any(|path| line.contains(path)) {
                is_post = true;
            }

            if line.is_empty() {
                empty_line_counter += 1;
            }

            match is_post {
                true => empty_line_counter < 2,
                false => empty_line_counter < 1,
            }
        })
        .collect();

    if http_request.is_empty() {
        return Ok(());
    }

    let mut http_request_iter = http_request.iter();

    let request_line = http_request_iter.next().unwrap();
    let _ = http_request_iter.next().unwrap();

    let mut content: Option<&String> = None;

    if is_post {
        content = match http_request_iter.clone().last() {
            Some(content) => Some(content),
            None => {
                return Err(anyhow::anyhow!(
                    "Failed to get content from request: {:?}",
                    http_request
                ))?
            }
        };
    }

    let peer = Peer(stream.peer_addr().unwrap().to_string());

    let client_id: Option<ClientID> = match state.clients.get(peer.ip()?) {
        Some(client) => Some(client.id),
        None => None,
    };

    trace!(
        "Request by {} (ID: {}): {:?}",
        peer.ip()?,
        match client_id {
            Some(client_id) => client_id.to_string(),
            None => "Unknown".to_string(),
        },
        request_line
    );

    let mut status_line = None;
    let mut filename = None;
    let mut content_type = None;

    let mut replace_content: Vec<[String; 2]> = Vec::new();

    match request_line.as_str() {
        "GET / HTTP/1.1" => {
            let peer_ip = peer.ip()?;

            let host = match peer_ip {
                "127.0.0.1" => format!("127.0.0.1:{}", config.host.port),
                _ => format!("{}:{}", config.host.ip, config.host.port),
            };

            replace_content.push(["#host".to_string(), host]);
            replace_content.push(["#title".to_string(), config.website.title.clone()]);

            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/index.html");
            content_type = Some("text/html");
        }
        "GET /wasm/frontend.js HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/wasm/frontend.js");
            content_type = Some("text/javascript");
        }
        "GET /wasm/frontend_bg.wasm HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/wasm/frontend_bg.wasm");
            content_type = Some("application/wasm");
        }
        "GET /hello HTTP/1.1" => {
            let client_id = match state.clients.get(peer.ip()?) {
                Some(client) => client.id,
                None => {
                    let client_id = ClientID::new();
                    state
                        .clients
                        .insert(peer.ip()?.to_string(), Client { id: client_id });

                    state
                        .changed_lines_sync
                        .insert(client_id, ChangedLines::default());

                    client_id
                }
            };

            info!("Client {} connected from {}", client_id.0, peer.ip()?);

            debug!("Current clients: {:?}", state.clients.keys());
            debug!("Current changed lines: {:?}", state.changed_lines_sync);

            let response = serde_json::to_string(&client_id).unwrap();

            // TODO: extract to function

            status_line = Some("HTTP/1.1 200 OK");
            content_type = Some("text/html");

            let status_line = status_line.unwrap();
            let content_type = content_type.unwrap();

            let length = response.len();

            let headermap = prepare_headermap(content_type, length);

            let response = format!(
                "{}\r\n{}\r\n{}\r\n\r\n",
                status_line,
                {
                    headermap
                        .iter()
                        .fold(String::new(), |mut acc, (key, value)| {
                            acc.push_str(&format!("{}: {}\r\n", key, value.to_str().unwrap()));
                            acc
                        })
                },
                response
            );

            let response = response.into_bytes();

            stream.write_all(response.as_slice()).unwrap();
        }
        "POST /send_lines HTTP/1.1" => {
            let content = unwrap_content(content, &http_request)?;

            let message = serde_json::from_str::<Message>(&content).unwrap();

            let other_lines = message.lines;
            let changed_lines = message.changed_lines;
            let canvas_size = match message.canvas_size {
                Some(canvas_size) => canvas_size,
                None => {
                    return Err(anyhow::anyhow!(
                        "Failed to get canvas size from request: {:?}",
                        http_request
                    ))?
                }
            };

            debug!("Received lines: {:?}", other_lines.keys());
            debug!("Current lines: {:?}", state.lines.keys());

            state.lines.merge(
                other_lines,
                &changed_lines,
                &canvas_size,
                shared::MergeMode::FromCanvas,
            );

            if changed_lines.is_some() {
                let changed_lines = changed_lines.unwrap().0;

                let changed_lines_sync = &mut state.changed_lines_sync;

                changed_lines_sync
                    .iter_mut()
                    .for_each(|(_, changed_lines_l)| {
                        // if client_id_l != &client_id {
                        //     changed_lines_l.0.extend(changed_lines.iter());
                        // } // TODO: check if can be ommited

                        changed_lines_l.0.extend(changed_lines.iter());
                    });
            }
        }
        "POST /delete_lines HTTP/1.1" => {
            let content = unwrap_content(content, &http_request)?;

            let changed_lines = serde_json::from_str::<ChangedLines>(&content).context(format!(
                "Failed to parse changed lines - content: {}",
                content
            ))?;

            for line_id in changed_lines.0.iter() {
                debug!("Deleting line: {}", line_id);
                state.lines.0.remove(line_id);
                debug!("Current lines: {:?}", state.lines.0.keys());
            }

            let changed_lines_sync = &mut state.changed_lines_sync;

            state.clients.iter().for_each(|client| {
                let client_id = client.1.id;

                changed_lines_sync
                    .entry(client_id)
                    .or_insert_with(ChangedLines::default)
                    .0
                    .extend(changed_lines.0.iter());
            });

            debug!("Changed lines: {:?}", changed_lines_sync);
        }
        "GET /get_lines HTTP/1.1" => {
            let client_id = unwrap_client_id(client_id, &mut stream, peer)?;

            let lines = state.lines.clone();

            let changed_lines = state.changed_lines_sync.remove(&client_id);

            let mut flag: Option<Flag> = None;

            if state.clear_sync.is_some() {
                let clear_sync = state.clear_sync.as_mut().unwrap();

                if clear_sync.remove(&client_id) {
                    flag = Some(Flag::Clear);
                }
            }

            let message = Message {
                lines: lines.clone(),
                changed_lines,
                flag,
                canvas_size: None,
            };

            let response = serde_json::to_string(&message).unwrap() + "\r\n\r\n";

            // TODO: extract to function

            status_line = Some("HTTP/1.1 200 OK");
            content_type = Some("text/html");

            let status_line = status_line.unwrap();
            let content_type = content_type.unwrap();

            let length = response.len();

            let headermap = prepare_headermap(content_type, length);

            let response = format!(
                "{}\r\n{}\r\n{}\r\n\r\n",
                status_line,
                {
                    headermap
                        .iter()
                        .fold(String::new(), |mut acc, (key, value)| {
                            acc.push_str(&format!("{}: {}\r\n", key, value.to_str().unwrap()));
                            acc
                        })
                },
                response
            );

            let response = response.into_bytes();

            stream.write_all(response.as_slice()).unwrap();
        }
        "POST /clear_lines HTTP/1.1" => {
            state.lines.clear();

            state.changed_lines_sync = HashMap::new();

            state.clear_sync = Some(state.clients.iter().map(|(_, client)| client.id).collect());
        }
        _ => {
            status_line = Some("HTTP/1.1 404 NOT FOUND");
            filename = Some("public/404.html");
            content_type = Some("text/html");
        }
    };

    if status_line.is_none() || filename.is_none() || content_type.is_none() {
        return Ok(());
    }

    let status_line = status_line.unwrap();
    let filename = filename.unwrap();
    let content_type = content_type.unwrap();

    let contents = match content_type {
        "text/html" => {
            let mut string = fs::read_to_string(filename)?;
            for replace in replace_content {
                string = string.replace(&replace[0], &replace[1]);
            }
            string.into_bytes()
        }
        "text/javascript" => fs::read_to_string(filename)?.into_bytes(),
        "application/wasm" => fs::read(filename)?.into(),
        _ => Vec::<u8>::new(),
    };

    let length = contents.len();

    let headermap = prepare_headermap(content_type, length);

    let response = format!("{}\r\n{}\r\n", status_line, {
        headermap
            .iter()
            .fold(String::new(), |mut acc, (key, value)| {
                acc.push_str(&format!("{}: {}\r\n", key, value.to_str().unwrap()));
                acc
            })
    });

    let mut response = response.into_bytes();
    response.extend(contents);

    stream.write_all(response.as_slice()).unwrap();

    Ok(())
}

fn unwrap_content<'a>(
    content: Option<&'a String>,
    http_request: &[String],
) -> Result<&'a String, anyhow::Error> {
    let content = match content {
        Some(content) => content,
        None => {
            return Err(anyhow::anyhow!(
                "Did not get any content from request: {:?}",
                http_request
            ))?
        }
    };
    Ok(content)
}

fn unwrap_client_id(
    client_id: Option<ClientID>,
    stream: &mut std::net::TcpStream,
    peer: Peer,
) -> Result<ClientID, anyhow::Error> {
    let client_id = match client_id {
        Some(client_id) => client_id,
        None => {
            let status_line = "HTTP/1.1 412 PRECONDITION FAILED";

            stream.write_all(format!("{}\r\n", status_line).as_bytes())?;

            return Err(anyhow::anyhow!(
                "Client ID not found for peer: {}",
                peer.ip()?
            ))?;
        }
    };
    Ok(client_id)
}

fn prepare_headermap(content_type: &'static str, length: usize) -> http::HeaderMap {
    let mut headermap = http::HeaderMap::new();

    headermap.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headermap.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Origin, X-Requested-With, Content-Type, Accept"),
    );
    headermap.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headermap.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    headermap
}
