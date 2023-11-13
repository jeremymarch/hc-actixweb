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
use libhc::HcError::Database;
use secrecy::Secret;
//use serde_json::Value;
//use std::collections::HashMap;
use std::fmt::Write;

use sign_in_with_apple::AppleClaims;
use sign_in_with_apple::GoogleClaims;
use sign_in_with_apple::Issuer;

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

/*
struct OAuthPost {
    code: Option<String>,
    state: Option<String>,
}
//https://stackoverflow.com/questions/42216700/how-can-i-redirect-after-oauth2-with-samesite-strict-and-still-get-my-cookies
//https://www.scottbrady91.com/openid-connect/implementing-sign-in-with-apple-in-aspnet-core
//https://www.oauth.com/oauth2-servers/signing-in-with-google/
//https://developer.okta.com/blog/2019/06/04/what-the-heck-is-sign-in-with-apple
//https://www.scottbrady91.com/openid-connect/implementing-sign-in-with-apple-in-aspnet-core
pub async fn oauth_post(
    (session, form, req): (Session, web::Form<OAuthPost>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let client_id = "us.philolog.hoplite-challenge.client";
    let client_secret = "eyJraWQiOiJZREtHQVk3QVA0IiwiYWxnIjoiRVMyNTYifQ.eyJpc3MiOiJFRkhDNFhGWjM4IiwiaWF0IjoxNjk3Njg1MjAxLCJleHAiOjE3MTMyMzcyMDEsImF1ZCI6Imh0dHBzOi8vYXBwbGVpZC5hcHBsZS5jb20iLCJzdWIiOiJ1cy5waGlsb2xvZy5ob3BsaXRlLWNoYWxsZW5nZS5jbGllbnQifQ.Xy-sYbch2xDnQFNRkr8vLDwu__FX2__pkdw_fWuZSasySceG2CERSZUcJ83bf4NVc3kvvfrl2LoQMfsgnZNEMw";
    let redirect_url = "https://example-app.com/redirect";

    if form.code.is_some() {

        if let Ok(state) = session.get::<String>("state") {
            if form.state.is_some() && form.state.unwrap() !=  state {
                //error
            }
        } else {
            None
        }

        // Token endpoint docs:
  // https://developer.apple.com/documentation/signinwithapplerestapi/generate_and_validate_tokens

  $response = http('https://appleid.apple.com/auth/token', [
    'grant_type' => 'authorization_code',
    'code' => $_POST['code'],
    'redirect_uri' => $redirect_uri,
    'client_id' => $client_id,
    'client_secret' => $client_secret,
  ]);

  if(!isset($response->access_token)) {
    echo '<p>Error getting an access token:</p>';
    echo '<pre>'; print_r($response); echo '</pre>';
    echo '<p><a href="/">Start Over</a></p>';
    die();
  }

  echo '<h3>Access Token Response</h3>';
  echo '<pre>'; print_r($response); echo '</pre>';


  $claims = explode('.', $response->id_token)[1];
  $claims = json_decode(base64_decode($claims));

  echo '<h3>Parsed ID Token</h3>';
  echo '<pre>';
  print_r($claims);
  echo '</pre>';

  die();

    }
}
*/

use actix_web::http::header;
use libhc::hc_create_oauth_user;
use oauth2::basic::BasicClient;
use oauth2::AuthUrl;
use oauth2::ClientId;
use oauth2::ClientSecret;
use oauth2::RedirectUrl;
use oauth2::ResponseType;
use oauth2::TokenUrl;
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge, Scope};
use serde::Deserialize;
use serde::Serialize;
use std::env;

#[derive(Deserialize)]
pub struct AuthRequest {
    code: Option<String>,
    state: String,
    id_token: Option<String>,
    user: Option<String>,
}

