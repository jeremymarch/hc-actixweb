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
use std::sync::atomic::AtomicUsize;
use uuid::uuid;

use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;
//use rand::Rng;

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
use axum::routing::post;
use http::StatusCode;
use sqlx::postgres::PgPoolOptions;
use time::Duration;
use tower::BoxError;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use libhc::dbpostgres::HcDbPostgres;
use libhc::GetSessions;
use libhc::SessionsListResponse;

use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(transparent)]
struct Username(String);

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

    let session_store = MemoryStore::default();
    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(
            SessionManagerLayer::new(session_store)
                .with_secure(false)
                .with_expiry(Expiry::OnInactivity(Duration::seconds(10)))
                .with_name("hcax"),
        );

    let app = axum::Router::new()
        .route("/index.html", axum::routing::get(index))
        .route("/list", post(get_sessions))
        .nest_service("/", ServeDir::new("static"))
        .nest_service("/dist", ServeDir::new("dist"))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive()) // Enable CORS policy
                .layer(layer),
        )
        .with_state(hcdb)
        .layer(session_service);

    let server = Server::bind(&"0.0.0.0:3000".parse().unwrap()).serve(app.into_make_service());

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
    State(db): State<HcDbPostgres>,
    extract::Form(payload): extract::Form<GetSessions>,
) -> Json<SessionsListResponse> {
    let verbs = libhc::hc_load_verbs("pp.txt");

    let user_id: Uuid = uuid!("96b875e7-fc53-4498-ad8d-9ce417e938b7");
    //let user_id = Uuid::new_v4();// { //login::get_user_id(session.clone()) {
    let username = Some(String::from("axumtest")); //login::get_username(session);

    session
        .insert("axumsession", user_id)
        .expect("Could not serialize.");

    let res = libhc::hc_get_sessions(&db, user_id, &verbs, username, &payload)
        .await
        .unwrap();

    // let res = SessionsListResponse {
    //     response_to: String::from("abc"),
    //     sessions: vec![],
    //     success: false,
    //     username: None,
    //     logged_in: false,
    //     current_session: None,
    // };
    Json(res)
}
