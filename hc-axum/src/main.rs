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
use axum::response::Response;
use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use std::sync::atomic::AtomicUsize;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

use axum::response::Json;
use axum::{
    response::{Html, IntoResponse},
    Server,
};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use axum::debug_handler;
use axum::error_handling::HandleErrorLayer;
use axum::extract;
use axum::extract::State;
use http::StatusCode;
use sqlx::postgres::PgPoolOptions;
use time::Duration;
use tower::BoxError;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use hoplite_verbs_rs::*;
use libhc::dbpostgres::HcDbPostgres;
use libhc::GetSessions;
use libhc::SessionsListResponse;
use std::sync::Arc;

use uuid::Uuid;

mod login;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(transparent)]
struct Username(String);

use axum::extract::FromRef;
#[derive(Clone)]
pub struct AppState {
    hcdb: HcDbPostgres,
    verbs: Vec<Arc<HcGreekVerb>>,
}
// impl FromRef<AppState> for HcDbPostgres {
//     fn from_ref(app_state: &AppState) -> HcDbPostgres {
//         app_state.hcdb.clone()
//     }
// }

// impl FromRef<AppState> for Vec<Arc<HcGreekVerb>> {
//     fn from_ref(app_state: &AppState) -> Vec<Arc<HcGreekVerb>> {
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
    let subscriber = FmtSubscriber::new();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting server");

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

        s.on_disconnect(move |s, _| async move {
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

    let session_store = MemoryStore::default();
    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(
            SessionManagerLayer::new(session_store)
                .with_secure(false)
                .with_expiry(Expiry::OnInactivity(Duration::days(365)))
                .with_name("hcax"),
        );

    let verbs = libhc::hc_load_verbs("pp.txt");

    let state = AppState { hcdb, verbs };

    let app = axum::Router::new()
        .route("/index.html", axum::routing::get(index))
        .route("/list", axum::routing::post(get_sessions))
        .route("/new", axum::routing::post(create_session))
        .route("/login", axum::routing::get(login::login_get))
        .route("/login", axum::routing::post(login::login_post))
        .route("/newuser", axum::routing::get(login::new_user_get))
        .route("/newuser", axum::routing::post(login::new_user_post))
        .route("/logout", axum::routing::get(login::logout))
        .route("/healthzzz", axum::routing::get(health_check))
        .nest_service("/", ServeDir::new("static"))
        .nest_service("/dist", ServeDir::new("dist"))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive()) // Enable CORS policy
                .layer(layer),
        )
        .with_state(state)
        //.with_state(verbs)
        .layer(session_service);

    let server = Server::bind(&"0.0.0.0:8088".parse().unwrap()).serve(app.into_make_service());

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }
    Ok(())
}

static INDEX_PAGE: &str = include_str!("../../hc-actix/src/index.html");

async fn index() -> impl IntoResponse {
    // let mut rng = rand::thread_rng();
    // let csp_nonce: String = rng.gen::<u32>().to_string();
    let csp_nonce: String = Uuid::new_v4().to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_str(&csp_nonce).unwrap(),
    );

    let page = INDEX_PAGE.replace("%NONCE%", &csp_nonce);
    (headers, Html(page))
}

#[debug_handler]
async fn get_sessions(
    session: Session,
    State(state): State<AppState>,
    extract::Form(payload): extract::Form<GetSessions>,
) -> Json<SessionsListResponse> {
    if let Ok(user_id) = session.get::<Uuid>("user_id") {
        //uuid!("96b875e7-fc53-4498-ad8d-9ce417e938b7");
        let username = session
            .get::<String>("username")
            .unwrap_or(Some("zzz".to_string()));

        if let Some(user_id) = user_id {
            let res =
                libhc::hc_get_sessions(&state.hcdb, user_id, &state.verbs, username, &payload)
                    .await
                    .unwrap();

            return Json(res);
        }
    }

    let res = SessionsListResponse {
        response_to: String::from(""),
        sessions: vec![],
        success: false,
        username: None,
        logged_in: false,
        current_session: None,
    };
    Json(res)
}
use libhc::CreateSessionQuery;
use libhc::HcError;
#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

async fn create_session(
    session: Session,
    State(state): State<AppState>,
    extract::Form(mut payload): extract::Form<CreateSessionQuery>,
) -> Json<StatusResponse> {
    if let Some(user_id) = session.get::<Uuid>("user_id").unwrap() {
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
        Json(res)
    } else {
        //not_logged_in_response()
        let res = StatusResponse {
            response_to: String::from("newsession"),
            mesg: "".to_string(),
            success: false,
        };
        Json(res)
    }
}

async fn health_check() -> Response {
    //remember that basic authentication blocks this
    String::from("").into_response() //send 200 with empty body
}
