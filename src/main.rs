#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

#[macro_use]
extern crate serde_derive;
// #[macro_use]
// extern crate log;

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

use dotenv;

use futures::future::{Future, ok as fut_ok};
use tokio::io::{stdin, Stdin};
use tokio_codec::{FramedRead, LinesCodec};
use r2d2_sqlite;
use r2d2_sqlite::SqliteConnectionManager;

use str_macro::str;

pub mod cah_server;
pub mod messages;
pub mod db;

use cah_server::CardId;
use db::Pool;

/// How often heartbeat pings are sent for the websockets
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
    match session.get::<CookieToken>("ct"){
        Ok(Some(ct)) => { ct },
        _ => { 
            let ct = CookieToken::new_v4();
            if session.set("ct", ct).is_ok() {
                ct
            } else {
                //TODO: Should this panic here?
                debug_assert!(false);
                CookieToken::nil()
            }
        }
    }
}

/// do websocket handshake and start `MyWebSocket` actor
fn ws_index(r: HttpRequest, stream: web::Payload, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>, path: web::Path<(String,)>) -> Result<HttpResponse, Error> {
    println!("{:?}", r);
    println!("Trying to connect to: {}", &path.0);
    // let cookie_token = session_get_cookie_token_or_default(&session);
    if let Ok(Some(cookie_token)) = session.get::<CookieToken>("ct") {
        let res = ws::start(MyWebSocket::new(cookie_token, server_address.get_ref().clone(), path.0.clone()), &r, stream);
        println!("{:?}", res.as_ref().unwrap());
        res
    } else {
        println!("ERROR: No cookie token found. Are you not logged in?");
        Err(Error::from(()))
    }
}

/// handler with path parameters like `/user/{name}/`
fn get_join_match(req: HttpRequest, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>, path: web::Path<String>) -> Result<HttpResponse, Error> {
    println!("{:?}", req);

    if let Ok(Some(cookie_token)) = session.get::<CookieToken>("ct") {
        let match_name = path.clone();
        let async_req = server_address.send(messages::incomming::JoinMatch{match_name: match_name, token: cookie_token});
        let res = async_req.wait();
        match res {
            Ok(Ok(game_state)) => Ok(HttpResponse::build(StatusCode::OK).body(serde_json::to_string(&game_state).unwrap())),
            Ok(Err(error)) => Ok(HttpResponse::build(StatusCode::UNAUTHORIZED).body(error)),
            Err(error) => Err(Error::from(error)),
        }

        
    } else {
        Err(Error::from(()))
    }
}


