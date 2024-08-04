/*
hc-axum

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

//use std::time::SystemTime;
//use std::time::UNIX_EPOCH;

use axum::extract::Query;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use socketioxide::socket::DisconnectReason;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use std::sync::atomic::AtomicUsize;
use tower_cookies::cookie::SameSite;
use tower_http::{/*cors::CorsLayer,*/ services::ServeDir};
use tracing::{/*error, Level,*/ info};
//use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use axum::response::Json;
use axum::response::{Html, IntoResponse};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use axum::debug_handler;
use axum::extract;
use axum::extract::State;
use http::StatusCode;
use sqlx::postgres::PgPoolOptions;
use time::Duration;
use tower_cookies::CookieManagerLayer;
use tower_sessions::{
    /*ExpiredDeletion, PostgresStore*/ Expiry, MemoryStore, Session, SessionManagerLayer,
};

use libhc::dbpostgres::HcDbPostgres;
use libhc::AnswerQuery;
use libhc::AskQuery;
use libhc::GetMoveQuery;
use libhc::GetMovesQuery;
use libhc::GetSessions;
use libhc::HcDb;
use libhc::HcGreekVerb;
use libhc::MoveResult;
use libhc::SessionState;
use libhc::SessionsListResponse;
use std::sync::Arc;

use libhc::synopsis;
use libhc::synopsis::SynopsisJsonResult;
use libhc::synopsis::SynopsisSaverRequest;

use uuid::Uuid;

mod login;

#[derive(Serialize, Deserialize)]
struct SynopsisResultUuid {
    id: Option<Uuid>,
    check: Option<bool>,
}

// use chrono::FixedOffset;
// use chrono::LocalResult;
// use chrono::TimeZone;

use libhc::CreateSessionQuery;
use libhc::HcError;
#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(transparent)]
struct Username(String);

#[derive(Clone)]
pub struct AxumAppState {
    hcdb: HcDbPostgres,
    verbs: Vec<Arc<HcGreekVerb>>,
}

#[derive(Serialize)]
struct GetMovesResponse {
    response_to: String,
    session_id: Uuid,
    moves: Vec<MoveResult>,
    success: bool,
}
//multiple states: https://stackoverflow.com/questions/75727029/axum-state-for-reqwest-client
//but we don't seem to need the following?
//use axum::extract::FromRef;
// impl FromRef<AxumAppState> for HcDbPostgres {
//     fn from_ref(app_state: &AxumAppState) -> HcDbPostgres {
//         app_state.hcdb.clone()
//     }
// }
// impl FromRef<AxumAppState> for Vec<Arc<HcGreekVerb>> {
//     fn from_ref(app_state: &AxumAppState) -> Vec<Arc<HcGreekVerb>> {
//         app_state.verbs.clone()
//     }
// }

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", untagged)]
enum Res {
    Login {
        #[serde(rename = "numUsers")]
        num_users: usize,
    },
    UserEvent {
        #[serde(rename = "numUsers")]
        num_users: usize,
        username: Username,
    },
    Message {
        username: Username,
        message: String,
    },
    Username {
        username: Username,
    },
}

