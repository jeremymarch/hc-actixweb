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

use actix_web::{http::StatusCode, ResponseError};
use actix_web::cookie::Key;
use actix_session::Session;
use thiserror::Error;
use actix_files as fs;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web_flash_messages::FlashMessagesFramework;
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web::cookie::time::Duration;
use actix_session::config::PersistentSession;
const SECS_IN_10_YEARS: i64 = 60 * 60 * 24 * 7 * 4 * 12 * 10;

use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;

use polytonic_greek::hgk_compare_multiple_forms;
use crate::db::update_answer_move;
use std::sync::Arc;

use std::io;

//use uuid::Uuid;

use chrono::prelude::*;

//use mime;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use sqlx::FromRow;
use sqlx::types::Uuid;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

use hoplite_verbs_rs::*;
mod login;
mod db;
mod libhc;

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MoveType {
    Practice,
    FirstMoveMyTurn,
    FirstMoveTheirTurn,

    AnswerMyTurn,
    AskTheirTurn,
    AskMyTurn,
    AnswerTheirTurn,

    GameOver,
}

#[derive(Deserialize,Serialize,PartialEq,Eq,Debug)]
pub struct HCVerbOption {
    id:u32,
    verb:String,
}

#[derive(Deserialize)]
pub struct AnswerQuery {
    qtype: String,
    answer: String,
    time: String,
    mf_pressed: bool,
    timed_out: bool,
    session_id:Uuid,
}

#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

