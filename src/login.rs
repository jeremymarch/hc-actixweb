/*
hc-actixweb

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
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use actix_web_flash_messages::FlashMessage;
use std::fmt::Write;

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

#[derive(serde::Deserialize)]
pub struct CreateUserFormData {
    username: String,
    password: String,
    confirm_password: String,
    email: String,
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

pub async fn logout(session: Session) -> Result<HttpResponse, AWError> {
    session.purge();
    //FlashMessage::error("Authentication error".to_string()).send();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}

pub async fn login_get(flash_messages: IncomingFlashMessages) -> Result<HttpResponse, AWError> {

    let mut error_html = String::from("");
    for m in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap(); 
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        //.insert_header(("X-Hdr", "sample"))
        .body(format!(r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <title>Login</title>
        <script>
            function setTheme() {{
                var mode = localStorage.getItem("mode");
                if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode == "dark") && mode != "light") {{
                    document.querySelector("HTML").classList.add("dark");
                }}
                else {{
                    document.querySelector("HTML").classList.remove("dark");
                }}
            }}
            setTheme();
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
        </script>
        <style>
            BODY {{ font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; flex-direction: column; }}
            TABLE {{ border:2px solid black;padding: 24px;border-radius: 10px; }}
            BUTTON {{ padding: 3px 16px; }}
            .dark BODY {{ background-color:black;color:white; }}
            .dark INPUT {{ background-color:black;color:white;border: 2px solid white;border-radius: 6px; }}
            .dark TABLE {{ border:2px solid white; }}
            .dark BUTTON {{ background-color:black;color:white;border:1px solid white; }}
            .dark a {{color:#03A5F3;}}
        </style>
    </head>
    <body>
        <form action="/login" method="post" onsubmit="return validate()">
            <table>
                <tbody>
                    <tr><td colspan="2" align="center">{}</td></tr>
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
                        <td colspan="2" align="right" style="padding-top:12px;">
                            <a href="newuser">New User</a>
                        </td>
                    </tr>
                </tbody>
            </table>
        </form>
        <script>/*document.getElementById("username").focus();*/</script>
    </body>
</html>
"##, error_html)))
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

pub async fn login_post(
    (session, form, req): (Session, web::Form<LoginFormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();

    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    if let Ok(user_id) = db.validate_login_db(&credentials.username, credentials.password.expose_secret()).await.map_err(map_sqlx_error) {
        session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
        if session.insert("user_id", user_id).is_ok() && session.insert("username", credentials.username).is_ok() {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish());
        }
    }

    session.purge();
    FlashMessage::error("Authentication error".to_string()).send();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}

pub async fn new_user_get(flash_messages: IncomingFlashMessages) -> Result<HttpResponse, AWError> {

    let mut error_html = String::from("");
    for m in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap(); 
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        //.insert_header(("X-Hdr", "sample"))
        .body(format!(r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <title>Login</title>
        <script>
            function setTheme() {{
                var mode = localStorage.getItem("mode");
                if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode == "dark") && mode != "light") {{
                    document.querySelector("HTML").classList.add("dark");
                }}
                else {{
                    document.querySelector("HTML").classList.remove("dark");
                }}
            }}
            setTheme();
            function validate() {{
                let u = document.forms[0]["username"].value;
                let p = document.forms[0]["password"].value;
                let c = document.forms[0]["confirm_password"].value;
                let e = document.forms[0]["email"].value;
                if (u.length < 2) {{
                  alert("Please enter a username");
                  return false;
                }}
                if (p.length < 8) {{
                    alert("Please enter a password of at least 8 characters");
                    return false;
                }}
                if (p != c) {{
                    alert("Password fields do not match");
                    return false;
                }}
                if (e.length < 6) {{
                    alert("Please enter an email");
                    return false;
                }}
              }}
        </script>
        <style>
            BODY {{ font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; flex-direction: column; }}
            TABLE {{ border:2px solid black;padding: 24px;border-radius: 10px; }}
            BUTTON {{ padding: 3px 16px; }}
            .dark BODY {{ background-color:black;color:white; }}
            .dark INPUT {{ background-color:black;color:white;border: 2px solid white;border-radius: 6px; }}
            .dark TABLE {{ border:2px solid white; }}
            .dark BUTTON {{ background-color:black;color:white;border:1px solid white; }}
            .dark a {{color:#03A5F3;}}
        </style>
    </head>
    <body>
        <form action="/newuser" method="post" onsubmit="return validate()">
            <table>
                <tbody>
                <tr><td colspan="2" align="center">{}</td></tr>
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
                            <label for="confirm_password">Confirm Password</label>
                        </td>
                        <td>
                            <input type="password" id="confirm_password" name="confirm_password">
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
</html>
"##, error_html)))
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

pub async fn new_user_post(
    (/*session, */form, req): (/*Session,*/ web::Form<CreateUserFormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();

    let username = form.0.username;
    let password = form.0.password;
    let confirm_password = form.0.confirm_password;
    let email = form.0.email;

    let timestamp = get_timestamp();

    if username.len() > 1 && password.len() > 3 && email.len() > 6 && password == confirm_password {
        if let Ok(_user_id) = db.create_user(&username, &password, &email, timestamp).await.map_err(map_sqlx_error) {
            //session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
            //if session.insert("user_id", user_id).is_ok() {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish());
            //}
        }
    }

    //session.purge();
    FlashMessage::error("Create user error".to_string()).send();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/newuser"))
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
pub fn get_username(session: Session) -> Option<String> {
    if let Ok(s) = session.get::<String>("username") {
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
