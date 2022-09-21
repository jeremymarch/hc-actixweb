/*
gkvocabdb

Copyright (C) 2021  Jeremy March

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

use super::*;

use secrecy::ExposeSecret;
use secrecy::Secret;
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[allow(clippy::eval_order_dependence)]
pub async fn login_get() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        //.insert_header(("X-Hdr", "sample"))
        .body(r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <title>Login</title>
        <script>
            function setTheme() {
                var mode = localStorage.getItem("mode");
                if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode == "dark") && mode != "light") {
                    document.querySelector("HTML").classList.add("dark");
                }
                else {
                    document.querySelector("HTML").classList.remove("dark");
                }
            }
            setTheme();
        </script>
        <style>
            BODY { font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; }
            TABLE { border:2px solid black;padding: 24px;border-radius: 10px; }
            BUTTON { padding: 3px 16px; }
            .dark BODY { background-color:black;color:white; }
            .dark INPUT { background-color:black;color:white;border: 2px solid white;border-radius: 6px; }
            .dark TABLE { border:2px solid white; }
            .dark BUTTON { background-color:black;color:white;border:1px solid white; }
        </style>
    </head>
    <body>
        <form action="/login" method="post">
            <table>
                <tbody>
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
                        <td colspan="2" align="center">
                            <a href="#" onclick="">Create User</a>
                        </td>
                    </tr>
                </tbody>
            </table>
        </form>
        <form action="/newuser" method="post">
            <table>
                <tbody>
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
                        <td>
                            <label for="password2">Retype Password</label>
                        </td>
                        <td>
                            <input type="password" id="password2" name="password2">
                        </td>
                    </tr><tr>
                    <td>
                        <label for="email">Email</label>
                    </td>
                    <td>
                        <input type="text" id="email" name="email">
                    </td>
                </tr>
                    <tr>
                        <td colspan="2" align="center">
                            <button type="submit">Create User</button>
                        </td>
                    </tr>
                </tbody>
            </table>
        </form>
        <script>/*document.getElementById("username").focus();*/</script>
    </body>
</html>"##))
}

//for testing without db
// fn validate_login(credentials: Credentials) -> Option<uuid::Uuid> {
//     if credentials.username.to_lowercase() == "user1"
//         && credentials.password.expose_secret() == "1234"
//     {
//         Some(Uuid::from_u128(0x8CD36EFFDF5744FF953B29A473D12347))
//     } else if credentials.username.to_lowercase() == "user2"
//         && credentials.password.expose_secret() == "1234"
//     {
//         Some(Uuid::from_u128(0xD75B0169E7C343838298136E3D63375C))
//     } else {
//         None
//     }
// }

#[allow(clippy::eval_order_dependence)]
pub async fn login_post(
    (session, form, req): (Session, web::Form<FormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    if let Ok(user_id) = db::validate_login_db(db, &credentials.username, credentials.password.expose_secret()).await.map_err(map_sqlx_error) {
        session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
        if session.insert("user_id", user_id).is_ok() {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish());
        }
    }

    session.purge();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}

pub fn get_user_id(session: Session) -> Option<uuid::Uuid> {
    if let Ok(s) = session.get::<uuid::Uuid>("user_id") {
        s
    }
    else {
        None
    }
}

/*
async fn check_login((session, _req): (Session, HttpRequest)) -> Result<HttpResponse, AWError> {
    //session.insert("user_id", 1);
    //session.renew();
    //session.purge();
    if let Some(user_id) = get_user_id(session) {
        return Ok(HttpResponse::Ok().json(LoginCheckResponse { is_logged_in:true,user_id:user_id }));
    }
    Ok(HttpResponse::Ok().json(LoginCheckResponse { is_logged_in:false,user_id:0 }))
}
*/

/* For Basic Authentication
use actix_web_httpauth::middleware::HttpAuthentication;
use actix_web_httpauth::extractors::basic::BasicAuth;
use actix_web_httpauth::extractors::basic::Config;
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web::dev::ServiceRequest;
use std::pin::Pin;

async fn validator_basic(req: ServiceRequest, credentials: BasicAuth) -> Result<ServiceRequest, Error> {

    let config = req.app_data::<Config>()
    .map(|data| Pin::new(data).get_ref().clone())
    .unwrap_or_else(Default::default);

    match validate_credentials_basic(credentials.user_id(), credentials.password().unwrap().trim()) {
        Ok(res) => {
            if res {
                Ok(req)
            } else {
                Err(AuthenticationError::from(config).into())
            }
        }
        Err(_) => Err(AuthenticationError::from(config).into()),
    }
}

fn validate_credentials_basic(user_id: &str, user_password: &str) -> Result<bool, std::io::Error> {
    if user_id.eq("greekdb") && user_password.eq("pass") {
        return Ok(true);
    }
    Err(std::io::Error::new(std::io::ErrorKind::Other, "Authentication failed!"))
}
*/
