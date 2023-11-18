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

use axum::response::Json;
use axum::{
    response::{Html, IntoResponse},
    Server,
};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use axum::response::Redirect;
use libhc::hc_validate_credentials;
use libhc::Credentials;

use axum::debug_handler;
use axum::error_handling::HandleErrorLayer;
use axum::extract;
use axum::extract::State;
use http::StatusCode;
use sqlx::postgres::PgPoolOptions;
use time::Duration;
use tower::BoxError;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use libhc::dbpostgres::HcDbPostgres;
use libhc::GetSessions;
use libhc::SessionsListResponse;

use secrecy::Secret;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

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
        .route("/list", axum::routing::post(get_sessions))
        .route("/login", axum::routing::get(login_get))
        .route("/login", axum::routing::post(login_post))
        .route("/logout", axum::routing::get(logout))
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

    if let Ok(user_id) = session.get::<Uuid>("user_id") {
        //uuid!("96b875e7-fc53-4498-ad8d-9ce417e938b7");
        let username = session
            .get::<String>("username")
            .unwrap_or(Some("zzz".to_string()));

        if let Some(user_id) = user_id {
            let res = libhc::hc_get_sessions(&db, user_id, &verbs, username, &payload)
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

#[debug_handler]
async fn login_get() -> impl IntoResponse {
    let error_html = "";

    let b = format!(
        r##"<!DOCTYPE html>
    <html lang="en">
        <head>
            <meta http-equiv="content-type" content="text/html; charset=utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1"/>
            <title>Login</title>
            <script nonce="2726c7f26c">
                function setTheme() {{
                    var mode = localStorage.getItem("mode");
                    if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode === "dark") && mode !== "light") {{
                        document.querySelector("HTML").classList.add("dark");
                    }}
                    else {{
                        document.querySelector("HTML").classList.remove("dark");
                    }}
                }}
                setTheme();
                function applelogin() {{
                    window.location.href = "oauth-login-apple";
                }}
                function googlelogin() {{
                    window.location.href = "oauth-login-google";
                }}
                function validate() {{
                    let u = document.forms[0]["username"].value;
                    let p = document.forms[0]["password"].value;
                    if (u == "") {{
                      alert("Please enter a username");
                      return false;
                    }}
                    if (p == "") {{
                        alert("Please enter a password");
                        return false;
                      }}
                  }}
                  function start() {{
                    document.getElementById("loginform").addEventListener('submit', validate, false);
                  }}
                  window.addEventListener('load', start, false);
            </script>
            <style nonce="2726c7f26c">
                BODY {{ font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; flex-direction: column; }}
                TABLE {{ border:2px solid black;padding: 24px;border-radius: 10px; }}
                BUTTON {{ padding: 3px 16px; }}
                .dark BODY {{ background-color:black;color:white; }}
                .dark INPUT {{ background-color:black;color:white;border: 2px solid white;border-radius: 6px; }}
                .dark TABLE {{ border:2px solid white; }}
                .dark BUTTON {{ background-color:black;color:white;border:1px solid white; }}
                .dark a {{color:#03A5F3;}}
                #newuserdiv {{ padding-top:12px;height:70px; }}
                #apple-login {{border: 1px solid white;width: 200px;margin: 0x auto;display: inline-block;margin-top: 12px; }}
                #google-login {{border: 1px solid white;width: 200px;margin: 0x auto;display: inline-block;margin-top: 12px; }}
                .oauthcell {{height:70px;}}
                .orcell {{height:20px;padding-top:20px;border-top:1px solid black;}}
                .dark .orcell {{border-top:1px solid white;}}
            </style>
        </head>
        <body>
            <form id="loginform" action="/login" method="post">
                <table>
                    <tbody>
                        <tr><td colspan="2" align="center">{error_html}</td></tr>
                        <tr>
                            <td>               
                                <label for="username">Username</label>
                            </td>
                            <td>
                                <input type="text" id="username" name="username">
                            </td>
                        </tr>
                        <tr>
                            <td>
                                <label for="password">Password</label>
                            </td>
                            <td>
                                <input type="password" id="password" name="password">
                            </td>
                        </tr>
                        <tr>
                            <td colspan="2" align="center">
                                <button type="submit">Login</button>
                            </td>
                        </tr>
                        <tr>
                            <td colspan="2" align="right" id="newuserdiv">
                                <a href="newuser">New User</a>
                            </td>
                        </tr>
                        <tr>
                            <td colspan="2" align="center" class="orcell">
                                or
                            </td>
                        </tr>
                        <tr>
                            <td colspan="2" align="center" class="oauthcell"><img id="apple-login" src="appleid_button@2x.png" onclick="applelogin()"/></td>
                        </tr>
                        <tr>
                            <td colspan="2" align="center" class="oauthcell"><img id="google-login" src="branding_guideline_sample_dk_sq_lg.svg" onclick="googlelogin()"/></td>
                        </tr>
                    </tbody>
                </table>
            </form>
        </body>
    </html>
    "##
    );

    Html(b)
}

async fn login_post(
    session: Session,
    State(db): State<HcDbPostgres>,
    extract::Form(form): extract::Form<LoginFormData>,
) -> impl IntoResponse {
    session.clear();
    //session.flush();
    let credentials = Credentials {
        username: form.username.clone(),
        password: form.password,
    };

    if let Ok(user_id) = hc_validate_credentials(&db, credentials).await
    //map_err(map_hc_error)
    //fix me, should handle error here in case db error, etc.
    {
        if session.insert("user_id", user_id).is_ok()
            && session.insert("username", form.username).is_ok()
        {
            return Redirect::to("/index.html");
        }
    }

    session.clear();
    Redirect::to("/login")
}

pub async fn logout(session: Session) -> impl IntoResponse {
    session.clear();
    session.flush();
    //FlashMessage::error(String::from("Authentication error")).send();
    Redirect::to("/login")
}
