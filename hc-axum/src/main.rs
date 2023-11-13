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

use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;
use rand::Rng;

use axum::{
    response::{ Html, IntoResponse},
    Server,
};
use http::{
    header::{HeaderMap, HeaderName, HeaderValue},
};
use axum::{response::Json};
use serde_json::{json, Value};
use axum::extract;
use axum::routing::post;
use libhc::GetSessions;

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
    let app = axum::Router::new()
        .route("/index.html", axum::routing::get(index))
        .route("/list", post(get_sessions))
        .nest_service("/", ServeDir::new("../static"))
        .nest_service("/dist", ServeDir::new("dist"))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive()) // Enable CORS policy
                .layer(layer),
        );
    let server = Server::bind(&"0.0.0.0:3000"
        .parse()
        .unwrap())
        .serve(app.into_make_service());

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }
    Ok(())
}

static INDEX_PAGE: &str = include_str!("../../hc-actix/src/index.html");

async fn index() -> impl IntoResponse {
    let mut rng = rand::thread_rng();
    let csp_nonce: String = rng.gen::<u32>().to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_str(&csp_nonce).unwrap(),
    );

    let page = INDEX_PAGE.replace("%NONCE%", &csp_nonce);
    (headers, Html(page))
}

async fn get_sessions(extract::Form(payload): extract::Form<GetSessions>) -> Json<Value> {
    Json(json!({ "msg": "Hello, world!" }))
}
