use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
    time::Instant,
};

use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpRequest, HttpServer, Responder};
use shared::Lines;

static LINES: LazyLock<Arc<Mutex<Lines>>> =
    LazyLock::new(|| Arc::new(Mutex::new(Lines::default())));

static CONNECTIONS: LazyLock<Arc<Mutex<BTreeMap<String, Instant>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(BTreeMap::new())));

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {}!", name)
}

#[get("/lines")]
async fn get_lines(req: HttpRequest) -> impl Responder {
    let ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_string();

    CONNECTIONS.lock().unwrap().insert(ip, Instant::now());

    LINES.lock().unwrap().to_string()
}

#[post("/lines")]
async fn post_lines(lines: web::Json<Lines>) -> impl Responder {
    let lines = lines.into_inner();
    LINES.lock().unwrap().update_from_other(lines);
    "ok"
}

#[post("/remove_lines")]
async fn remove_lines(ids: web::Json<Vec<u64>>) -> impl Responder {
    let mut lines = LINES.lock().unwrap();
    for id in ids.into_inner() {
        lines.remove(&id);
    }

    "ok"
}

#[post("/clear")]
async fn clear_lines() -> impl Responder {
    LINES.lock().unwrap().clear();
    "ok"
}

#[get("/num_connections")]
async fn num_connections() -> impl Responder {
    CONNECTIONS.lock().unwrap().len().to_string()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        let now = Instant::now();
        let mut connections = CONNECTIONS.lock().unwrap();
        connections.retain(|_, instant| now.duration_since(*instant).as_secs() < 30);
    });

    HttpServer::new(|| {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .service(greet)
            .service(get_lines)
            .service(post_lines)
            .service(remove_lines)
            .service(clear_lines)
            .service(num_connections)
    })
    .bind(("127.0.0.1", 8432))?
    .run()
    .await
}
