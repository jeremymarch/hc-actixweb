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
use crate::map_hc_error;
use actix_session::Session;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::Error as AWError;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use libhc::dbpostgres::HcDbPostgres;
use libhc::Credentials;
use secrecy::Secret;
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

pub async fn logout(session: Session) -> Result<HttpResponse, AWError> {
    session.purge();
    //FlashMessage::error(String::from("Authentication error")).send();
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
            #newuserdiv {{ padding-top:12px; }}
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
                </tbody>
            </table>
        </form>
    </body>
</html>
"##)))
}

use libhc::hc_validate_credentials;
pub async fn login_post(
    (session, form, req): (Session, web::Form<LoginFormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();

    let credentials = Credentials {
        username: form.0.username.clone(),
        password: form.0.password,
    };

    if let Ok(user_id) = hc_validate_credentials(db, credentials)
        .await
        .map_err(map_hc_error)
    //fix me, should handle error here in case db error, etc.
    {
        session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
        if session.insert("user_id", user_id).is_ok()
            && session.insert("username", form.0.username).is_ok()
        {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish());
        }
    }

    session.purge();
    FlashMessage::error(String::from("Authentication error")).send();
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
              function start() {{
                document.getElementById("createuserform").addEventListener('submit', validate, false);
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
        </style>
    </head>
    <body>
        <form id="createuserform" action="/newuser" method="post">
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
    </body>
</html>
"##)))
}

pub async fn new_user_post(
    (/*session, */ form, req): (/*Session,*/ web::Form<CreateUserFormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();

    let username = form.0.username;
    let password = form.0.password;
    let confirm_password = form.0.confirm_password;
    let email = form.0.email;

    let timestamp = libhc::get_timestamp();

    if username.len() > 1 && password.len() > 3 && email.len() > 6 && password == confirm_password {
        match libhc::hc_create_user(db, &username, &password, &email, timestamp)
            .await
            .map_err(map_hc_error)
        {
            Ok(_user_id) => {
                //session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                //if session.insert("user_id", user_id).is_ok() {
                Ok(HttpResponse::SeeOther()
                    .insert_header((LOCATION, "/login"))
                    .finish())
                //}
            }
            Err(_e) => {
                //session.purge();
                FlashMessage::error(String::from("Create user error")).send();
                Ok(HttpResponse::SeeOther()
                    .insert_header((LOCATION, "/newuser"))
                    .finish())
            }
        }
    } else {
        //session.purge();
        FlashMessage::error(String::from("Create user error")).send();
        Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/newuser"))
            .finish())
    }
}

pub fn get_user_id(session: Session) -> Option<uuid::Uuid> {
    if let Ok(s) = session.get::<uuid::Uuid>("user_id") {
        s
    } else {
        None
    }
}
pub fn get_username(session: Session) -> Option<String> {
    if let Ok(s) = session.get::<String>("username") {
        s
    } else {
        None
    }
}