pub struct AppState {
    pub apple_oauth: BasicClient,
    pub google_oauth: BasicClient,
}
/*
#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppleClaims {
    iss: Option<String>,
    aud: Option<String>,
    exp: Option<u64>,
    iat: Option<u64>,
    sub: Option<String>,
    c_hash: Option<String>,
    auth_time: Option<u64>,
    nonce: Option<String>,
    nonce_supported: Option<bool>,
    email: Option<String>,
    //this is bool for Google and String for Apple
    //https://developer.apple.com/forums/thread/121411?answerId=378290022#378290022
    email_verified: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GoogleClaims {
    iss: Option<String>,
    aud: Option<String>,
    exp: Option<u64>,
    iat: Option<u64>,
    sub: Option<String>,
    c_hash: Option<String>,
    auth_time: Option<u64>,
    nonce: Option<String>,
    nonce_supported: Option<bool>,
    email: Option<String>,
    //this is bool for Google and String for Apple
    //https://developer.apple.com/forums/thread/121411?answerId=378290022#378290022
    email_verified: Option<bool>,
}
*/
#[derive(Debug, Serialize, Deserialize)]
struct AppleOAuthUserName {
    #[serde(rename(serialize = "firstName"), rename(deserialize = "firstName"))]
    first_name: Option<String>,
    #[serde(rename(serialize = "lastName"), rename(deserialize = "lastName"))]
    last_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppleOAuthUser {
    name: AppleOAuthUserName,
    email: Option<String>,
}

pub fn get_google_client() -> BasicClient {
    let client_id = ClientId::new(
        env::var("GOOGLE_CLIENT_ID").expect("Missing the GOOGLE_CLIENT_ID environment variable."),
    );
    let client_secret = ClientSecret::new(
        env::var("GOOGLE_CLIENT_SECRET")
            .expect("Missing the GOOGLE_CLIENT_SECRET environment variable."),
    );
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string())
        .expect("Invalid token endpoint URL");

    // Set up the config for the Google OAuth2 process.
    BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url)).set_redirect_uri(
        RedirectUrl::new("https://hoplite-challenge.philolog.us/gauth".to_string())
            .expect("Invalid redirect URL"),
    )
}

pub fn get_apple_client() -> BasicClient {
    let client_id = ClientId::new(
        env::var("APPLE_CLIENT_ID").expect("Missing the APPLE_CLIENT_ID environment variable."),
    );
    let client_secret = ClientSecret::new(
        env::var("APPLE_CLIENT_SECRET")
            .expect("Missing the APPLE_CLIENT_SECRET environment variable."),
    );
    let auth_url = AuthUrl::new("https://appleid.apple.com/auth/authorize".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://appleid.apple.com/auth/token".to_string())
        .expect("Invalid token endpoint URL");

    // Set up the config for the Apple OAuth2 process.
    BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url)).set_redirect_uri(
        RedirectUrl::new("https://hoplite-challenge.philolog.us/auth".to_string())
            .expect("Invalid redirect URL"),
    )
}

pub async fn oauth_login_apple(
    (session, req): (Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let data = req.app_data::<AppState>().unwrap();

    let (pkce_code_challenge, _pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce = uuid::Uuid::new_v4(); // use UUID as random and unique nonce

    let (authorize_url, csrf_state) = &data
        .apple_oauth
        .authorize_url(CsrfToken::new_random)
        .set_response_type(&ResponseType::new("code id_token".to_string()))
        .add_extra_param("response_mode".to_string(), "form_post".to_string())
        .add_extra_param("nonce".to_string(), nonce.to_string())
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("name".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_code_challenge) //apple does not support this, but no problem including it
        .url();

    let state = csrf_state.secret().to_string();
    session.renew();
    session
        .insert::<String>("oauth_state", state)
        .expect("session.insert state");

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, authorize_url.to_string()))
        .finish())
}

pub async fn oauth_login_google(
    (session, req): (Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let data = req.app_data::<AppState>().unwrap();
    // Google supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, _pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce = uuid::Uuid::new_v4(); // use UUID as random and unique nonce

    let (authorize_url, csrf_state) = &data
        .google_oauth
        .authorize_url(CsrfToken::new_random)
        .set_response_type(&ResponseType::new("code id_token".to_string()))
        .add_extra_param("response_mode".to_string(), "form_post".to_string())
        .add_extra_param("nonce".to_string(), nonce.to_string())
        .add_scope(Scope::new("openid".to_string()))
        // .add_scope(Scope::new("name".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_code_challenge) // apple does not support this
        .url();

    let state = csrf_state.secret().to_string();
    session.renew();
    session
        .insert::<String>("oauth_state", state)
        .expect("session.insert state");

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, authorize_url.to_string()))
        .finish())
}

