extern crate actix;

use actix_web::{server, App, HttpRequest, http, Result};
use actix_web::middleware::Logger;
use actix_web::middleware::session::{RequestSession, SessionStorage, CookieSessionBackend};
use actix_web::fs;
use actix_web::fs::{NamedFile};

use std::cell::Cell;
use std::path::Path;

// This struct represents state
struct AppState {
    counter: Cell<usize>,
}

fn index(req: &HttpRequest<AppState>) -> Result<NamedFile> {
    // access session data
    if let Some(count) = req.session().get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        req.session().set("counter", count+1)?;
    } else {
        req.session().set("counter", 1)?;
    }


    Ok( NamedFile::open(Path::new("website/index.html"))? )

    // Ok("Welcome!")
}

fn counter_page(req: &HttpRequest<AppState>) -> String {
    let count = req.state().counter.get() + 1; // <- get count
    req.state().counter.set(count); // <- store new count in state

    format!("Request number: {}", count) // <- response with count
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
