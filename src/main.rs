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
use actix_web::cookie::time::Duration;
use actix_session::config::PersistentSession;
const SECS_IN_10_YEARS: i64 = 60 * 60 * 24 * 7 * 4 * 12 * 10;

use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;

use rustunicodetests::hgk_compare_multiple_forms;
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

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveType {
    Practice = 0,
    FirstMoveMyTurn = 1,
    FirstMoveYourTurn = 2,

    AnswerMyTurn = 3,
    AskYourTurn = 4,
    AskMyTurn = 5,
    AnswerYourTurn = 6,

    GameOver = 7,
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
pub struct ResponseQuery {
    qtype: String,
    starting_form: String,
    change_desc: String,
    has_mf: bool,
    is_correct:bool,
    //answer: String,
}

#[derive(Deserialize,Serialize)]
pub struct CreateSessionQuery {
    qtype:String,
    unit: String,
    opponent:String,
}

#[derive(Deserialize,Serialize)]
pub struct SessionListRequest {
    practice:bool,
    game:bool,
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
pub struct GetMoveQuery {
    session_id:sqlx::types::Uuid,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct UserResult {
    user_id: sqlx::types::Uuid,
    user_name: String,
    password: String,
    email: String,
    timestamp: i64,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct SessionResult {
    session_id: Uuid, 
    challenger_user_id: Uuid,
    challenged_user_id: Option<Uuid>,
    timestamp: i64,
}

#[derive(Deserialize, Serialize, FromRow)]
pub struct MoveResult {
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

#[derive(Deserialize, Serialize)]
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
    time: Option<String>,//time for prev answer
    response_to:String,
    success:bool,
    mesg:Option<String>,
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
async fn get_move(
    (info, req, session): (web::Form<GetMoveQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(session) {
        
        let mut res = db::get_session_state(&db, user_id, info.session_id).await.map_err(map_sqlx_error)?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }

        res.response_to = "getmoves".to_string();
        res.success = true;
        res.mesg = None;

        return Ok(HttpResponse::Ok().json(res));
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
        };
        //let res = ("abc","def",);
    Ok(HttpResponse::Ok().json(res))
    }
}

#[allow(clippy::eval_order_dependence)]
async fn enter(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {

        //pull prev move from db to get verb and params
        let m = db::get_last_move(&db, info.session_id).await.map_err(map_sqlx_error)?;

        //test answer to get correct_answer and is_correct
        //let luw = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
        //let luwverb = Arc::new(HcGreekVerb::from_string(1, luw, REGULAR).unwrap());
        let prev_form = HcGreekVerbForm {verb:verbs[0].clone(), person:HcPerson::from_u8(m.person.unwrap()), number:HcNumber::from_u8(m.number.unwrap()), tense:HcTense::from_u8(m.tense.unwrap()), voice:HcVoice::from_u8(m.voice.unwrap()), mood:HcMood::from_u8(m.mood.unwrap()), gender:None, case:None};

        let correct_answer = prev_form.get_form(false).unwrap().last().unwrap().form.to_string();
        let is_correct = hgk_compare_multiple_forms(&correct_answer.replace('/', ","), &info.answer);

        let res = update_answer_move(
            db,
            info.session_id,
            user_id,
            &info.answer,
            &correct_answer,
            is_correct,
            &info.time,
            info.mf_pressed,
            info.timed_out,
            timestamp).await.map_err(map_sqlx_error)?;

        let mut res = db::get_session_state(&db, user_id, info.session_id).await.map_err(map_sqlx_error)?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "answerresponse".to_string();
        res.success = true;
        res.mesg = None;

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
    };
    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn ask(
    (info, req, session): (web::Form<AskQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        
        let _ = db::insert_ask_move(&db, user_id, info.session_id, info.person, info.number, info.tense, info.mood, info.voice, info.verb, timestamp).await.map_err(map_sqlx_error)?;

        let mut res = db::get_session_state(&db, user_id, info.session_id).await.map_err(map_sqlx_error)?;

        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "ask".to_string();
        res.success = true;
        res.mesg = None;

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
        };
        Ok(HttpResponse::Ok().json(res))
    }
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
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(web::resource("/enter").route(web::post().to(enter)))
        .service(web::resource("/new").route(web::post().to(create_session)))
        .service(web::resource("/list").route(web::post().to(get_sessions)))
        .service(web::resource("/getmove").route(web::post().to(get_move)))
        .service(web::resource("/ask").route(web::post().to(ask)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}
