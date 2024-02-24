/*
hc-actix

Copyright (C) 2022  Jeremy March

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

mod login;
mod server;
mod session;

use actix_files as fs;
use actix_session::Session;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::Key;
use actix_web::cookie::SameSite;
use actix_web::http::header::HeaderValue;
use actix_web::http::header::{CONTENT_SECURITY_POLICY, STRICT_TRANSPORT_SECURITY};
use actix_web::{http::StatusCode, ResponseError};
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;

use libhc::dbpostgres::HcDbPostgres;
//use libhc::dbsqlite::HcDbSqlite;
use libhc::AnswerQuery;
use libhc::AskQuery;
use libhc::CreateSessionQuery;
use libhc::GetMoveQuery;
use libhc::GetMovesQuery;
use libhc::GetSessions;
use libhc::HcError;
use libhc::HcGreekVerb;
use libhc::MoveResult;
use libhc::MoveType;
use thiserror::Error;

use actix::Actor;
use actix::Addr;
use actix_web::Error;
use actix_web_actors::ws;
use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};

use actix_session::config::PersistentSession;
use actix_web::cookie::time::Duration;
const SECS_IN_10_YEARS: i64 = 60 * 60 * 24 * 7 * 4 * 12 * 10;

use rand::Rng;
use std::io;

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
//sqlite
// use sqlx::sqlite::SqliteConnectOptions;
// use sqlx::SqlitePool;
// use std::str::FromStr;

use sqlx::types::Uuid;

use crate::login::AppState; //for oauth

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[derive(Serialize)]
struct GetMovesResponse {
    response_to: String,
    session_id: Uuid,
    moves: Vec<MoveResult>,
    success: bool,
}

#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

/// Entry point for our websocket route
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::HcGameServer>>,
    session: Session,
) -> Result<HttpResponse, Error> {
    if let Some(uuid) = login::get_user_id(session.clone()) {
        //println!("uuid {:?}", uuid);
        let db = req.app_data::<HcDbPostgres>().unwrap();
        let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();
        let username = login::get_username(session);
        ws::start(
            session::WsHcGameSession {
                id: uuid,
                hb: Instant::now(),
                room: server::MAIN_ROOM,
                name: None,
                addr: srv.get_ref().clone(),
                verbs: verbs.clone(),
                db: db.clone(),
                username,
            },
            &req,
            stream,
        )
    } else {
        Ok(HttpResponse::InternalServerError().finish())
    }
}

//let updated_ip = get_ip(&req).unwrap_or_else(|| String::from(""));
//let user_agent = get_user_agent(&req).unwrap_or("");
fn _get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}
fn _get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

static INDEX_PAGE: &str = include_str!("index.html");
static CSP: &str = "style-src 'nonce-%NONCE%';script-src 'nonce-%NONCE%' 'wasm-unsafe-eval' \
                    'unsafe-inline'; object-src 'none'; base-uri 'none'";

async fn index_page() -> Result<HttpResponse, AWError> {
    let mut rng = rand::thread_rng();
    let csp_nonce: String = rng.gen::<u32>().to_string();

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header((CONTENT_SECURITY_POLICY, CSP.replace("%NONCE%", &csp_nonce)))
        .body(INDEX_PAGE.replace("%NONCE%", &csp_nonce)))
}

async fn get_sessions(
    (info, session, req): (web::Form<GetSessions>, Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session.clone()) {
        let username = login::get_username(session);

        let res = libhc::hc_get_sessions(db, user_id, verbs, username, &info)
            .await
            .map_err(map_hc_error)?;
        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn get_game_moves(
    (info, session, req): (web::Form<GetMovesQuery>, Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();

    if let Some(_user_id) = login::get_user_id(session.clone()) {
        let res = GetMovesResponse {
            response_to: String::from("getgamemoves"),
            session_id: info.session_id,
            moves: libhc::hc_get_game_moves(db, &info)
                .await
                .map_err(map_hc_error)?,
            success: true,
        };

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn create_session(
    (session, mut info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let timestamp = libhc::get_timestamp();

        let (mesg, success) =
            match libhc::hc_insert_session(db, user_id, &mut info, verbs, timestamp).await {
                Ok(_session_uuid) => (String::from("inserted!"), true),
                Err(HcError::UnknownError) => (String::from("opponent not found!"), false),
                Err(e) => (format!("error inserting: {e:?}"), false),
            };
        let res = StatusResponse {
            response_to: String::from("newsession"),
            mesg,
            success,
        };
        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn get_move(
    (info, req, session): (web::Form<GetMoveQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_get_move(db, user_id, false, info.session_id, verbs)
            .await
            .map_err(map_hc_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn enter(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_answer(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_hc_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn ask(
    (info, req, session): (web::Form<AskQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_ask(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_hc_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn mf(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_mf_pressed(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_hc_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

fn not_logged_in_response() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Unauthorized().finish())
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    error: String,
    message: String,
}

#[derive(Error, Debug)]
pub struct PhilologusError {
    code: StatusCode,
    name: String,
    error: String,
}

impl std::fmt::Display for PhilologusError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "PhilologusError: {} {}: {}.",
            self.code.as_u16(),
            self.name,
            self.error
        )
    }
}

impl ResponseError for PhilologusError {
    fn error_response(&self) -> HttpResponse {
        let error_response = ErrorResponse {
            code: self.code.as_u16(),
            message: self.error.to_string(),
            error: self.name.to_string(),
        };
        HttpResponse::build(self.code).json(error_response)
    }
}

pub fn map_hc_error(e: HcError) -> PhilologusError {
    match e {
        HcError::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("sqlx error"),
            error: format!("sqlx Configuration: {}", e),
        },
        HcError::AuthenticationError => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("authentication error"),
            error: String::from("authentication error"),
        },
        HcError::UnknownError => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("unknown error"),
            error: String::from("unknown error"),
        },
    }
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // start ws server actor
    let app_state = Arc::new(AtomicUsize::new(0));
    let server = server::HcGameServer::new(app_state.clone()).start();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://db.sqlite?mode=rwc
    // let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
    //     panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
    // });

    // let db_path = "testing.sqlite?mode=rwc";
    // let options = SqliteConnectOptions::from_str(db_path)
    //     .expect("Could not connect to db.")
    //     .foreign_keys(true)
    //     .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    //     .read_only(false)
    //     .collation("PolytonicGreek", |l, r| {
    //         l.to_lowercase().cmp(&r.to_lowercase())
    //     });

    // let pool = PgPoolOptions::new()
    //     .max_connections(5)
    //     .connect("postgres://jwm:1234@localhost/hc").await?;

    //e.g. export HOPLITE_DB=postgres://jwm:1234@localhost/hc
    let db_string = std::env::var("HOPLITE_DB")
        .unwrap_or_else(|_| panic!("Environment variable for db string not set: HOPLITE_DB."));

    let hcdb = HcDbPostgres {
        db: PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_string)
            .await
            .expect("Could not connect to db."),
    };
    /*
    let options = SqliteConnectOptions::from_str(":memory:")
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| {
            l.to_lowercase().cmp(&r.to_lowercase())
        });
    let hcdb = HcDbSqlite {
        db: SqlitePool::connect_with(options)
            .await
            .expect("Could not connect to db."),
    };
    */

    // let hcdb = HcDbPostgres { db: SqlitePool::connect_with(options)
    //     .await
    //     .expect("Could not connect to db.")
    // };

    libhc::hc_create_db(&hcdb)
        .await
        .expect("Error creating database");

    //1. to make a new key:
    // let secret_key = Key::generate(); // only for testing: should use same key from .env file/variable, else have to login again on each restart
    // println!("key: {}{}", hex::encode( secret_key.signing() ), hex::encode( secret_key.encryption() ));

    //2. a simple example testing key
    //https://docs.rs/cookie/0.16.0/src/cookie/secure/key.rs.html#35
    // let key: &Vec<u8> = &(0..64).collect();
    // let secret_key = Key::from(key);

    //3. to load from string
    // let string_key_64_bytes = "c67ba35ad969a3f4255085c359f120bae733c5a5756187aaffab31c7c84628b6a9a02ce6a1e923a945609a884f913f83ea50675b184514b5d15c3e1a606a3fd2";
    // let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    // let secret_key = Key::from(&key);

    //4. or load from env
    //e.g. export HCKEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285
    let string_key_64_bytes =
        std::env::var("HCKEY").unwrap_or_else(|_| panic!("Key env not set: HCKEY."));
    let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    let secret_key = Key::from(&key);

    let cookie_secure = !cfg!(debug_assertions); //cookie is secure for release, not secure for debug builds

    //for flash messages on login page
    let message_store = CookieMessageStore::builder(
        secret_key.clone(), /*Key::from(hmac_secret.expose_secret().as_bytes())*/
    )
    .secure(cookie_secure)
    .same_site(SameSite::Strict)
    .build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();

    HttpServer::new(move || {
        let oauth_apple_client = login::get_apple_client();
        let oauth_google_client = login::get_google_client();

        App::new()
            .app_data(AppState {
                apple_oauth: oauth_apple_client,
                google_oauth: oauth_google_client,
            })
            .app_data(libhc::hc_load_verbs("pp.txt"))
            .app_data(hcdb.clone())
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .wrap(
                middleware::DefaultHeaders::new()
                    // .add((CONTENT_SECURITY_POLICY,
                    //     HeaderValue::from_static("style-src 'nonce-2726c7f26c';\
                    //         script-src 'nonce-2726c7f26c' 'wasm-unsafe-eval' 'unsafe-inline'; object-src 'none'; base-uri 'none'")))
                    .add((
                        STRICT_TRANSPORT_SECURITY,
                        HeaderValue::from_static("max-age=31536000" /* 1 year */),
                    )),
            )
            .wrap(middleware::Compress::default()) // enable automatic response compression - usually register this first
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(cookie_secure) //cookie_secure must be false if testing without https
                    .cookie_same_site(SameSite::Strict) //none is required for oauth session variable oauth_state
                    .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                    .session_lifecycle(
                        PersistentSession::default()
                            .session_ttl(Duration::seconds(SECS_IN_10_YEARS)),
                    )
                    .cookie_name(String::from("hcid"))
                    .build(),
            )
            .wrap(message_framework.clone())
            .wrap(middleware::Logger::default()) // enable logger - always register Actix Web Logger middleware last
            .configure(config)
    })
    .workers(2)
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(index_page))
        .service(web::resource("/oauth-login-apple").route(web::get().to(login::oauth_login_apple)))
        .service(
            web::resource("/oauth-login-google").route(web::get().to(login::oauth_login_google)),
        )
        .service(web::resource("/auth").route(web::post().to(login::oauth_auth_apple)))
        .service(web::resource("/gauth").route(web::post().to(login::oauth_auth_google)))
        .route("/login", web::get().to(login::login_get))
        .route("/login", web::post().to(login::login_post))
        .route("/newuser", web::get().to(login::new_user_get))
        .route("/newuser", web::post().to(login::new_user_post))
        .route("/logout", web::get().to(login::logout))
        //.route("/ws", web::get().to(ws_route))
        .service(web::resource("/ws").route(web::get().to(ws_route)))
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(web::resource("/enter").route(web::post().to(enter)))
        .service(web::resource("/new").route(web::post().to(create_session)))
        .service(web::resource("/list").route(web::post().to(get_sessions)))
        .service(web::resource("/getmove").route(web::post().to(get_move)))
        .service(web::resource("/getgamemoves").route(web::post().to(get_game_moves)))
        .service(web::resource("/ask").route(web::post().to(ask)))
        .service(web::resource("/mf").route(web::post().to(mf)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}
