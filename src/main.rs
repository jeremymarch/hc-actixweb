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
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
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

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[derive(Deserialize)]
struct AnswerQuery {
    qtype: String,
    answer: String,
    time: String,
    mf_pressed: bool,
    timed_out: bool,
}

#[derive(Serialize)]
struct ResponseQuery {
    qtype: String,
    starting_form: String,
    change_desc: String,
    has_mf: bool,
    is_correct:bool,
    //answer: String,
}

#[derive(Deserialize,Serialize)]
struct CreateSessionQuery {
    qtype:String,
    unit: String,
    opponent:String,
}

#[derive(Deserialize,Serialize)]
struct SessionListRequest {
    practice:bool,
    game:bool,
}

#[derive(Deserialize,Serialize, FromRow)]
struct SessionsListQuery {
    session_id: sqlx::types::Uuid,
    opponent: Option<sqlx::types::Uuid>,
    opponent_name: Option<String>,
    timestamp: String,
    myturn: bool,
}

#[derive(Deserialize,Serialize, FromRow)]
struct MoveResult {
    move_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
    ask_user_id: sqlx::types::Uuid,
    answer_user_id: Option<sqlx::types::Uuid>,
    verb_id: Option<u32>,
    person: Option<u8>,
    number: Option<u8>,
    tense: Option<u8>,
    mood: Option<u8>,
    voice: Option<u8>,
    time: Option<String>,
    timed_out: Option<bool>,
    mf_pressed: Option<bool>,
    timestamp: i64,
}

#[derive(Deserialize,Serialize, FromRow)]
struct UserResult {
    user_id: sqlx::types::Uuid,
    user_name: String,
    password: String,
    email: String,
    timestamp: i64,
}

struct SessionDesc {
    session_id: Uuid,
    name: String,
    time_down: bool,
    unit: Option<u8>,
    custom_time: Option<u32>, //seconds
    custom_verbs: Vec<HcGreekVerb>,
    custom_persons: Vec<HcPerson>,
    custom_numbers: Vec<HcPerson>,
    custom_tenses: Vec<HcPerson>,
    custom_voices: Vec<HcPerson>,
    custom_moods: Vec<HcPerson>,
    timestamp_created: u32,
    user_id: u32,
    opponent_id: Option<u32>,
}

struct MoveDesc {
    move_id: Uuid,
    session_id: u32,
    verb_form: HcGreekVerbForm,
    is_correct: bool,
    time: String,
    timed_out: bool,
    mf_pressed: bool,
    answer: String,
    timestamp_created: u32,
    user_id: u32,
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

#[allow(clippy::eval_order_dependence)]
async fn get_sessions(
    (session, info, req): (Session, web::Form<SessionListRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let mut mesg = String::from("");
    if let Some(user_id) = login::get_user_id(session) {
        println!("************************LOGGED IN2 ");

        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");
        
        let res = db::get_sessions(&db, user_id).await.map_err(map_sqlx_error)?;
        Ok(HttpResponse::Ok().json(res))

    }
    else {
        mesg = "error inserting: not logged in".to_string();
        let res = ResponseQuery {
            qtype: "test".to_string(),
            starting_form: mesg,
            change_desc: "change_desc".to_string(),
            has_mf: false,
            is_correct: false,
            //answer: String,
        };
    
        //let res = ("abc","def",);
        Ok(HttpResponse::Ok().json(res))
    }
}

#[allow(clippy::eval_order_dependence)]
async fn create_session(
    (session, info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let mut mesg = String::from("");
    if let Some(user_id) = login::get_user_id(session) {
        println!("************************LOGGED IN ");

        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");

        let opponent_user_id = match db::get_user_id(&db, &info.opponent).await.map_err(map_sqlx_error) {
            Ok(o) => Some(o.user_id),
            Err(_) => None,
        };

        //failed to find opponent or opponent is self
        if (info.opponent.len() > 0 && opponent_user_id.is_none()) || (opponent_user_id.is_some() && opponent_user_id.unwrap() == user_id) {
            return Ok(HttpResponse::Ok().finish()); //todo oops
        }

        let unit = if let Ok(v) = info.unit.parse::<u32>() { Some(v) } else { None };
        
        match db::insert_session(&db, user_id, unit, opponent_user_id, timestamp).await {
            Ok(e) => {
                mesg = "inserted!".to_string();
            },
            Err(e) => {
                mesg = format!("error inserting: {:?}", e);
            }
        }
    }
    else {
        mesg = "error inserting: not logged in".to_string();
    }

    let res = ResponseQuery {
        qtype: "test".to_string(),
        starting_form: mesg,
        change_desc: "change_desc".to_string(),
        has_mf: false,
        is_correct: false,
        //answer: String,
    };

    //let res = ("abc","def",);
    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn enter(
    (info, req): (web::Form<AnswerQuery>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = ResponseQuery {
        qtype: "test".to_string(),
        starting_form: "starting_form".to_string(),
        change_desc: "change_desc".to_string(),
        has_mf: false,
        is_correct: false,
        //answer: String,
    };

    //let res = ("abc","def",);
    Ok(HttpResponse::Ok().json(res))
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

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    error: String,
    message: String,
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

    let options = SqliteConnectOptions::from_str(&db_path)
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| {
            l.to_lowercase().cmp(&r.to_lowercase())
        });

    let db_pool = SqlitePool::connect_with(options)
        .await
        .expect("Could not connect to db.");

    let res = db::create_db(&db_pool).await;
    if res.is_err() {
        println!("error: {:?}", res);
    }

    //let secret_key = Key::generate(); // TODO: Should be from .env file, else have to login again on each restart
    let key: &Vec<u8> = &(0..64).collect(); //todo
    let secret_key = Key::from(key);
    
    HttpServer::new(move || {

        App::new()
            .app_data(db_pool.clone())
            .wrap(middleware::Logger::default())
            .wrap(SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone()).cookie_secure(false).build()) //cookie_secure must be false if testing without https
            //.wrap(middleware::Compress::default())
            .configure(config)
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/login", web::get().to(login::login_get))
        .route("/login", web::post().to(login::login_post))
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(web::resource("/enter").route(web::post().to(enter)))
        .service(web::resource("/new").route(web::post().to(create_session)))
        .service(web::resource("/list").route(web::post().to(get_sessions)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}
