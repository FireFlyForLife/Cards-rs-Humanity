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
#[macro_use]
extern crate log;

use std::io;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web::http::StatusCode;
use actix_files as fs;
use actix_web_actors::ws;
use actix_session::{Session, CookieSession};

use uuid::Uuid;


pub mod cah_server;
pub mod messages;

use cah_server::CardId;

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

type CookieToken = Uuid;

fn session_get_cookie_token_or_default(session: &Session) -> CookieToken {
    match session.get::<CookieToken>("uuid"){
        Ok(Some(uuid)) => { uuid },
        _ => { 
            let uuid = CookieToken::new_v4();
            if session.set("uuid", uuid).is_ok() {
                uuid
            } else {
                //TODO: Should this panic here?
                debug_assert!(false);
                CookieToken::nil()
            }
        }
    }
}

/// do websocket handshake and start `MyWebSocket` actor
fn ws_index(r: HttpRequest, stream: web::Payload, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>) -> Result<HttpResponse, Error> {
    println!("{:?}", r);
    let cookie_token = session_get_cookie_token_or_default(&session);
    let res = ws::start(MyWebSocket::new(cookie_token, server_address.get_ref().clone()), &r, stream);
    println!("{:?}", res.as_ref().unwrap());
    res
}

/// websocket connection is long running connection, it easier
/// to handle with an actor
struct MyWebSocket {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    cookie_token: CookieToken,
    
    server_addr: Addr<cah_server::CahServer>,
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<messages::outgoing::Message> for MyWebSocket {
    type Result = ();

    fn handle(&mut self, msg: messages::outgoing::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        self.server_addr.do_send(messages::incomming::Connect{addr: addr.recipient(), token: self.cookie_token.clone()});
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
                    if !json_message["type"].is_string() {
                        ctx.text(format!("{:?}", HttpResponse::build(StatusCode::BAD_REQUEST).reason("'type' is not available in json request").finish()));
                    }
                    match json_message["type"].as_str().unwrap() {
                        "submitCard" => {
                            if json_message["card_id"].is_number() {
                                let card_id: CardId = json_message["card_id"].as_number().unwrap().into();
                                // println!("Player {} has submitted card with id:{}", self.user_id, json_message["card_id"].as_number().unwrap());
                                let submit_card = messages::incomming::SubmitCard{token: self.cookie_token, card_id: card_id};
                                self.server_addr.do_send(submit_card);
                            } else {
                                ctx.text(format!("{:?}", HttpResponse::build(StatusCode::BAD_REQUEST).reason("'card_id' is not a 'number' available in json request").finish()));
                            }
                        }
                        _ => {
                            println!("Unknown type of message received in websocket. type: {}, only supported types: submitCard, connectMatch. Full json message: {}", json_message["type"].as_str().unwrap(), text);
                        }
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
    fn new(token: CookieToken, server_addr: Addr<cah_server::CahServer>) -> Self {
        Self { hb: Instant::now(), cookie_token: token, server_addr: server_addr }
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
fn counter_page(state: web::Data<Mutex<usize>>, req: HttpRequest) -> HttpResponse {
    println!("{:?}", req);
    *(state.lock().unwrap()) += 1;

    HttpResponse::Ok().body(format!("Num of requests: {}", state.lock().unwrap()))
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let counter = web::Data::new(Mutex::new(0usize));

    let sys = System::new("ws-example");
    let server = cah_server::CahServer::default().start();

    //move is necessary to give closure below ownership of counter
    HttpServer::new(move || {
        App::new()
            .register_data(counter.clone()) // <- create app with shared state
            .register_data(web::Data::new(server.clone()))
            .wrap(CookieSession::signed(&COOKIE_SIGNED_KEY) // <- create cookie based session middleware
                .secure(false))
            // enable logger
            .wrap(middleware::Logger::default())
            
            // register simple handler, goto counter page
            .service(web::resource("/counter").to(counter_page))
            // WebSocket connections go here
            .service(web::resource("/ws/").route(web::get().to(ws_index)))
            // the default website should display the index page located in the website folder and serve all css/js files relative to it.
            .service(fs::Files::new("/", "website").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .start();

    sys.run()
}