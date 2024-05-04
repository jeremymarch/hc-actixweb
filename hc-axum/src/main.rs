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
use libhc::hgk_compare_multiple_forms;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use axum::response::Response;
use serde::{Deserialize, Serialize};
use socketioxide::socket::DisconnectReason;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use std::sync::atomic::AtomicUsize;
use tower::ServiceBuilder;
use tower_cookies::cookie::SameSite;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::Level;
use tracing::{error, info};
//use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use axum::response::Json;
use axum::response::{Html, IntoResponse};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use axum::debug_handler;
use axum::error_handling::HandleErrorLayer;
use axum::extract;
use axum::extract::State;
use http::StatusCode;
use sqlx::postgres::PgPoolOptions;
use time::Duration;
use tower::BoxError;
use tower_cookies::CookieManagerLayer;
use tower_sessions::{
    ExpiredDeletion, Expiry, MemoryStore, PostgresStore, Session, SessionManagerLayer,
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

use libhc::synopsis::get_forms;
use libhc::synopsis::SaverResults;
use libhc::synopsis::SynopsisJsonResult;
use libhc::synopsis::SynopsisSaverRequest;

use uuid::Uuid;

mod login;

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

    let (layer, io) = SocketIo::new_layer();
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

    /*
        let session_store = PostgresStore::new(hcdb.db.clone());
        session_store.migrate().await?;

        let deletion_task = tokio::task::spawn(
            session_store
                .clone()
                .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
        );
    */
    let session_store = MemoryStore::default();

    let cookie_secure = !cfg!(debug_assertions); //cookie is secure for release, not secure for debug builds
    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(
            SessionManagerLayer::new(session_store)
                .with_secure(cookie_secure)
                .with_expiry(Expiry::OnInactivity(Duration::days(365)))
                .with_name("hcax")
                .with_http_only(true)
                .with_same_site(SameSite::Strict), //None, Strict, Lax //oauth needs None, but Chrome needs at least Lax for normal login to work
        );

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
        .route(
            "/greek-synopsis-result",
            axum::routing::get(greek_synopsis_result),
        )
        .route(
            "/greek-synopsis-list",
            axum::routing::get(greek_synopsis_list),
        )
        .route("/greek-synopsis", axum::routing::get(greek_synopsis))
        .route(
            "/greek-synopsis-saver",
            axum::routing::post(greek_synopsis_saver),
        )
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
        .layer(session_service)
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

async fn index() -> impl IntoResponse {
    // let mut rng = rand::thread_rng();
    // let csp_nonce: String = rng.gen::<u32>().to_string();
    let csp_nonce: String = Uuid::new_v4().to_string(); //.simple().encode_upper(&mut Uuid::encode_buffer()).to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
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
    if let Some(user_id) = login::get_user_id(&session) {
        //uuid!("96b875e7-fc53-4498-ad8d-9ce417e938b7");
        let username = login::get_username(&session);

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
    if let Some(user_id) = login::get_user_id(&session) {
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

use chrono::FixedOffset;
use chrono::LocalResult;
use chrono::TimeZone;

async fn greek_synopsis_result() -> impl IntoResponse {
    let csp_nonce: String = Uuid::new_v4().to_string(); //.simple().encode_upper(&mut Uuid::encode_buffer()).to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_str(&CSP.replace("%NONCE%", &csp_nonce)).unwrap(),
    );

    let page = SYNOPSIS_PAGE
        .replace("%NONCE%", &csp_nonce)
        .replace("%ISRESULTCSS%", "synopsis-result")
        .replace("%ISRESULT%", "true");
    (headers, Html(page))
}

async fn greek_synopsis() -> impl IntoResponse {
    let csp_nonce: String = Uuid::new_v4().to_string(); //.simple().encode_upper(&mut Uuid::encode_buffer()).to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_str(&CSP.replace("%NONCE%", &csp_nonce)).unwrap(),
    );

    let page = SYNOPSIS_PAGE
        .replace("%NONCE%", &csp_nonce)
        .replace("%ISRESULTCSS%", "synopsis-form")
        .replace("%ISRESULT%", "false");

    (headers, Html(page))
}

async fn greek_synopsis_list(
    session: Session,
    State(state): State<AxumAppState>,
) -> impl IntoResponse {
    let mut tx = state.hcdb.begin_tx().await.unwrap();
    let list = tx.greek_get_synopsis_list().await.unwrap();
    tx.commit_tx().await.unwrap();

    let mut res = String::from(
        r#"<!DOCTYPE html>
    <html>
    <head>
    <meta charset="UTF-8">
    <style nonce="2726c7f26c">
        .synlist { width: 600px;
            margin: 0px auto;
            border-collapse: collapse;
            font-size: 16pt;
            font-family:helvetica,arial;
        }
        .synlist td { padding: 3px; }
        .headerrow {border-bottom:1px solid black;font-weight:bold;}
    </style>

    </head>
    <body><table id='table1' class='synlist'>
    <tr><td class='headerrow'>Date</td><td class='headerrow'>Verb</td></tr></table>
    <script nonce="2726c7f26c">
    const rows = ["#,
    );

    for l in list {
        res.push_str(format!("['{}','{}','{}'],", l.0, l.1, l.4).as_str());
    }

    res.push_str(r#"
        ];

        function formatDate(date) {
            return new Date(date + 'Z').toLocaleString('en-CA');
        }

        const dFrag = document.createDocumentFragment();
        let count = 0;
        for (let r = 0; r < rows.length; r++) {
            const tr = document.createElement('tr');

            const td = document.createElement('td');
            //td.classList.add('moodrows');
            td.innerHTML = "<a href='greek-synopsis-result?id=" + rows[r][0] + "'>" + formatDate(rows[r][1]) + "</a>";
            tr.append(td);

            const td2 = document.createElement('td');
            td2.innerText = rows[r][2];
            tr.append(td2);

            dFrag.appendChild(tr);
        }
        document.getElementById('table1').appendChild(dFrag);
      
    </script></body></html>"#);

    Html(res)
}

async fn greek_synopsis_saver(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Json(payload): extract::Json<SynopsisSaverRequest>,
) -> Result<Json<SynopsisJsonResult>, StatusCode> {
    //let db2 = req.app_data::<SqliteUpdatePool>().unwrap();
    //let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //let user_agent = get_user_agent(&req).unwrap_or("");
    //https://stackoverflow.com/questions/66989780/how-to-retrieve-the-ip-address-of-the-client-from-httprequest-in-actix-web
    // let ip = if req.peer_addr().is_some() {
    //     req.peer_addr().unwrap().ip().to_string()
    // } else {
    //     "".to_string()
    // };

    let verb_id = payload.verb.try_into().unwrap();
    let correct_answers = get_forms(
        &state.verbs,
        verb_id,
        payload.person,
        payload.number,
        payload.ptccase,
        payload.ptcgender,
    );
    let mut is_correct = Vec::new();
    // let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));
    for (i, f) in payload.r.iter().enumerate() {
        if let Some(a) = &correct_answers[i] {
            is_correct.push(hgk_compare_multiple_forms(a, &f.replace("---", "—"), true));
        } else {
            is_correct.push(true);
        }
    }

    let mut res_forms = Vec::<SaverResults>::new();
    for (n, i) in correct_answers.into_iter().enumerate() {
        res_forms.push(SaverResults {
            given: payload.r[n].clone(),
            correct: i,
            is_correct: is_correct[n],
        });
    }

    let res = SynopsisJsonResult {
        verb_id: payload.verb,
        person: payload.person,
        number: payload.number,
        case: payload.ptccase,
        gender: payload.ptcgender,
        unit: payload.unit,
        pp: payload.pp.clone(),
        // pp: verbs[verb_id]
        //     .pps
        //     .iter()
        //     .map(|x| x.replace('/', " or ").replace("  ", " "))
        //     .collect::<Vec<_>>()
        //     .join(", "),
        name: payload.sname.clone(),
        advisor: payload.advisor.clone(),
        f: res_forms,
    };

    let mut tx = state
        .hcdb
        .begin_tx()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = tx
        .greek_insert_synopsis(
            None,
            &payload,
            //ip.as_str(),
            //user_agent,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit_tx()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    //Ok(HttpResponse::Ok().finish())
    //let res = 1;
    //Ok(HttpResponse::Ok().json(res))
    Ok(Json(res))
}

async fn synopsis_json(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Json(payload): extract::Json<SynopsisSaverRequest>,
) -> Result<Json<SynopsisJsonResult>, StatusCode> {
    // let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));
    //let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();
    // let pp = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
    // let verb = Arc::new(HcGreekVerb::from_string(1, pp, REGULAR, 0).unwrap());
    let verb_id: usize = payload.verb.try_into().unwrap();

    let forms = get_forms(
        &state.verbs,
        verb_id,
        payload.person,
        payload.number,
        payload.ptccase,
        payload.ptcgender,
    );

    let mut res = Vec::<SaverResults>::new();
    for f in forms {
        res.push(SaverResults {
            given: f.unwrap_or("".to_string()),
            correct: None,
            is_correct: true,
        });
    }

    let res = SynopsisJsonResult {
        verb_id: payload.verb,
        person: payload.person,
        number: payload.number,
        case: payload.ptccase,
        gender: payload.ptcgender,
        unit: payload.unit,
        pp: state.verbs[verb_id]
            .pps
            .iter()
            .map(|x| x.replace('/', " or ").replace("  ", " "))
            .collect::<Vec<_>>()
            .join(", "),
        name: "".to_string(),
        advisor: "".to_string(),
        f: res,
    };

    //Ok(HttpResponse::Ok().json(res))
    Ok(Json(res))
}

async fn get_move(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(payload): extract::Form<GetMoveQuery>,
) -> Result<Json<SessionState>, StatusCode> {
    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(&session) {
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

    if let Some(user_id) = login::get_user_id(&session) {
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

    if let Some(user_id) = login::get_user_id(&session) {
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

    if let Some(user_id) = login::get_user_id(&session) {
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

    if let Some(user_id) = login::get_user_id(&session) {
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