static NUM_USERS: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // if std::env::var_os("RUST_LOG").is_none() {
    //     std::env::set_var("RUST_LOG", "hc-axum=debug,tower_http=debug")
    // }
    // let subscriber = FmtSubscriber::new();
    // tracing::subscriber::set_global_default(subscriber)?;

    // let subscriber = FmtSubscriber::builder()
    //     .with_line_number(true)
    //     .with_max_level(Level::DEBUG)
    //     .finish();
    // tracing::subscriber::set_global_default(subscriber)?;

    tracing_subscriber::registry()
        .with(EnvFilter::new(std::env::var("RUST_LOGxxx").unwrap_or_else(
            |_| {
                "hc_axum=debug,libhc=debug,sqlx=debug,tower_http=debug,hoplite_verbs_rs=debug"
                    .into()
            },
        )))
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    info!("Starting server");
    tracing::debug!("Starting server");

    /*
       on connect
       s.join(["self-my-uuid"])

       getmove
       s.join(["game-uuid"]).leave(["old-game-uuid"]);
    */

    let (_layer, io) = SocketIo::new_layer();
    io.ns("/", |s: SocketRef| {
        s.on("new message", |s: SocketRef, Data::<String>(msg)| {
            let username = s.extensions.get::<Username>().unwrap().clone();
            let msg = Res::Message {
                username,
                message: msg,
            };
            s.broadcast().emit("new message", msg).ok();
        });

        s.on("abc", |s: SocketRef, Data::<String>(msg)| {
            tracing::error!("abc received");
            //let username = s.extensions.get::<Username>().unwrap().clone();
            let msg = Res::Message {
                username: Username(String::from("blah")),
                message: msg,
            };
            s.broadcast().emit("abc", msg).ok();
        });

        s.on("add user", |s: SocketRef, Data::<String>(username)| {
            if s.extensions.get::<Username>().is_some() {
                return;
            }
            let i = NUM_USERS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            s.extensions.insert(Username(username.clone()));
            s.emit("login", Res::Login { num_users: i }).ok();

            let res = Res::UserEvent {
                num_users: i,
                username: Username(username),
            };
            s.broadcast().emit("user joined", res).ok();
        });

        s.on("typing", |s: SocketRef| {
            let username = s.extensions.get::<Username>().unwrap().clone();
            s.broadcast()
                .emit("typing", Res::Username { username })
                .ok();
        });

        s.on("stop typing", |s: SocketRef| {
            let username = s.extensions.get::<Username>().unwrap().clone();
            s.broadcast()
                .emit("stop typing", Res::Username { username })
                .ok();
        });

        s.on_disconnect(move |s: SocketRef, _: DisconnectReason| async move {
            if let Some(username) = s.extensions.get::<Username>() {
                let i = NUM_USERS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1;
                let res = Res::UserEvent {
                    num_users: i,
                    username: username.clone(),
                };
                s.broadcast().emit("user left", res).ok();
            }
        });
    });

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
    libhc::hc_create_db(&hcdb)
        .await
        .expect("Error creating database");

    let cookie_secure = !cfg!(debug_assertions);
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(cookie_secure)
        .with_expiry(Expiry::OnInactivity(Duration::days(365)))
        .with_name("hcax")
        .with_http_only(true)
        .with_same_site(SameSite::Strict); //None, Strict, Lax //oauth needs None, but Chrome needs at least Lax for normal login to work

    let verbs = libhc::hc_load_verbs("pp.txt");

    let app_state = AxumAppState { hcdb, verbs };

    let serve_dir = ServeDir::new("static"); //.not_found_service(axum::routing::get(index)); //not_found_service gives 404 status

    let app = axum::Router::new()
        .route("/", axum::routing::get(index))
        .route("/index.html", axum::routing::get(index))
        .route("/list", axum::routing::post(get_sessions))
        .route("/new", axum::routing::post(create_session))
        .route("/getmove", axum::routing::post(get_move))
        .route("/getgamemoves", axum::routing::post(get_game_moves))
        .route("/enter", axum::routing::post(enter))
        .route("/mf", axum::routing::post(mf))
        .route("/ask", axum::routing::post(ask))
        .route("/login", axum::routing::get(login::login_get))
        .route("/login", axum::routing::post(login::login_post))
        .route(
            "/oauth-login-apple",
            axum::routing::get(login::oauth_login_apple),
        )
        .route(
            "/oauth-login-google",
            axum::routing::get(login::oauth_login_google),
        )
        .route("/auth", axum::routing::post(login::oauth_auth_apple))
        .route("/gauth", axum::routing::post(login::oauth_auth_google))
        .route("/newuser", axum::routing::get(login::new_user_get))
        .route("/newuser", axum::routing::post(login::new_user_post))
        .route("/logout", axum::routing::get(login::logout))
        .route("/healthzzz", axum::routing::get(health_check))
        // .route(
        //     "/greek-synopsis-result",
        //     axum::routing::get(greek_synopsis_result),
        // )
        .route(
            "/greek-synopsis-results",
            axum::routing::get(greek_synopsis_list),
        )
        .route("/greek-synopsis", axum::routing::get(greek_synopsis))
        .route(
            "/greek-synopsis-saver",
            axum::routing::post(greek_synopsis_saver),
        )
        .route("/sgi", axum::routing::get(sgi_schedule))
        // .route("/latin-synopsis-result", axum::routing::get(latin_synopsis_result))
        // .route("/latin-synopsis-list", axum::routing::get(latin_synopsis_list))
        // .route("/latin-synopsis-saver", axum::routing::get(latin_synopsis_saver))
        // .route("/latin-synopsis", axum::routing::get(latin_synopsis))
        .route("/synopsis-json", axum::routing::post(synopsis_json))
        .fallback_service(serve_dir) //for js, wasm, etc
        // .layer(
        //     ServiceBuilder::new()
        //         .layer(CorsLayer::permissive()) // Enable CORS policy
        //         .layer(layer.with_hyper_v1()),
        // )
        .with_state(app_state)
        .layer(session_layer)
        .layer(CookieManagerLayer::new());

    // let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8088));
    // let server = Server::bind(&addr).serve(app.into_make_service());
    // if let Err(e) = server.await {
    //     error!("Error starting axum server: {}", e);
    // }
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