#[derive(Deserialize,Serialize)]
pub struct CreateSessionQuery {
    qtype:String,
    unit: String,
    opponent:String,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct SessionsListQuery {
    session_id: sqlx::types::Uuid,
    challenged: Option<sqlx::types::Uuid>, //the one who didn't start the game, or null for practice
    opponent: Option<sqlx::types::Uuid>,
    opponent_name: Option<String>,
    timestamp: String,
    myturn: bool,
    move_type:MoveType,
}

#[derive(Deserialize,Serialize)]
pub struct SessionsListResponse {
    response_to: String,
    sessions: Vec<SessionsListQuery>,
    success: bool,
    username: Option<String>,
}

#[derive(Deserialize,Serialize)]
pub struct GetMoveQuery {
    session_id:sqlx::types::Uuid,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct UserResult {
    user_id: sqlx::types::Uuid,
    user_name: String,
    password: String,
    email: String,
    user_type: u32,
    timestamp: i64,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct SessionResult {
    session_id: Uuid, 
    challenger_user_id: Uuid,
    challenged_user_id: Option<Uuid>,
    highest_unit: Option<u32>,
    custom_verbs: Option<String>, 
    max_changes: u8,
    challenger_score: Option<u32>,
    challenged_score: Option<u32>,
    timestamp: i64,
    // challenger_score:Option<u32>,
    // challenged_score:Option<u32>,
}

#[derive(Deserialize, Serialize, FromRow)]
pub struct MoveResult {
    move_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
    ask_user_id: Option<sqlx::types::Uuid>,
    answer_user_id: Option<sqlx::types::Uuid>,
    verb_id: Option<u32>,
    person: Option<u8>,
    number: Option<u8>,
    tense: Option<u8>,
    mood: Option<u8>,
    voice: Option<u8>,
    answer: Option<String>,
    correct_answer: Option<String>,
    is_correct: Option<u8>,
    time: Option<String>,
    timed_out: Option<bool>,
    mf_pressed: Option<bool>,
    asktimestamp: i64,
    answeredtimestamp: Option<i64>,
}

#[derive(Deserialize,Serialize)]
pub struct AskQuery {
    session_id: Uuid,
    person: u8,
    number: u8,
    tense: u8,
    voice: u8,
    mood: u8,
    verb: u32,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct SessionState {
    session_id: Uuid,
    move_type: MoveType,
    myturn: bool,
    starting_form:Option<String>,
    answer:Option<String>,
    is_correct: Option<bool>,
    correct_answer:Option<String>,
    verb: Option<u32>,
    person: Option<u8>,
    number: Option<u8>,
    tense: Option<u8>,
    voice: Option<u8>,
    mood: Option<u8>,
    person_prev: Option<u8>,
    number_prev: Option<u8>,
    tense_prev: Option<u8>,
    voice_prev: Option<u8>,
    mood_prev: Option<u8>,
    time: Option<String>, //time for prev answer
    response_to:String,
    success:bool,
    mesg:Option<String>,
    verbs:Option<Vec<HCVerbOption>>,
}

fn get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

async fn get_sessions(
    (session, req): (Session, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = login::get_user_id(session.clone()) {

        let username = login::get_username(session);

        //let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");
        
        let res = SessionsListResponse {
            response_to: "getsessions".to_string(),
            sessions: libhc::hc_get_sessions(db, user_id).await.map_err(map_sqlx_error)?,
            success: true,
            username,
        };

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = StatusResponse {
            response_to: "getsessions".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn create_session(
    (session, info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {

        let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");

        let (mesg, success) = match libhc::hc_insert_session(db, user_id, &info, verbs, timestamp).await {
            Ok(_session_uuid) => {
                ("inserted!".to_string(), true) 
            },
            Err(sqlx::Error::RowNotFound) => {
                ("opponent not found!".to_string(), false)
            },
            Err(e) => {
                (format!("error inserting: {:?}", e), false)
            }
        };
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg: mesg,
            success: success,
        };
        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn get_move(
    (info, req, session): (web::Form<GetMoveQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_get_move(db, user_id, &info, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
            verbs: None,
        };
        //let res = ("abc","def",);
        //Ok(HttpResponse::InternalServerError().finish())
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn enter(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {

        let res = libhc::hc_answer(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        return Ok(HttpResponse::Ok().json(res));
    }
    let res = SessionState {
        session_id: info.session_id,
        move_type: MoveType::Practice,
        myturn: false,
        starting_form:None,
        answer:None,
        is_correct: None,
        correct_answer:None,
        verb: None,
        person: None,
        number: None,
        tense: None,
        voice: None,
        mood: None,
        person_prev: None,
        number_prev: None,
        tense_prev: None,
        voice_prev: None,
        mood_prev: None,
        time: None,//time for prev answer
        response_to:"ask".to_string(),
        success:false,
        mesg:Some("not logged in".to_string()),
        verbs: None,
    };
    Ok(HttpResponse::Ok().json(res))
}

async fn ask(
    (info, req, session): (web::Form<AskQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_ask(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
            verbs: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn mf(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_mf_pressed(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
            verbs: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
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

fn map_sqlx_error(e: sqlx::Error) -> PhilologusError {
    match e {
        sqlx::Error::Configuration(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Configuration: {}", e),
        },
        sqlx::Error::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Database: {}", e),
        },
        sqlx::Error::Io(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Io: {}", e),
        },
        sqlx::Error::Tls(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Tls: {}", e),
        },
        sqlx::Error::Protocol(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Protocol: {}", e),
        },
        sqlx::Error::RowNotFound => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx RowNotFound".to_string(),
        },
        sqlx::Error::TypeNotFound { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx TypeNotFound".to_string(),
        },
        sqlx::Error::ColumnIndexOutOfBounds { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnIndexOutOfBounds".to_string(),
        },
        sqlx::Error::ColumnNotFound(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx ColumnNotFound: {}", e),
        },
        sqlx::Error::ColumnDecode { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnDecode".to_string(),
        },
        sqlx::Error::Decode(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Decode: {}", e),
        },
        sqlx::Error::PoolTimedOut => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolTimeOut".to_string(),
        },
        sqlx::Error::PoolClosed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolClosed".to_string(),
        },
        sqlx::Error::WorkerCrashed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx WorkerCrashed".to_string(),
        },
        sqlx::Error::Migrate(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Migrate: {}", e),
        },
        _ => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx Unknown error".to_string(),
        },
    }
}

fn load_verbs(path:&str) -> Vec<Arc<HcGreekVerb>> {
    let mut verbs = vec![];
    if let Ok(pp_file) = File::open(path) {
        let pp_reader = BufReader::new(pp_file);
        for (idx, pp_line) in pp_reader.lines().enumerate() {
            if let Ok(line) = pp_line {
                if !line.starts_with('#') { //skip commented lines
                    verbs.push(Arc::new(HcGreekVerb::from_string_with_properties(idx as u32, &line).unwrap()));
                }
            }
        }
    }
    verbs
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://db.sqlite?mode=rwc
    // let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
    //     panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
    // });
    let db_path = "testing.sqlite?mode=rwc";

    let options = SqliteConnectOptions::from_str(db_path)
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| {
            l.to_lowercase().cmp(&r.to_lowercase())
        });

    // let pool = PgPoolOptions::new()
    //     .max_connections(5)
    //     .connect("postgres://postgres:password@localhost/test").await?;

    let db_pool = SqlitePool::connect_with(options)
        .await
        .expect("Could not connect to db.");

    let res = db::create_db(&db_pool).await;
    if res.is_err() {
        println!("error: {:?}", res);
    }

    //1. to make a new key:
    // let secret_key = Key::generate(); // only for testing: should use same key from .env file/variable, else have to login again on each restart
    // println!("key: {}{}", hex::encode( secret_key.signing() ), hex::encode( secret_key.encryption() ));

    //2. a simple example testing key
    //https://docs.rs/cookie/0.16.0/src/cookie/secure/key.rs.html#35
    let key: &Vec<u8> = &(0..64).collect();
    let secret_key = Key::from(key);

    //3. to load from string
    // let string_key_64_bytes = "c67ba35ad969a3f4255085c359f120bae733c5a5756187aaffab31c7c84628b6a9a02ce6a1e923a945609a884f913f83ea50675b184514b5d15c3e1a606a3fd2";
    // let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    // let secret_key = Key::from(&key);

    //4. or load from env
    //e.g. export HCKEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285
    //let string_key_64_bytes = std::env::var("HCKEY").unwrap_or_else(|_| { panic!("Key env not set.") });
    //let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    //let secret_key = Key::from(&key);

    //for flash messages on login page
    let message_store = CookieMessageStore::builder( secret_key.clone() /*Key::from(hmac_secret.expose_secret().as_bytes())*/ ).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    
    HttpServer::new(move || {

        App::new()
            .app_data(load_verbs("../hoplite_verbs_rs/testdata/pp.txt"))
            .app_data(db_pool.clone())
            .wrap(middleware::Compress::default()) // enable automatic response compression - usually register this first
            .wrap(SessionMiddleware::builder(
                CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(false) //cookie_secure must be false if testing without https
                    .cookie_same_site(actix_web::cookie::SameSite::Strict)
                    .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                    .session_lifecycle(
                        PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_10_YEARS))
                    )
                    .cookie_name(String::from("hcid"))
                    .build())
            .wrap(message_framework.clone())
            .wrap(middleware::Logger::default()) // enable logger - always register Actix Web Logger middleware last
            .configure(config)
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/login", web::get().to(login::login_get))
        .route("/login", web::post().to(login::login_post))
        .route("/newuser", web::get().to(login::new_user_get))
        .route("/newuser", web::post().to(login::new_user_post))
        .route("/logout", web::get().to(login::logout))
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(web::resource("/enter").route(web::post().to(enter)))
        .service(web::resource("/new").route(web::post().to(create_session)))
        .service(web::resource("/list").route(web::post().to(get_sessions)))
        .service(web::resource("/getmove").route(web::post().to(get_move)))
        .service(web::resource("/ask").route(web::post().to(ask)))
        .service(web::resource("/mf").route(web::post().to(mf)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}


#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use crate::libhc::*;
    use crate::db::create_user;

    #[test]
    async fn test_index_post() {

        let verbs = load_verbs("../hoplite_verbs_rs/testdata/pp.txt");
        
        let db_path = "sqlite::memory:";
        let options = SqliteConnectOptions::from_str(&db_path)
            .expect("Could not connect to db.")
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .read_only(false)
            .collation("PolytonicGreek", |l, r| {
                l.to_lowercase().cmp(&r.to_lowercase())
            });
    
        let db = SqlitePool::connect_with(options)
            .await
            .expect("Could not connect to db.");
    
        let res = db::create_db(&db).await;
        if res.is_err() {
            println!("error: {:?}", res);
        }

        let mut timestamp = get_timestamp();

        // let uuid1 = Uuid::from_u128(0x8CD36EFFDF5744FF953B29A473D12347);
        // let uuid2 = Uuid::from_u128(0xD75B0169E7C343838298136E3D63375C);
        // let invalid_uuid = Uuid::from_u128(0x00000000000000000000000000000001);
        let uuid1 = create_user(&db, "testuser1", "abcdabcd", "user1@blah.com", timestamp).await.unwrap();
        let uuid2 = create_user(&db, "testuser2", "abcdabcd", "user2@blah.com", timestamp).await.unwrap();
        let invalid_uuid = create_user(&db, "testuser3", "abcdabcd", "user3@blah.com", timestamp).await.unwrap();

        let csq = CreateSessionQuery {
            qtype:"abc".to_string(),
            unit: "20".to_string(),
            opponent: "testuser2".to_string(),
        };

        let session_uuid = hc_insert_session(&db, uuid1, &csq, &verbs, timestamp).await;
        assert!(res.is_ok());

        let aq = AskQuery {
            session_id: *session_uuid.as_ref().unwrap(),
            person: 0,
            number: 0,
            tense: 0,
            voice: 0,
            mood: 0,
            verb: 0,
        };

        //ask from invalid user should be blocked
        let ask = hc_ask(&db, invalid_uuid, &aq, timestamp, &verbs).await;
        assert!(ask.is_ok() == false);

        //a valid ask
        let ask = hc_ask(&db, uuid1, &aq, timestamp, &verbs).await;
        assert!(ask.is_ok());

        //check that we are preventing out-of-sequence asks
        let ask = hc_ask(&db, uuid1, &aq, timestamp, &verbs).await;
        assert!(ask.is_ok() == false);

        let m = GetMoveQuery {
            session_id:*session_uuid.as_ref().unwrap(),
        };

        let ss = hc_get_move(&db, uuid1, &m, &verbs).await;

        let ss_res = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerTheirTurn, 
            myturn: false, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(0), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: None, 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let ss2 = hc_get_move(&db, uuid2, &m, &verbs).await;

        let ss_res2 = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerMyTurn, 
            myturn: true, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(0), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: None, 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        let answerq = AnswerQuery {
            qtype: "abc".to_string(),
            answer: "παιδεύω".to_string(),
            time: "25:01".to_string(),
            mf_pressed: false,
            timed_out: false,
            session_id:*session_uuid.as_ref().unwrap(),
        };

        //answer from invalid user should be blocked
        let answer = hc_answer(&db, invalid_uuid, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok() == false);

        //a valid answer
        let answer = hc_answer(&db, uuid2, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        assert_eq!(answer.unwrap().is_correct.unwrap(), true);

        //check that we are preventing out-of-sequence answers
        let answer = hc_answer(&db, uuid2, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok() == false);

        let ss = hc_get_move(&db, uuid1, &m, &verbs).await;

        let ss_res = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AskTheirTurn, 
            myturn: false, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: Some("παιδεύω".to_string()), 
            is_correct: Some(true), 
            correct_answer: Some("παιδεύω".to_string()), 
            verb: Some(0), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: Some("25:01".to_string()), 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };
        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let ss2 = hc_get_move(&db, uuid2, &m, &verbs).await;

        let ss_res2 = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AskMyTurn, 
            myturn: true, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: Some("παιδεύω".to_string()), 
            is_correct: Some(true), 
            correct_answer: Some("παιδεύω".to_string()), 
            verb: Some(0), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: Some("25:01".to_string()), 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);


        let aq2 = AskQuery {
            session_id: *session_uuid.as_ref().unwrap(),
            person: 1,
            number: 1,
            tense: 0,
            voice: 0,
            mood: 0,
            verb: 0,
        };

        timestamp += 1;
        //a valid ask
        let ask = hc_ask(&db, uuid2, &aq2, timestamp, &verbs).await;
        assert!(ask.is_ok());

        let ss = hc_get_move(&db, uuid1, &m, &verbs).await;
        assert!(ss.is_ok());
        let ss_res = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerMyTurn, 
            myturn: true, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(0), 
            person: Some(1), 
            number: Some(1), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: Some(0), 
            number_prev: Some(0), 
            tense_prev: Some(0), 
            voice_prev: Some(0), 
            mood_prev: Some(0), 
            time: None, 
            response_to: "getmoves".to_string(),
            success: true, 
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let ss2 = hc_get_move(&db, uuid2, &m, &verbs).await;

        let ss_res2 = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerTheirTurn, 
            myturn: false, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(0), 
            person: Some(1), 
            number: Some(1), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: Some(0), 
            number_prev: Some(0), 
            tense_prev: Some(0), 
            voice_prev: Some(0), 
            mood_prev: Some(0),  
            time: None, 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };
        assert!(ss2.unwrap() == ss_res2);

        //an incorrect answer
        timestamp += 1;
        let answerq = AnswerQuery {
            qtype: "abc".to_string(),
            answer: "παιδ".to_string(),
            time: "25:01".to_string(),
            mf_pressed: false,
            timed_out: false,
            session_id:*session_uuid.as_ref().unwrap(),
        };

        //a valid answer
        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        assert_eq!(answer.unwrap().is_correct.unwrap(), false);

        let ss = hc_get_move(&db, uuid1, &m, &verbs).await;

        let ss_res = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::FirstMoveMyTurn, 
            myturn: true, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: Some("παιδ".to_string()), 
            is_correct: Some(false), 
            correct_answer: Some("παιδεύετε".to_string()), 
            verb: Some(0), 
            person: Some(1), 
            number: Some(1), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: Some("25:01".to_string()), 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None, 
            verbs: Some(vec![/* take out paideuw: HCVerbOption { id: 0, verb: "παιδεύω".to_string() },*/ HCVerbOption { id: 113, verb: "—, ἀνερήσομαι".to_string() }, HCVerbOption { id: 114, verb: "—, ἐρήσομαι".to_string() }, HCVerbOption { id: 29, verb: "ἀγγέλλω".to_string() }, HCVerbOption { id: 23, verb: "ἄγω".to_string() }, HCVerbOption { id: 25, verb: "ἀδικέω".to_string() }, HCVerbOption { id: 73, verb: "αἱρέω".to_string() }, HCVerbOption { id: 74, verb: "αἰσθάνομαι".to_string() }, HCVerbOption { id: 110, verb: "αἰσχῡ\u{301}νομαι".to_string() }, HCVerbOption { id: 35, verb: "ἀκούω".to_string() }, HCVerbOption { id: 92, verb: "ἁμαρτάνω".to_string() }, HCVerbOption { id: 83, verb: "ἀναβαίνω".to_string() }, HCVerbOption { id: 42, verb: "ἀνατίθημι".to_string() }, HCVerbOption { id: 30, verb: "ἀξιόω".to_string() }, HCVerbOption { id: 36, verb: "ἀποδέχομαι".to_string() }, HCVerbOption { id: 43, verb: "ἀποδίδωμι".to_string() }, HCVerbOption { id: 99, verb: "ἀποθνῄσκω".to_string() }, HCVerbOption { id: 111, verb: "ἀποκρῑ\u{301}νομαι".to_string() }, HCVerbOption { id: 100, verb: "ἀποκτείνω".to_string() }, HCVerbOption { id: 112, verb: "ἀπόλλῡμι".to_string() }, HCVerbOption { id: 12, verb: "ἄρχω".to_string() }, HCVerbOption { id: 101, verb: "ἀφῑ\u{301}ημι".to_string() }, HCVerbOption { id: 120, verb: "ἀφικνέομαι".to_string() }, HCVerbOption { id: 44, verb: "ἀφίστημι".to_string() }, HCVerbOption { id: 84, verb: "βαίνω".to_string() }, HCVerbOption { id: 37, verb: "βάλλω".to_string() }, HCVerbOption { id: 13, verb: "βλάπτω".to_string() }, HCVerbOption { id: 102, verb: "βουλεύω".to_string() }, HCVerbOption { id: 38, verb: "βούλομαι".to_string() }, HCVerbOption { id: 52, verb: "γίγνομαι".to_string() }, HCVerbOption { id: 85, verb: "γιγνώσκω".to_string() }, HCVerbOption { id: 4, verb: "γράφω".to_string() }, HCVerbOption { id: 121, verb: "δεῖ".to_string() }, HCVerbOption { id: 60, verb: "δείκνῡμι".to_string() }, HCVerbOption { id: 39, verb: "δέχομαι".to_string() }, HCVerbOption { id: 31, verb: "δηλόω".to_string() }, HCVerbOption { id: 75, verb: "διαφέρω".to_string() }, HCVerbOption { id: 8, verb: "διδάσκω".to_string() }, HCVerbOption { id: 45, verb: "δίδωμι".to_string() }, HCVerbOption { id: 93, verb: "δοκέω".to_string() }, HCVerbOption { id: 16, verb: "δουλεύω".to_string() }, HCVerbOption { id: 94, verb: "δύναμαι".to_string() }, HCVerbOption { id: 9, verb: "ἐθέλω".to_string() }, HCVerbOption { id: 76, verb: "εἰμί".to_string() }, HCVerbOption { id: 95, verb: "εἶμι".to_string() }, HCVerbOption { id: 86, verb: "ἐκπῑ\u{301}πτω".to_string() }, HCVerbOption { id: 96, verb: "ἐλαύνω".to_string() }, HCVerbOption { id: 78, verb: "ἔξεστι(ν)".to_string() }, HCVerbOption { id: 61, verb: "ἐπανίσταμαι".to_string() }, HCVerbOption { id: 103, verb: "ἐπιβουλεύω".to_string() }, HCVerbOption { id: 62, verb: "ἐπιδείκνυμαι".to_string() }, HCVerbOption { id: 97, verb: "ἐπίσταμαι".to_string() }, HCVerbOption { id: 79, verb: "ἕπομαι".to_string() }, HCVerbOption { id: 53, verb: "ἔρχομαι".to_string() }, HCVerbOption { id: 63, verb: "ἐρωτάω".to_string() }, HCVerbOption { id: 77, verb: "ἔστι(ν)".to_string() }, HCVerbOption { id: 115, verb: "εὑρίσκω".to_string() }, HCVerbOption { id: 98, verb: "ἔχω".to_string() }, HCVerbOption { id: 104, verb: "ζητέω".to_string() }, HCVerbOption { id: 116, verb: "ἡγέομαι".to_string() }, HCVerbOption { id: 24, verb: "ἥκω".to_string() }, HCVerbOption { id: 10, verb: "θάπτω".to_string() }, HCVerbOption { id: 5, verb: "θῡ\u{301}ω".to_string() }, HCVerbOption { id: 105, verb: "ῑ\u{314}\u{301}ημι".to_string() }, HCVerbOption { id: 46, verb: "ἵστημι".to_string() }, HCVerbOption { id: 47, verb: "καθίστημι".to_string() }, HCVerbOption { id: 32, verb: "καλέω".to_string() }, HCVerbOption { id: 48, verb: "καταλῡ\u{301}ω".to_string() }, HCVerbOption { id: 122, verb: "κεῖμαι".to_string() }, HCVerbOption { id: 2, verb: "κελεύω".to_string() }, HCVerbOption { id: 20, verb: "κλέπτω".to_string() }, HCVerbOption { id: 117, verb: "κρῑ\u{301}νω".to_string() }, HCVerbOption { id: 17, verb: "κωλῡ\u{301}ω".to_string() }, HCVerbOption { id: 40, verb: "λαμβάνω".to_string() }, HCVerbOption { id: 64, verb: "λανθάνω".to_string() }, HCVerbOption { id: 87, verb: "λέγω".to_string() }, HCVerbOption { id: 21, verb: "λείπω".to_string() }, HCVerbOption { id: 3, verb: "λῡ\u{301}ω".to_string() }, HCVerbOption { id: 54, verb: "μανθάνω".to_string() }, HCVerbOption { id: 55, verb: "μάχομαι".to_string() }, HCVerbOption { id: 106, verb: "μέλλω".to_string() }, HCVerbOption { id: 33, verb: "μένω".to_string() }, HCVerbOption { id: 56, verb: "μεταδίδωμι".to_string() }, HCVerbOption { id: 57, verb: "μετανίσταμαι".to_string() }, HCVerbOption { id: 58, verb: "μηχανάομαι".to_string() }, HCVerbOption { id: 26, verb: "νῑκάω".to_string() }, HCVerbOption { id: 88, verb: "νομίζω".to_string() }, HCVerbOption { id: 118, verb: "οἶδα".to_string() }, HCVerbOption { id: 80, verb: "ὁράω".to_string() }, HCVerbOption { id: 65, verb: "παραγίγνομαι".to_string() }, HCVerbOption { id: 66, verb: "παραδίδωμι".to_string() }, HCVerbOption { id: 67, verb: "παραμένω".to_string() }, HCVerbOption { id: 41, verb: "πάσχω".to_string() }, HCVerbOption { id: 6, verb: "παύω".to_string() }, HCVerbOption { id: 14, verb: "πείθω".to_string() }, HCVerbOption { id: 1, verb: "πέμπω".to_string() }, HCVerbOption { id: 89, verb: "πῑ\u{301}πτω".to_string() }, HCVerbOption { id: 107, verb: "πιστεύω".to_string() }, HCVerbOption { id: 27, verb: "ποιέω".to_string() }, HCVerbOption { id: 18, verb: "πολῑτεύω".to_string() }, HCVerbOption { id: 15, verb: "πρᾱ\u{301}ττω".to_string() }, HCVerbOption { id: 90, verb: "προδίδωμι".to_string() }, HCVerbOption { id: 123, verb: "πυνθάνομαι".to_string() }, HCVerbOption { id: 108, verb: "συμβουλεύω".to_string() }, HCVerbOption { id: 81, verb: "συμφέρω".to_string() }, HCVerbOption { id: 109, verb: "συνῑ\u{301}ημι".to_string() }, HCVerbOption { id: 119, verb: "σύνοιδα".to_string() }, HCVerbOption { id: 22, verb: "σῴζω".to_string() }, HCVerbOption { id: 11, verb: "τάττω".to_string() }, HCVerbOption { id: 34, verb: "τελευτάω".to_string() }, HCVerbOption { id: 49, verb: "τίθημι".to_string() }, HCVerbOption { id: 28, verb: "τῑμάω".to_string() }, HCVerbOption { id: 124, verb: "τρέπω".to_string() }, HCVerbOption { id: 68, verb: "τυγχάνω".to_string() }, HCVerbOption { id: 69, verb: "ὑπακούω".to_string() }, HCVerbOption { id: 70, verb: "ὑπομένω".to_string() }, HCVerbOption { id: 125, verb: "φαίνω".to_string() }, HCVerbOption { id: 82, verb: "φέρω".to_string() }, HCVerbOption { id: 59, verb: "φεύγω".to_string() }, HCVerbOption { id: 91, verb: "φημί".to_string() }, HCVerbOption { id: 71, verb: "φθάνω".to_string() }, HCVerbOption { id: 50, verb: "φιλέω".to_string() }, HCVerbOption { id: 51, verb: "φοβέομαι".to_string() }, HCVerbOption { id: 7, verb: "φυλάττω".to_string() }, HCVerbOption { id: 72, verb: "χαίρω".to_string() }, HCVerbOption { id: 19, verb: "χορεύω".to_string() }, HCVerbOption { id: 126, verb: "χρή".to_string() }]),
        };
        //println!("{:?}\n\n{:?}", ss_res, ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let ss2 = hc_get_move(&db, uuid2, &m, &verbs).await;

        let ss_res2 = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AskTheirTurn, 
            myturn: false, 
            starting_form: Some("παιδεύω".to_string()), 
            answer: Some("παιδ".to_string()), 
            is_correct: Some(false), 
            correct_answer: Some("παιδεύετε".to_string()), 
            verb: Some(0), 
            person: Some(1), 
            number: Some(1), 
            tense: Some(0), 
            voice: Some(0), 
            mood: Some(0), 
            person_prev: Some(0), 
            number_prev: Some(0), 
            tense_prev: Some(0), 
            voice_prev: Some(0), 
            mood_prev: Some(0), 
            time: Some("25:01".to_string()), 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        //ask new verb after incorrect result
        let aq3 = AskQuery {
            session_id: *session_uuid.as_ref().unwrap(),
            person: 0,
            number: 0,
            tense: 1,
            voice: 1,
            mood: 1,
            verb: 1,
        };

        timestamp += 1;
        //a valid ask
        let ask = hc_ask(&db, uuid1, &aq3, timestamp, &verbs).await;
        assert!(ask.is_ok());

        let ss = hc_get_move(&db, uuid1, &m, &verbs).await;
        assert!(ss.is_ok());
        let ss_res = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerTheirTurn, 
            myturn: false, 
            starting_form: Some("πέμπω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(1), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(1), 
            voice: Some(1), 
            mood: Some(1), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None, 
            time: None, 
            response_to: "getmoves".to_string(),
            success: true, 
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let ss2 = hc_get_move(&db, uuid2, &m, &verbs).await;

        let ss_res2 = SessionState { 
            session_id: *session_uuid.as_ref().unwrap(), 
            move_type: MoveType::AnswerMyTurn, 
            myturn: true, 
            starting_form: Some("πέμπω".to_string()), 
            answer: None, 
            is_correct: None, 
            correct_answer: None, 
            verb: Some(1), 
            person: Some(0), 
            number: Some(0), 
            tense: Some(1), 
            voice: Some(1), 
            mood: Some(1), 
            person_prev: None, 
            number_prev: None, 
            tense_prev: None, 
            voice_prev: None, 
            mood_prev: None,  
            time: None, 
            response_to: "getmoves".to_string(), 
            success: true, 
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss2.as_ref().unwrap());
        //println!("2: {:?}", ss_res2);
        assert!(ss2.unwrap() == ss_res2);
    }
}