fn get_list_rooms(_r: HttpRequest, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>) -> impl Future<Item = HttpResponse, Error = Error> {
    let token = session_get_cookie_token_or_default(&session);
    
    server_address.send(messages::incomming::ListRooms{cookie_token: token})
        .map_err(Error::from)
        .map( |matches| { HttpResponse::Ok().body(json::stringify(matches)) })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequestPayload {
    //TODO: Limit lengths characters and stuff
    pub username: String,
    pub password: String,
}

fn post_page_login(_r: HttpRequest, body: web::Form<LoginRequestPayload>, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>) -> impl Future<Item = HttpResponse, Error = Error> {
    server_address.send(messages::incomming::Login{username_or_email: body.username.clone(), password: body.password.clone()})
        .map_err(Error::from)
        .map(move |matches| {
            match matches {
                Ok(cookie_token) => {
                    let _cookie_succeeded = session.set("ct", cookie_token);
                    HttpResponse::Ok().body(cookie_token.to_string()) 
                },
                Err(error_message) =>  HttpResponse::build(StatusCode::UNAUTHORIZED).body(error_message)
            }
        })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequestPayload {
    //TODO: Limit lengths characters and stuff
    pub email: String,
    pub username: String,
    pub password: String,
}

fn post_page_register(_r: HttpRequest, body: web::Form<RegisterRequestPayload>, server_address: web::Data<Addr<cah_server::CahServer>>) -> impl Future<Item = HttpResponse, Error = Error> {
    server_address.send(messages::incomming::RegisterAccount{email: body.email.clone(), username: body.username.clone(), password: body.password.clone()})
        .map_err(Error::from)
        .map( |matches| {
            match matches {
                Ok(()) => HttpResponse::Ok().body("Succesfully registered!"),
                Err(error_message) =>  HttpResponse::build(StatusCode::UNAUTHORIZED).body(error_message)
            }
        })
}

/// websocket connection is long running connection, it easier
/// to handle with an actor
pub struct MyWebSocket {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    cookie_token: CookieToken,
    match_name: String,
    
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
        let connect_request = self.server_addr.send(messages::incomming::SocketConnectMatch{addr: addr.clone(), token: self.cookie_token.clone()});
        match connect_request.wait() {
            Ok(_) => {},
            Err(err_msg) => { 
                println!("ERROR while connecting websocket: '{}'", err_msg);
                self.server_addr.do_send(messages::incomming::Disconnect{token: self.cookie_token.clone()});
            }
        }
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        self.server_addr.do_send(messages::incomming::Leavematch{match_name: self.match_name.clone(), token: self.cookie_token.clone()});

        Running::Stop
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for MyWebSocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                println!("WS: {:?}", &text);

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
                            // if json_message["card_content"].is_string() {
                            //     let card_content: String = json_message["card_id"].as_str().unwrap().to_string();
                            //     let submit_card = messages::incomming::SubmitCard{token: self.cookie_token, card_id: card_content};
                            //     self.server_addr.do_send(submit_card);
                            // } else {
                            //     ctx.text(format!("{:?}", HttpResponse::build(StatusCode::BAD_REQUEST).reason("'card_content' is not a 'string' available in json request").finish()));
                            // }
                        },
                        "startGame" => {
                            self.server_addr.do_send(messages::incomming::StartMatch{token: self.cookie_token, match_name: self.match_name.clone()});
                        },
                        "revealCard" => {
                            if json_message["card_id"].is_number() {
                                let card_id: CardId = json_message["card_id"].as_number().unwrap().into();
                                // println!("Player {} has submitted card with id:{}", self.user_id, json_message["card_id"].as_number().unwrap());
                                let msg = messages::incomming::RevealCard{token: self.cookie_token.clone(), match_name: self.match_name.clone(), card_id: card_id};
                                self.server_addr.do_send(msg);
                            } else {
                                ctx.text(format!("{:?}", HttpResponse::build(StatusCode::BAD_REQUEST).reason("'card_id' is not a 'number' available in json request").finish()));
                            }
                        },
                        "czarChoice" => {
                            if json_message["card_id"].is_number() {
                                let card_id: CardId = json_message["card_id"].as_number().unwrap().into();
                                // println!("Player {} has submitted card with id:{}", self.user_id, json_message["card_id"].as_number().unwrap());
                                let czar_choice = messages::incomming::CzarChoice{token: self.cookie_token, match_name: self.match_name.clone(), card_id: card_id };
                                self.server_addr.do_send(czar_choice);
                            } else {
                                ctx.text(format!("{:?}", HttpResponse::build(StatusCode::BAD_REQUEST).reason("'card_id' is not a 'number' available in json request").finish()));
                            }
                        },
                        _ => {
                            println!("Unknown type of message received in websocket. type: {}, only supported types: submitCard, startGame. Full json message: {}", json_message["type"].as_str().unwrap(), text);
                        }
                    }

                } else {
                    ctx.text(text);
                }
            },
            ws::Message::Binary(bin) => { 
                println!("WS bin: {:?}", &bin);                
                ctx.binary(bin) 
            },
            ws::Message::Close(close_reason) => {
                println!("WS close: {:?}", &close_reason);
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

impl MyWebSocket {
    fn new(token: CookieToken, server_addr: Addr<cah_server::CahServer>, match_name: String) -> Self {
        Self { hb: Instant::now(), cookie_token: token, match_name: match_name, server_addr: server_addr }
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

#[derive(Message)]
pub struct StopServer;

pub struct AsyncCLI {
    input_future_handle: Option<SpawnHandle>,
    cah_addr: Addr<cah_server::CahServer>,
}
impl AsyncCLI {
    pub fn new(cah_addr: Addr<cah_server::CahServer>) -> Self {
        AsyncCLI {
            input_future_handle: None,
            cah_addr: cah_addr,
        }
    }
}
impl Actor for AsyncCLI {
    type Context = Context<AsyncCLI>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let stdin = tokio_stdin_stdout::stdin(0);
        // let stdout = tokio_stdin_stdout::stdout(0); // .make_sendable();

        let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
        
        let cah_addr_clone = self.cah_addr.clone();
        let framed_stdin_future = framed_stdin.for_each(move |line| {
            match &line[..] {
                "stop" => { let _ = cah_addr_clone.try_send(StopServer); },
                _ => {},
            }

            fut_ok(())
        }).map_err(|_err| {});
            // .and_then(move |line| {
            // // `and_then` above is not a Future's "and_then", it Stream's "and_then".
                

            //     fut_ok(())
            // })
            // .map(|_| {})
            // .map_err(|_| {});
        
        let actix_framed_stdin_future = actix::fut::wrap_future(framed_stdin_future);
        let input_future_handle = ctx.spawn( actix_framed_stdin_future );
        self.input_future_handle = Some(input_future_handle);
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        if let Some(input_future_handle) = self.input_future_handle {
            ctx.cancel_future(input_future_handle);
        }

        Running::Stop
    }
}


/// simple handle
fn counter_page(state: web::Data<Mutex<usize>>, req: HttpRequest) -> HttpResponse {
    println!("{:?}", req);
    *(state.lock().unwrap()) += 1;

    HttpResponse::Ok().body(format!("Num of requests: {}", state.lock().unwrap()))
}

fn get_card_deck(_r: HttpRequest, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>, path: web::Path<(String,)>) -> impl Future<Item=HttpResponse, Error=Error> {
    let token_result = session.get("ct");
    let deck_name = path.into_inner().0;
    
    match token_result {
        Ok(Some(cookie_token)) => server_address.send(messages::incomming::GetCards{token: cookie_token, deck_name: deck_name})
            .map_err(Error::from)
            .map(|deck_result| {
                match deck_result {
                                                //TODO: And Fix this this unwrap too
                    Ok(deck) => HttpResponse::Ok().body(serde_json::to_string(&deck).unwrap()),
                    Err(err_str) => HttpResponse::BadRequest().body(err_str)
                }
            }),
        _ => panic!("FFS COMPILER") //<<<--- TODO: FIX THIS HERE
    }
}

fn post_add_card(_r: HttpRequest, session: Session, body: web::Payload, server_address: web::Data<Addr<cah_server::CahServer>>, path: web::Path<(String, String)>) -> impl Future<Item=HttpResponse, Error=Error> {
    let cookie_token_result = session.get("ct").map_err(|err| format!("error getting cookie token: {}", err));
    
    web::block::<_, (CookieToken, String, bool), String>(move || {
        println!("RECEIVED ADD CARD THING");
        //TODO: Give a meaningful error message
        let cookie_token = cookie_token_result?.ok_or(str!("No cookie token available"))?;
        println!("COOKIE SESSION VALId");
        
        let is_black_string = &path.0;
        let is_black = 
            if is_black_string == "b" { true }
            else if is_black_string == "w" { false }
            else { return Err(format!("card type indentifier in the URL should be 'b' or 'w', but instead was: {}", is_black_string)); };

        println!("PARSED ISBLACK");

        let card_deck: String = path.1.clone();

        Ok((cookie_token, card_deck, is_black))
    })
    .map_err(|blocking_err| format!("Error while adding card: {}", blocking_err))
    .and_then(|(cookie_token, card_deck, is_black)| {
        body.concat2()
            .map_err(move |err| {println!("Error receiidng paylpoad: {}", err); format!("Error receiving payload string: {}", err) })
            .and_then(move |bytes| {
                web::block::<_, (CookieToken, String, String, bool), String>(move ||{
                    let card_content = String::from_utf8(bytes.to_vec()).map_err(|conversion_err| format!("Payload is not a valid UTF8 String! {}", conversion_err))?;
                    Ok((cookie_token, card_deck, card_content, is_black))
                })
                .map_err(|blocking_err| format!("Error while adding card: {}", blocking_err))
            })
    })
    .and_then(move |(cookie_token, card_deck, card_content, is_black)| server_address.send(messages::incomming::AddCard{token: cookie_token, deck_name: card_deck.clone(), card_content: card_content, is_black: is_black})
        .map_err(|mailbox_err| format!("Error adding card in mailbox: {}", mailbox_err))
        .map( |card_id_result| {
            match card_id_result {
                Ok(card_id) => HttpResponse::Ok().body(format!("{}", card_id)),
                Err(error_message) =>  HttpResponse::build(StatusCode::BAD_REQUEST).body(error_message)
            }
        }))
        //TODO: Return the actual error
    .map_err( |proper_error| { println!("error while trying to add card: {}", proper_error); Error::from(()) } ) 
}

fn post_del_card(_r: HttpRequest, session: Session, server_address: web::Data<Addr<cah_server::CahServer>>, path: web::Path<(String, CardId)>) -> impl Future<Item=HttpResponse, Error=Error> {
    let cookie_token = match session.get("ct") {
        Ok(Some(cookie_token)) => cookie_token,
        _ => {Default::default()}//return HttpResponse::BadRequest().body("No cookie token found, try logging in first.")
    };

    let deck_name = path.0.clone();
    let card_id = path.1;

    let msg = messages::incomming::DelCard{token: cookie_token, deck_name: deck_name, card_id: card_id};
    server_address.send(msg)
        //TODO: Actually return a proper error here
        .map_err(|postbox_err| {println!("postbox err: {}", postbox_err); Error::from(()) })
        .map(|_| HttpResponse::Ok().finish())
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    dotenv::dotenv().ok();

    env_logger::init();

    let counter = web::Data::new(Mutex::new(0usize));

    let sys = System::new("CrsH-Server");

    // Start N db executor actors (N = number of cores avail)
    // I moved this in the CahServer
    let manager = SqliteConnectionManager::file("db/some.db");
    let pool = Pool::new(manager).unwrap();

    let server = cah_server::CahServer::new(pool).start();
    let _async_cli = AsyncCLI::new(server.clone()).start();

    //move is necessary to give closure below ownership of counter
    HttpServer::new(move || {
        App::new()
            .register_data(counter.clone()) // <- create app with shared state
            .register_data(web::Data::new(server.clone()))
            // .register_data(web::Data::new(pool.clone()))
            .wrap(CookieSession::signed(&COOKIE_SIGNED_KEY) // <- create cookie based session middleware
                .secure(false))
            // enable logger
            .wrap(middleware::Logger::default())
            
            // register simple handler, goto counter page
            .service(web::resource("/counter").to(counter_page))
            // WebSocket connections go here
            .service(web::resource("/ws/{match}").route(web::get().to(ws_index)))
            .service( web::scope("/api/")
                .service(web::resource("/list_matches").route(web::get().to_async(get_list_rooms)))
                .service(web::resource("/login").route(web::post().to_async(post_page_login)))
                .service(web::resource("/register").route(web::post().to_async(post_page_register)))
                .service(web::resource("/join/{match}").route(web::get().to(get_join_match))) 
                .service(web::resource("/cards/{card_deck}").route(web::get().to_async(get_card_deck)))
                .service(web::resource("/add/{type}/{card_deck}").route(web::post().to_async(post_add_card)))
                .service(web::resource("/del/{card_deck}/{card_id}").route(web::post().to_async(post_del_card)))
                )
            // the default website should display the index page located in the website folder and serve all css/js files relative to it.
            .service(fs::Files::new("/", "website").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .start();

    let run_result = sys.run();

    println!("Server has gracefully shutdown!");

    run_result
}
