extern crate actix;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use actix_web::{server, App, HttpRequest, HttpResponse, HttpMessage, http, Result};
use actix_web::middleware::Logger;
use actix_web::middleware::session::{RequestSession, SessionStorage, CookieSessionBackend};
use actix_web::fs;
use actix_web::fs::{NamedFile};

use serde::Deserialize;

use actix::prelude::*;
use actix_web::{Json};
use futures::future::Future;

use bytes::Bytes;

use std::cell::Cell;
use std::path::Path;
use std::str;

// This struct represents state
struct AppState {
    counter: Cell<usize>,
}

type AppHttpRequest = HttpRequest<AppState>;

fn index(req: &AppHttpRequest) -> Result<NamedFile> {
    // access session data
    if let Some(count) = req.session().get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        req.session().set("counter", count+1)?;
    } else {
        req.session().set("counter", 1)?;
    }

    Ok( NamedFile::open(Path::new("website/index.html"))? )
}

#[derive(Debug, Serialize, Deserialize)]
struct CardSubmittedPayload {
    card_id: i64,
}

fn card_submitted(body: Json<CardSubmittedPayload>) -> HttpResponse {
    // let query = request.query();
    // println!("query object: {:?}", query);
    // println!("query body: {:?}", request.body().then(f: F));
    //let value = serde_json::to_value(str::from_utf8(body.to_vec().as_slice()).unwrap()).unwrap();

    // let card_id_option = query.get("cardId");
    // if card_id_option.is_none() {
        // return HttpResponse::new(http::StatusCode::from_u16(422u16).unwrap());
    // }
    // let card_id = card_id_option.unwrap();

    // println!("{}", str::from_utf8(body.to_vec().as_slice()).unwrap());
    println!("json body {:?}", body.card_id);

    HttpResponse::new(http::StatusCode::from_u16(200u16).unwrap())
}


const SERVER_ADDRESS: &str = "127.0.0.1:8080";
const COOKIE_SESSION_SIGNATURE: [u8; 32] = [0, 1, 0, 1, 2, 3, 0, 65, 0, 2, 4, 0, 3, 2, 10, 23, 10, 3, 10, 65, 21, 12, 32, 200, 250, 0, 12, 120, 43, 164, 123, 101];

fn main() {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();


    let sys = actix::System::new("basic-example");

    server::new(|| {
        App::with_state(AppState { counter: Cell::new(0) })
            .middleware(Logger::default())
            .middleware(SessionStorage::new(
                CookieSessionBackend::signed(&COOKIE_SESSION_SIGNATURE).secure(false)
            ))
            //TODO: Disable this in release
            .handler(
                "static", 
                fs::StaticFiles::new(".").expect("Cannot read static files in directory '.'").show_files_listing())
            // .route("submitCard", http::Method::POST, |req: AppHttpRequest| req.with )
            .resource("submitCard", |r| r.method(http::Method::POST).with(card_submitted))
            .handler(
                "/",
                fs::StaticFiles::new("website").expect("Cannot read static files in directory 'website'").index_file("index.html"))
            // .handler(
            //     "/data",
            //     fs::StaticFiles::new("pictures").expect("Cannot read static files in directory 'pictures'"))
            // .resource("/counter.html", |r| r.method(http::Method::GET).f(counter_page))
            // .resource("/", |r| r.method(http::Method::GET).f(index))
        })
        .bind(SERVER_ADDRESS).expect(&format!("Could not start server on: '{}'", SERVER_ADDRESS))
        .keep_alive(75)
        .start();

    println!("Started server on: '{}'", SERVER_ADDRESS);    

    let _ = sys.run();
}