static SYNOPSIS_PAGE: &str = include_str!("greek-synopsis.html");

static INDEX_PAGE: &str = include_str!("../../hc-actix/src/index.html");
static CSP: &str = "style-src 'nonce-%NONCE%';script-src 'nonce-%NONCE%' 'wasm-unsafe-eval' \
                    'unsafe-inline'; object-src 'none'; base-uri 'none'";
static CSP_HEADER: &str = "content-security-policy";

async fn index() -> impl IntoResponse {
    // let mut rng = rand::thread_rng();
    // let csp_nonce: String = rng.gen::<u32>().to_string();
    let csp_nonce: String = Uuid::new_v4().to_string(); //.simple().encode_upper(&mut Uuid::encode_buffer()).to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(CSP_HEADER),
        HeaderValue::from_str(&CSP.replace("%NONCE%", &csp_nonce)).unwrap(),
    );

    let page = INDEX_PAGE.replace("%NONCE%", &csp_nonce);
    (headers, Html(page))
}

#[debug_handler]
async fn get_sessions(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<GetSessions>,
) -> Result<Json<SessionsListResponse>, StatusCode> {
    if let Some(user_id) = login::get_user_id(&session).await {
        //uuid!("96b875e7-fc53-4498-ad8d-9ce417e938b7");
        let username = login::get_username(&session).await;

        let res = libhc::hc_get_sessions(&state.hcdb, user_id, &state.verbs, username, &payload)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn create_session(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(mut payload): extract::Form<CreateSessionQuery>,
) -> Result<Json<StatusResponse>, StatusCode> {
    if let Some(user_id) = login::get_user_id(&session).await {
        let timestamp = libhc::get_timestamp();

        let (mesg, success) = match libhc::hc_insert_session(
            &state.hcdb,
            user_id,
            &mut payload,
            &state.verbs,
            timestamp,
        )
        .await
        {
            Ok(_session_uuid) => (String::from("inserted!"), true),
            Err(HcError::UnknownError) => (String::from("opponent not found!"), false),
            Err(e) => (format!("error inserting: {e:?}"), false),
        };
        let res = StatusResponse {
            response_to: String::from("newsession"),
            mesg,
            success,
        };
        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[debug_handler]
async fn greek_synopsis(
    session: Session,
    Query(id): axum::extract::Query<SynopsisResultUuid>,
    State(state): State<AxumAppState>,
) -> impl IntoResponse {
    let mut json = String::from("false");

    if let Some(a) = id.id {
        if let Some(res) = synopsis::get_synopsis_result(a, &state.hcdb).await {
            json = serde_json::to_string(&res).unwrap();
        }
    }

    let csp_nonce: String = Uuid::new_v4().to_string(); //.simple().encode_upper(&mut Uuid::encode_buffer()).to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(CSP_HEADER),
        HeaderValue::from_str(&CSP.replace("%NONCE%", &csp_nonce)).unwrap(),
    );

    //let user_id = login::get_user_id(&session);
    let username = login::get_username(&session).await;
    let name = if username.is_some() {
        format!("const username = '{}';", username.unwrap())
    } else {
        String::from("const username = false;")
    };

    let show_check: bool = id.check.is_some();

    let page = SYNOPSIS_PAGE
        .replace("%NONCE%", &csp_nonce)
        .replace("const username = false;", name.as_str())
        .replace(
            "const resultJson = false;",
            format!("const resultJson = {};", json).as_str(),
        )
        .replace("%SHOWCHECK%", if show_check { "initial" } else { "none" });

    (headers, Html(page))
}

use chrono::Days;
use chrono::NaiveDate;
use chrono::NaiveTime;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;

#[derive(PartialEq)]
enum LgiDayType {
    Normal,
    RestAndStudy,
    SundayReview,
    Holiday,
}

#[derive(PartialEq)]
enum LgiClassType {
    None,
    MorningOptional,
    Drill1,
    Drill2,
    NoonOptional,
    Lecture,
    ProseComp,
    ProseCompSmall,
    VocNotes,
    SpecialLecture,
    FridayReview,
    Exam,
    AfternoonOptional,
    Elective,
    SundayReview,
    RestAndStudy,
}

struct LgiDoc {
    desc: String,
    docx: String,
    pdf: String,
}

struct LgiSection {
    group: String,
    faculty: String,
    room: String,
    name: String,
}

struct LgiClass {
    class_type: LgiClassType,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
    desc: String,
    faculty: String,
    room: String,
    docs: Vec<LgiDoc>,
    sections: Vec<LgiSection>,
    name: String,
}

struct LgiDay {
    day_type: LgiDayType,
    day: NaiveDate,
    day_num: u32,
    week: u32,
    classes: Vec<LgiClass>,
}

struct LgiCourse {
    day1: NaiveDate,
    days: Vec<LgiDay>,
    holidays: Vec<NaiveDate>,
}

fn is_holiday(date: &NaiveDate, holidays: &Vec<NaiveDate>) -> bool {
    for h in holidays.iter() {
        if date == h {
            return true;
        }
    }
    false
}

fn make_schedule() -> LgiCourse {
    let xml: &str = include_str!("sgi.xml");
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    //let mut count = 0;
    let mut txt = Vec::new();
    let mut buf = Vec::new();

    let mut sgi = LgiCourse {
        day1: NaiveDate::parse_from_str("2000-01-01", "%Y-%m-%d").unwrap(),
        days: vec![],
        holidays: vec![],
    };

    let mut day_count = 0;
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            Ok(Event::Eof) => break, // exits the loop when reaching end of file

            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"sgi" => {
                        for a in e.attributes() {
                            if a.as_ref().unwrap().key == QName(b"day1") {
                                let d = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                                sgi.day1 = NaiveDate::parse_from_str(&d, "%Y-%m-%d").unwrap();
                            }
                        }
                    }
                    b"holiday" => {
                        for a in e.attributes() {
                            if a.as_ref().unwrap().key == QName(b"date") {
                                let d = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                                sgi.holidays
                                    .push(NaiveDate::parse_from_str(&d, "%Y-%m-%d").unwrap());
                            }
                            // else if a.as_ref().unwrap().key == QName(b"name") {
                            //     let d = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            //     sgi.day1 = NaiveDate::parse_from_str(&d, "%Y-%m-%d").unwrap();
                            // }
                        }
                    }
                    b"day" => {
                        let mut day_num = String::from("0");
                        for a in e.attributes() {
                            if a.as_ref().unwrap().key == QName(b"n") {
                                day_num =
                                    std::str::from_utf8(&a.unwrap().value).unwrap().to_string()
                            }
                        }
                        let mut the_day = sgi.day1 + Days::new(day_count);
                        if is_holiday(&the_day, &sgi.holidays) {
                            let day = LgiDay {
                                day_type: LgiDayType::Holiday,
                                day: the_day,
                                day_num: 0,
                                week: 0,
                                classes: vec![],
                            };

                            sgi.days.push(day);
                            day_count += 1;
                            the_day = sgi.day1 + Days::new(day_count);
                        } else if the_day.format("%A").to_string() == "Saturday" {
                            let day = LgiDay {
                                day_type: LgiDayType::RestAndStudy,
                                day: the_day,
                                day_num: 0,
                                week: 0,
                                classes: vec![],
                            };

                            sgi.days.push(day);
                            day_count += 1;
                            the_day = sgi.day1 + Days::new(day_count);

                            let day = LgiDay {
                                day_type: LgiDayType::SundayReview,
                                day: the_day,
                                day_num: 0,
                                week: 0,
                                classes: vec![],
                            };

                            sgi.days.push(day);
                            day_count += 1;
                            the_day = sgi.day1 + Days::new(day_count);
                        }

                        let day = LgiDay {
                            day_type: LgiDayType::Normal,
                            day: the_day,
                            day_num: day_num.parse::<u32>().unwrap(),
                            week: 0,
                            classes: vec![],
                        };

                        sgi.days.push(day);
                        day_count += 1;
                    }
                    b"class" => {
                        let mut faculty = String::from("");
                        let mut start = String::from("");
                        let mut end = String::from("");
                        let mut name = String::from("");
                        let mut item_type = String::from("");
                        for a in e.attributes() {
                            if a.as_ref().unwrap().key == QName(b"start") {
                                start = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"end") {
                                end = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"who") {
                                faculty =
                                    std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"name") {
                                name = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"type") {
                                item_type =
                                    std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            }
                        }
                        let c = LgiClass {
                            class_type: match item_type.as_str() {
                                "vocNotes" => LgiClassType::VocNotes,
                                "morningOptional" => LgiClassType::MorningOptional,
                                "noonOptional" => LgiClassType::NoonOptional,
                                _ => LgiClassType::None,
                            },
                            start_time: if let Ok(a) = NaiveTime::parse_from_str(&start, "%I:%M %P")
                            {
                                Some(a)
                            } else {
                                None
                            },
                            end_time: if let Ok(a) = NaiveTime::parse_from_str(&end, "%I:%M %P") {
                                Some(a)
                            } else {
                                None
                            },
                            desc: String::from(""),
                            faculty: faculty,
                            room: String::from(""),
                            docs: vec![],
                            sections: vec![],
                            name: name,
                        };
                        sgi.days.last_mut().unwrap().classes.push(c);

                        // let days: &mut Vec<LgiDay> = &mut sgi.days;
                        // let day: &mut LgiDay = &mut days.last().unwrap();
                        // day.classes.push(c);
                    }
                    b"section" => {
                        let mut faculty = String::from("");
                        let mut group = String::from("");
                        let mut room = String::from("");
                        let mut name = String::from("");
                        for a in e.attributes() {
                            if a.as_ref().unwrap().key == QName(b"who") {
                                faculty =
                                    std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"group") {
                                group = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"room") {
                                room = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            } else if a.as_ref().unwrap().key == QName(b"name") {
                                name = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                            }
                        }

                        let s = LgiSection {
                            group: group,
                            faculty: faculty,
                            room: room,
                            name: name,
                        };
                        sgi.days
                            .last_mut()
                            .unwrap()
                            .classes
                            .last_mut()
                            .unwrap()
                            .sections
                            .push(s);
                    }
                    b"doc" => {}
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => txt.push(e.unescape().unwrap().into_owned()),

            _ => (),
        }
        // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }
    sgi
}

// sgiDropbox
//     handouts
//     years
//         2023
//         2024

async fn sgi_schedule(session: Session) -> impl IntoResponse {
    let _user_id = login::get_user_id(&session).await;
    let _username = login::get_username(&session).await;

    let sgi = make_schedule();

    let mut res = String::from("<!DOCTYPE html><html><head><style>td {text-align:center;vertical-align:middle;}</style></head><body><table cellspacing=0 cellpadding=0 border=1 style='width: 90%;margin: 0px auto;'>");
    for i in sgi.days.iter() {
        res.push_str("<tr><td>");
        res.push_str(i.day.format("%A").to_string().as_str());
        res.push_str("<br/>");
        res.push_str(i.day.format("%B %d").to_string().as_str());
        if i.day_num > 0 {
            res.push_str("<br/>Day ");
            res.push_str(i.day_num.to_string().as_str());
        }
        res.push_str("</td>");
        for j in i.classes.iter() {
            res.push_str("<td>");
            if j.start_time.is_some() {
                res.push_str(j.start_time.unwrap().format("%l:%M").to_string().as_str());
            }
            if j.start_time.is_some() && j.end_time.is_some() {
                res.push_str(" - ");
            }
            if j.end_time.is_some() {
                res.push_str(j.end_time.unwrap().format("%l:%M").to_string().as_str());
            }
            if !j.name.is_empty() {
                res.push_str("<br/>");
                res.push_str(&j.name)
            } else if j.class_type == LgiClassType::VocNotes {
                res.push_str("<br/>Vocabulary Notes");
            } else if j.class_type == LgiClassType::MorningOptional
                || j.class_type == LgiClassType::NoonOptional
            {
                res.push_str("<br/>(optional)");
            }
            if j.sections.len() == 0 {
                res.push_str("<br/>");
                res.push_str(&j.faculty);
            }
            for k in j.sections.iter() {
                res.push_str("<br/>");

                if !k.name.is_empty() {
                    res.push_str(&k.name);
                    res.push_str(" – ");
                } else if !k.group.is_empty() {
                    res.push_str(&k.group);
                    res.push_str(" – ");
                }

                res.push_str(&k.faculty);
            }
            res.push_str("</td>");
        }
        res.push_str("</tr>");
    }
    res.push_str("</table></body></html>");

    Html(res)
}

async fn greek_synopsis_list(
    session: Session,
    State(state): State<AxumAppState>,
) -> impl IntoResponse {
    let user_id = login::get_user_id(&session).await;
    let username = login::get_username(&session).await;

    let mut tx = state.hcdb.begin_tx().await.unwrap();
    let list = tx.greek_get_synopsis_list(user_id).await.unwrap();
    tx.commit_tx().await.unwrap();

    let mut res = String::from(
        r#"<!DOCTYPE html>
    <html>
    <head>
    <meta charset="UTF-8">
    <style nonce="2726c7f26c">
    @font-face {
        font-family: 'WebNewAthenaUnicode';
        src: url('/newathu5_8.ttf') format('truetype');
      }
        body {
            font-family: helvetica, arial;
            margin:0px;
        }
        .synlist { width: 600px;
            margin: 0px auto;
            border-collapse: collapse;
            font-size: 16pt;
            font-family:helvetica,arial;
        }
        .synlist td { padding: 3px; }
        .headerrow {border-bottom:1px solid black;font-weight:bold;}
    #hamburgercontainer {
        padding: 0px;
        margin: 0px;
      }
      #hamburger {
        background-color: white;
        border: 1px solid #666;
        border-radius: 4px;
        cursor: pointer;
        height: 20px;
        width: 20px;
      }
      #hamburger {
        z-index: 999;
        position: relative;
      }
      #hamburger rect {
        fill: black;
      }
      #appTitle {
        font-weight: bold;
        position: relative;
        right: 10px;
      }
      #menubar {
        height: 1.5rem;
        border-bottom: 1px solid black;
        display: flex;
        justify-content: flex-end;
        background-color: white;
        position: relative;
        z-index: 900;
        padding: 0.2rem 1rem;
      }
      #loginContainer {
        padding: 0px 20px;
      }
      #loginlink {
        display: inline;
      }
      #logoutlink {
        display: none;
      }
      #newSynopsisLink {
        display:none;
        position:absolute;
        left: 10px;
      }
      #table1 {
        display:none;
      }
      .loggedin #loginlink { display: none; }
      .loggedin #logoutlink { display: inline; }
      .loggedin #newSynopsisLink { display: inline; }
      .loggedin #table1 { display: table; }
      .greekFont {
        font-family: NewAthenaUnicode, WebNewAthenaUnicode,helvetica,arial;
      }
      #settingsdiv ul {
        list-style-type: none;
        margin: 0px;
        padding: 0px;
        text-align: left;
      }
      .settings {
        position: absolute;
        top: 70px;
        right: 11px;
        width: 230px;
        z-index: 999;
        padding: 10px;
      }
      #settingsdiv {
        background-color: white;
        color: black;
        border: 2px solid black;
        border-radius: 10px;
      }

      #settingsdiv {
        display:none;
      }
      #settingsdiv .settingsItem {
        background-color: white;
        color: black;
        display: block;
        padding: 6px 10px;
        cursor: pointer;
        border: 2px solid white;
      }
      #settingsdiv .settingsItem:hover {
        border: 2px solid black;
      }
      #settingsdiv div:focus {
        border: 2px solid black;
        outline: 0px solid black;
      }
      .settingsOn #settingsdiv {
        display:block;
      }
      .settingsOn #backdrop {
        display:block;
      }
      .settingsDarkMode {
        text-align:right;
        float:right;
        padding:0px;
        border:none;
      }
      #settingsDarkMode {
        height:70px;
      }
      #backdrop {
        position: absolute;
        top: 0px;
        left: 0px;
        width: 100%;
        height: 100%;
        z-index: 900;
        display:none;
      }
      .dark #settingsdiv {
        background-color: black;
        color: white;
        border: 2px solid white;
      }
      .dark #settingsdiv li {
        color: white;
      }
      .dark #settingsdiv .settingsItem {
        background-color: black;
        color: white;
        display: block;
        padding: 6px 10px;
        cursor: pointer;
        border: 2px solid black;
      }
      .dark #settingsdiv .settingsItem:hover {
        border: 2px solid white;
      }
      .dark #settingsdiv div:focus {
        border: 2px solid white;
        outline: 0px solid white;
      }
      .dark #menubar {
        color: white;
      }
      .dark #hamburger {
        background-color: black;
      }
      .dark #hamburger rect {
        fill: white;
      }
      .dark body {
        background-color: black;
        color: white;
      }
      .dark a {
        color: #03A5F3;
      }
      .dark #menubar {
        background-color: black;
        color: white;
        border-bottom: 1px solid white;
      }
      * {
        box-sizing: border-box;
      }
    </style>
    <script nonce="%NONCE%" type="text/javascript">
        'use strict';
        function q (i) { return document.querySelector(i); }
        function setTheme () {
            const mode = localStorage.getItem('mode');
            if ((window.matchMedia('(prefers-color-scheme: dark)').matches || mode === 'dark') && mode !== 'light') {
            q('HTML').classList.add('dark');
            } else {
            q('HTML').classList.remove('dark');
            }
        }
        setTheme();
    </script>
    </head>
    <body>
    <div id="menubar"><a id="newSynopsisLink" href="greek-synopsis">New Synopsis</a>
    <div id="loginContainer"><a id="loginlink" href="login">login</a>
        <span id="logoutlink">
          <span id="username"></span>
          (<a href="logout">logout</a>)
        </span>
    </div>
    <div id="appTitle">SYNOPSIS</div>

    <div id="hamburgercontainer">
      <svg id="hamburger" viewBox="0 0 120 120">
          <rect x="10" y="30" width="100" height="12"></rect>
          <rect x="10" y="56" width="100" height="12"></rect>
          <rect x="10" y="82" width="100" height="12"></rect>
      </svg>
  </div>