pub async fn oauth_auth_apple(
    (session, params, req): (Session, web::Form<AuthRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let data = req.app_data::<AppState>().unwrap();

    let saved_state = session.get::<String>("oauth_state").unwrap();

    if let Some(param_code) = &params.code {
        let code = AuthorizationCode::new(param_code.clone());
        let received_state = CsrfToken::new(params.state.clone());
        let user = params.user.clone();
        let id_token = params.id_token.clone();

        let _token = &data.apple_oauth.exchange_code(code);

        if let Some(ref id_token_ref) = id_token {
            if saved_state.unwrap() == *received_state.secret() {
                
                let mut first_name = String::from("");
                let mut last_name = String::from("");
                let mut email = String::from("");

                if let Some(ref user) = user {
                    if let Ok(apple_oauth_user) = serde_json::from_str::<AppleOAuthUser>(user) {
                        first_name = apple_oauth_user.name.first_name.unwrap_or(String::from(""));
                        last_name = apple_oauth_user.name.last_name.unwrap_or(String::from(""));
                        email = apple_oauth_user.email.unwrap_or(String::from(""));
                    }
                }

                println!("apple test test3");
                if let Ok(result) = sign_in_with_apple::validate::<AppleClaims>(
                    &env::var("APPLE_CLIENT_ID")
                        .expect("Missing the APPLE_CLIENT_ID environment variable."),
                    id_token_ref,
                    false,
                    Issuer::APPLE,
                )
                .await
                {
                    let sub = result.claims.sub;
                    let iss = result.claims.iss;
                    email = result.claims.email.unwrap_or(String::from(""));

                    let timestamp = libhc::get_timestamp();
                    match hc_create_oauth_user(
                        db,
                        &iss,
                        &sub,
                        if email.is_empty() { None } else { Some(&email) },
                        &first_name,
                        &last_name,
                        &email,
                        timestamp,
                    )
                    .await
                    {
                        Ok((user_id, user_name)) => {
                            session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                            if session.insert("user_id", user_id).is_ok()
                                && session.insert("username", user_name).is_ok()
                            {
                                return Ok(HttpResponse::SeeOther()
                                    .insert_header((LOCATION, "/"))
                                    .finish());
                            }
                        }
                        Err(Database(e)) => {
                            FlashMessage::error(e.to_string()).send();
                            return nav_to_login(session);
                        }
                        _ => (),
                    }
                }

                return nav_to_login(session);
            }
        }
    }

    nav_to_login(session)
}

pub async fn oauth_auth_google(
    (session, params, req): (Session, web::Form<AuthRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDbPostgres>().unwrap();
    let data = req.app_data::<AppState>().unwrap();
    let saved_state = session.get::<String>("oauth_state").unwrap();

    if let Some(param_code) = &params.code {
        // println!("code code");
        let code = AuthorizationCode::new(param_code.clone());
        let received_state = CsrfToken::new(params.state.clone());
        //let user = params.user.clone(); //google doesn't send user this way
        let id_token = params.id_token.clone();

        // Exchange the code with a token.
        let _token = &data.google_oauth.exchange_code(code);

        if let Some(ref id_token_ref) = id_token {
            if saved_state.unwrap() == *received_state.secret() {
                let first_name = String::from("");
                let last_name = String::from("");
                let mut email = String::from("");

                println!("cccccc {:?}", id_token_ref);
                println!("google test test3");
                if let Ok(result) = sign_in_with_apple::validate::<GoogleClaims>(
                    &env::var("GOOGLE_CLIENT_ID")
                        .expect("Missing the GOOGLE_CLIENT_ID environment variable."),
                    id_token_ref,
                    false,
                    Issuer::GOOGLE,
                )
                .await
                {
                    let sub = result.claims.sub;
                    let iss = result.claims.iss;
                    email = result.claims.email.unwrap_or(String::from(""));

                    let timestamp = libhc::get_timestamp();

                    match hc_create_oauth_user(
                        db,
                        &iss,
                        &sub,
                        if email.is_empty() { None } else { Some(&email) },
                        &first_name,
                        &last_name,
                        &email,
                        timestamp,
                    )
                    .await
                    {
                        Ok((user_id, user_name)) => {
                            session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                            if session.insert("user_id", user_id).is_ok()
                                && session.insert("username", user_name).is_ok()
                            {
                                return Ok(HttpResponse::SeeOther()
                                    .insert_header((LOCATION, "/"))
                                    .finish());
                            }
                        }
                        Err(Database(e)) => {
                            FlashMessage::error(e.to_string()).send();
                            return nav_to_login(session);
                        }
                        _ => (),
                    }
                }

                return nav_to_login(session);
            }
        }
    }

    nav_to_login(session)
}

fn nav_to_login(session: Session) -> Result<HttpResponse, AWError> {
    session.purge();
    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/login".to_string()))
        .finish())
}
