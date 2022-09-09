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

use uuid::Uuid;

//use mime;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
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
    unit: u32,
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

#[allow(clippy::eval_order_dependence)]
async fn create_session(
    (session, info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {

        let _ = db::insert_session(&db, user_id, info.unit).await;

    }
    else {
        println!("not logged in");
    }

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

    let secret_key = Key::generate(); // TODO: Should be from .env file
    
    HttpServer::new(move || {

        App::new()
            //.app_data(web::JsonConfig::default().error_handler(|err, _req| actix_web::error::InternalError::from_response(
            //    err, HttpResponse::Conflict().finish()).into()))
            //.wrap(json_cfg)
            .app_data(db_pool.clone())
            .wrap(middleware::Logger::default())
            //.wrap(auth_basic) //this blocks healthcheck
            .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
            .wrap(middleware::Compress::default())
            //.wrap(error_handlers)
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
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}