</div>
    <table id='table1' class='synlist'>
    <tr><td class='headerrow'>Date</td>"#,
    );

    if username.is_none() {
        res.push_str(r#"<td class='headerrow'>User</td>"#);
    }

    res.push_str(
        r#"<td class='headerrow'>Verb</td></tr>
    </table>
    <div id="settingsdiv" class="settings">
  Settings<br>
  <ul>
    <!-- <li>
      <div id='aboutButton' tabindex='0' class='menulink settingsItem'>about</div>
    </li>
    <li>
      <div id='iOSButton' tabindex='0' class='menulink settingsItem'>iOS/Android app</div>
    </li> -->
    <!--<li><div id='configureButton' tabindex='0' class='menulink settingsItem'>configure</div></li>-->
    <!--<li>
      <div id='toggleHistoryButton' tabindex='0' class='menulink settingsItem'>show/hide history</div>
    </li>-->
    <li>
      <div id="settingsDarkMode" tabindex='0' class="settingsItem">dark mode:
        <div class='settingsDarkMode'>
          <label for='darkModeSystem'>system</label>
          <input id='darkModeSystem' type='radio' name='darkmode' value='system'/><br>
          <label for='darkModeDark'>dark</label>
          <input id='darkModeDark' type='radio' name='darkmode' value='dark'/><br>
          <label for='darkModeLight'>light</label><input id='darkModeLight' type='radio' name='darkmode' value='light'/>
        </div>
      </div>
    </li>
  </ul>
</div>
    <div id="mesg"></div>
    <div id="backdrop"></div>
    <script nonce="2726c7f26c">
    let username = %USERNAME%;
    const rows = ["#,
    );

    let name = if username.is_some() {
        format!("'{}'", username.unwrap())
    } else {
        String::from("false")
    };
    res = res.replace("%USERNAME%", name.as_str());

    for l in list {
        let verb = &state.verbs[l.4.parse::<usize>().unwrap()].pps[0];
        res.push_str(
            format!(
                "['{}','{}','{}','{}'],",
                l.0,
                l.1,
                verb,
                l.2.unwrap_or(String::from(""))
            )
            .as_str(),
        );
    }

    res.push_str(r#"
        ];
        function q (i) { return document.querySelector(i); }
        function formatDate(date) {
            return new Date(date + 'Z').toLocaleString('en-CA');
        }

        const dFrag = document.createDocumentFragment();
        let count = 0;
        for (let r = 0; r < rows.length; r++) {
            const tr = document.createElement('tr');

            const td = document.createElement('td');
            //td.classList.add('moodrows');
            td.innerHTML = "<a href='greek-synopsis?id=" + rows[r][0] + "'>" + formatDate(rows[r][1]) + "</a>";
            tr.append(td);

            if (!username) {
                const td2 = document.createElement('td');
                td2.innerText = rows[r][3];
                tr.append(td2);
            }

            const td3 = document.createElement('td');
            td3.classList.add('greekFont')
            td3.innerText = rows[r][2];
            tr.append(td3);

            dFrag.appendChild(tr);
        }
        document.getElementById('table1').appendChild(dFrag);

        if (username) {
            const userName = username ? username : 'anon';
            document.body.classList.add('loggedin');
            q('#username').innerText = userName;
            //globalUserName = userName;
          } else {
            document.body.classList.remove('loggedin');
            q('#username').innerText = '';
            //globalUserName = null;
          }

          function toggleSettingsOn () {
            document.body.classList.add('settingsOn');
          }
          function toggleSettingsOff () {
            document.body.classList.remove('settingsOn');
          }

          function toggleSettings () {
            if (document.body.classList.contains('settingsOn')) {
              toggleSettingsOff();
            } else {
              const mode = window.localStorage.getItem('mode');
              switch (mode) {
                case 'dark':
                  document.querySelector('#darkModeDark').checked = true;
                  break;
                case 'light':
                  document.querySelector('#darkModeLight').checked = true;
                  break;
                default:
                  document.querySelector('#darkModeSystem').checked = true;
                  break;
              }
              toggleSettingsOn();
            }
          }

          function darkModeClick () {
            switch (this.id) {
              case 'darkModeDark':
                window.localStorage.setItem('mode', 'dark');
                break;
              case 'darkModeLight':
                window.localStorage.setItem('mode', 'light');
                break;
              default:
                window.localStorage.removeItem('mode');
                break;
            }
            // eslint-disable-next-line no-undef
            setTheme();
          }

    document.getElementById('hamburger').addEventListener('click', toggleSettings, false);
    document.getElementById('backdrop').addEventListener('click', toggleSettings, false);
    document.getElementById('darkModeSystem').addEventListener('click', darkModeClick, false);
    document.getElementById('darkModeDark').addEventListener('click', darkModeClick, false);
    document.getElementById('darkModeLight').addEventListener('click', darkModeClick, false);
    </script></body></html>"#);

    Html(res)
}

async fn greek_synopsis_saver(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Json(payload): extract::Json<SynopsisSaverRequest>,
) -> Result<Json<SynopsisJsonResult>, StatusCode> {
    let user_id = login::get_user_id(&session).await;

    let res = synopsis::save_synopsis(payload, user_id, &state.verbs, &state.hcdb)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(res))
}

