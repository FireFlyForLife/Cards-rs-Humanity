#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]
//! Application may have multiple data objects that are shared across
//! all handlers within same Application. Data could be added
//! with `App::data()` method, multiple different data objects could be added.
//!
//! > **Note**: http server accepts an application factory rather than an
//! application > instance. Http server constructs an application instance for
//! each thread, > thus application data
//! > must be constructed multiple times. If you want to share data between
//! different > threads, a shared object should be used, e.g. `Arc`.
//!
//! Check [user guide](https://actix.rs/book/actix-web/sec-2-application.html) for more info.

#[macro_use]
extern crate serde_derive;


use std::io;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_files as fs;
use actix_web_actors::ws;
use actix_session::{Session, CookieSession};

use uuid::Uuid;


mod cah_server;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
// A constant all cookies to be signed with
const COOKIE_SIGNED_KEY: [u8; 32] = [
    200,  2,  0,  0,  34,  75,  0,  0,
    0,  0,  3,  0,  0,  0,  0,  0,
    0,  9,  0,  0,  5,  0,  53,  0,
    8,  0,  0,  0,  0,  32,  0,  0];

fn session_get_uuid_or_default(session: &Session) -> Uuid{
    match session.get::<Uuid>("uuid"){
        Ok(Some(uuid)) => { uuid },
        _ => { 
            let uuid = Uuid::new_v4();
            if session.set("uuid", uuid).is_ok() {
                uuid
            } else {
                //TODO: Should this panick here?
                debug_assert!(false);
                Uuid::nil()
            }
        }
    }
}

/// do websocket handshake and start `MyWebSocket` actor
fn ws_index(r: HttpRequest, stream: web::Payload, session: Session) -> Result<HttpResponse, Error> {
    println!("{:?}", r);
    let uuid = session_get_uuid_or_default(&session);
    let res = ws::start(MyWebSocket::new(uuid), &r, stream);
    println!("{:?}", res.as_ref().unwrap());
    res
}

/// websocket connection is long running connection, it easier
/// to handle with an actor
struct MyWebSocket {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    user_id: Uuid,
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }

    fn stopping(&mut self, _context: &mut Self::Context) -> Running {

        Running::Stop
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for MyWebSocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        println!("WS: {:?}", msg);
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                if let Ok(json_message) = json::parse(&text) {
                    if json_message["type"] == "submitCard" && json_message["card_id"].is_number() {
                        println!("Player #{} has submitted card with id:{}", self.user_id, json_message["card_id"].as_number().unwrap());
                    }
                } else {
                    ctx.text(text);
                }
            },
            ws::Message::Binary(bin) => ctx.binary(bin),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

impl MyWebSocket {
    fn new(user_id: Uuid) -> Self {
        Self { hb: Instant::now(), user_id: user_id }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping("");
        });
    }
}

/// simple handle
fn index(state: web::Data<Mutex<usize>>, req: HttpRequest) -> HttpResponse {
    println!("{:?}", req);
    *(state.lock().unwrap()) += 1;

    HttpResponse::Ok().body(format!("Num of requests: {}", state.lock().unwrap()))
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let counter = web::Data::new(Mutex::new(0usize));

    //move is necessary to give closure below ownership of counter
    HttpServer::new(move || {
        App::new()
            .register_data(counter.clone()) // <- create app with shared state
            .wrap(CookieSession::signed(&COOKIE_SIGNED_KEY) // <- create cookie based session middleware
                .secure(false))
            // enable logger
            .wrap(middleware::Logger::default())
            
            // register simple handler, goto counter page
            .service(web::resource("/counter").to(index))
            .service(web::resource("/ws/").route(web::get().to(ws_index)))
            // the default website should display the index page located in the website folder and serve all css/js files relative to it.
            .service(fs::Files::new("/", "website").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
}