async fn synopsis_json(
    _session: Session,
    State(state): State<AxumAppState>,
    extract::Json(payload): extract::Json<SynopsisSaverRequest>,
) -> Result<Json<SynopsisJsonResult>, StatusCode> {
    let res = synopsis::get_synopsis(payload, &state.verbs);

    Ok(Json(res))
}

async fn get_move(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<GetMoveQuery>,
) -> Result<Json<SessionState>, StatusCode> {
    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(&session).await {
        let res = libhc::hc_get_move(
            &state.hcdb,
            user_id,
            false,
            payload.session_id,
            &state.verbs,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn get_game_moves(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<GetMovesQuery>,
) -> Result<Json<GetMovesResponse>, StatusCode> {
    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(_user_id) = login::get_user_id(&session).await {
        let res = GetMovesResponse {
            response_to: String::from("getgamemoves"),
            session_id: payload.session_id,
            moves: libhc::hc_get_game_moves(&state.hcdb, &payload)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            success: true,
        };
        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn enter(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<AnswerQuery>,
) -> Result<Json<SessionState>, StatusCode> {
    tracing::info!("enter");
    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(&session).await {
        let res = libhc::hc_answer(&state.hcdb, user_id, &payload, timestamp, &state.verbs)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn ask(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<AskQuery>,
) -> Result<Json<SessionState>, StatusCode> {
    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(&session).await {
        let res = libhc::hc_ask(&state.hcdb, user_id, &payload, timestamp, &state.verbs)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn mf(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<AnswerQuery>,
) -> Result<Json<SessionState>, StatusCode> {
    let timestamp = libhc::get_timestamp();

    if let Some(user_id) = login::get_user_id(&session).await {
        let res = libhc::hc_mf_pressed(&state.hcdb, user_id, &payload, timestamp, &state.verbs)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(res))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn health_check() -> Response {
    //remember that basic authentication blocks this
    StatusCode::OK.into_response() //send 200 with empty body
}
