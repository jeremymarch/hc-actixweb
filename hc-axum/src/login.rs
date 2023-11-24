use crate::AxumAppState;
use axum::debug_handler;
use axum::extract;
use axum::extract::State;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Redirect;
use libhc::hc_validate_credentials;
use libhc::Credentials;
use secrecy::Secret;
use tower_cookies::Cookie;
use tower_cookies::Cookies;
use tower_sessions::cookie::SameSite;
use tower_sessions::Session;

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

pub fn get_user_id(session: &Session) -> Option<uuid::Uuid> {
    if let Ok(s) = session.get::<uuid::Uuid>("user_id") {
        s
    } else {
        None
    }
}
pub fn get_username(session: &Session) -> Option<String> {
    if let Ok(s) = session.get::<String>("username") {
        s
    } else {
        None
    }
}

const OAUTH_COOKIE: &str = "oauth_state";
const OAUTH_COOKIE_NONCE: &str = "oauth_nonce";
// pub fn get_oauth_state(session: &Session) -> Option<String> {
//     if let Ok(s) = session.get::<String>(OAUTH_COOKIE) {
//         s
//     } else {
//         None
//     }
// }

#[debug_handler]
pub async fn login_get() -> impl IntoResponse {
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

pub async fn login_post(
    session: Session,
    State(state): State<AxumAppState>,
    extract::Form(form): extract::Form<LoginFormData>,
) -> impl IntoResponse {
    //session.clear();
    //session.flush();
    let credentials = Credentials {
        username: form.username.clone(),
        password: form.password,
    };

    if let Ok(user_id) = hc_validate_credentials(&state.hcdb, credentials).await
    //map_err(map_hc_error)
    //fix me, should handle error here in case db error, etc.
    {
        if session.insert("user_id", user_id).is_ok()
            && session.insert("username", form.username).is_ok()
        {
            return Redirect::to("/"); //index.html
        }
    }

    //session.clear();
    Redirect::to("/login")
}

pub async fn logout(session: Session) -> impl IntoResponse {
    session.clear();
    //session.flush();
    //FlashMessage::error(String::from("Authentication error")).send();
    Redirect::to("/login")
}

pub async fn new_user_get() -> impl IntoResponse {
    let error_html = String::from("");
    // for m in flash_messages.iter().filter(|m| m.level() == Level::Error) {
    //     writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    // }

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
    "##
    );

    Html(b)
}

pub async fn new_user_post(
    State(state): State<AxumAppState>,
    extract::Form(form): extract::Form<CreateUserFormData>,
) -> impl IntoResponse {
    let username = form.username;
    let password = form.password;
    let confirm_password = form.confirm_password;
    let email = form.email;

    let timestamp = libhc::get_timestamp();

    if username.len() > 1 && password.len() > 3 && email.len() > 6 && password == confirm_password {
        match libhc::hc_create_user(&state.hcdb, &username, &password, &email, timestamp)
            .await
            //.map_err(map_hc_error)
        {
            Ok(_user_id) => {
                //session.clear(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                //if session.insert("user_id", user_id).is_ok() {
                    Redirect::to("/login")
                //}
            }
            Err(_e) => {
                //session.purge();
                //FlashMessage::error(String::from("Create user error")).send();
                Redirect::to("/newuser")
            }
        }
    } else {
        //session.purge();
        //FlashMessage::error(String::from("Create user error")).send();
        Redirect::to("/newuser")
    }
}

use libhc::hc_create_oauth_user;
use libhc::HcError::Database;
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
use sign_in_with_apple::AppleClaims;
use sign_in_with_apple::GoogleClaims;
use sign_in_with_apple::Issuer;
use std::env;

#[derive(Deserialize)]
pub struct AuthRequest {
    code: Option<String>,
    state: String,
    id_token: Option<String>,
    user: Option<String>,
}

// pub struct AppState {
//     pub apple_oauth: BasicClient,
//     pub google_oauth: BasicClient,
// }

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

pub async fn oauth_login_apple(session: Session, cookies: Cookies) -> impl IntoResponse {
    let (pkce_code_challenge, _pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce = uuid::Uuid::new_v4(); // use UUID as random and unique nonce

    let (authorize_url, csrf_state) = get_apple_client()
        .authorize_url(CsrfToken::new_random)
        .set_response_type(&ResponseType::new("code id_token".to_string()))
        .add_extra_param("response_mode".to_string(), "form_post".to_string())
        .add_extra_param("nonce".to_string(), nonce.to_string())
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("name".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_code_challenge) //apple does not support this, but no problem including it
        .url();

    session.clear();

    let cookie = Cookie::build(OAUTH_COOKIE, csrf_state.secret().to_string())
        // .domain("hoplite-challenge.philolog.us")
        // .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::None) //this must be None for oauth
        .finish();
    let cookie_nonce = Cookie::build(OAUTH_COOKIE_NONCE, nonce.to_string())
        // .domain("hoplite-challenge.philolog.us")
        // .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::None) //this must be None for oauth
        .finish();
    cookies.add(cookie);
    cookies.add(cookie_nonce);

    Redirect::to(&authorize_url.to_string())
}

pub async fn oauth_login_google(session: Session, cookies: Cookies) -> impl IntoResponse {
    // Google supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, _pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce = uuid::Uuid::new_v4(); // use UUID as random and unique nonce

    let (authorize_url, csrf_state) = get_google_client()
        .authorize_url(CsrfToken::new_random)
        .set_response_type(&ResponseType::new("code id_token".to_string()))
        .add_extra_param("response_mode".to_string(), "form_post".to_string())
        .add_extra_param("nonce".to_string(), nonce.to_string())
        .add_scope(Scope::new("openid".to_string()))
        // .add_scope(Scope::new("name".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_code_challenge) // apple does not support this
        .url();

    session.clear();

    let cookie = Cookie::build(OAUTH_COOKIE, csrf_state.secret().to_string())
        // .domain("hoplite-challenge.philolog.us")
        // .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::None) //this must be None for oauth
        .finish();
    let cookie_nonce = Cookie::build(OAUTH_COOKIE_NONCE, nonce.to_string())
        // .domain("hoplite-challenge.philolog.us")
        // .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::None) //this must be None for oauth
        .finish();
    cookies.add(cookie);
    cookies.add(cookie_nonce);

    Redirect::to(&authorize_url.to_string())
}

pub async fn oauth_auth_apple(
    session: Session,
    cookies: Cookies,
    State(state): State<AxumAppState>,
    extract::Form(params): extract::Form<AuthRequest>,
) -> impl IntoResponse {
    let saved_state = match cookies.get(OAUTH_COOKIE) {
        Some(v) => Some(v.value().to_string()),
        None => None,
    };
    cookies.remove(Cookie::new(OAUTH_COOKIE, ""));
    let oauth2_nonce = match cookies.get(OAUTH_COOKIE_NONCE) {
        Some(v) => Some(v.value().to_string()),
        None => None,
    };
    cookies.remove(Cookie::new(OAUTH_COOKIE_NONCE, ""));

    if let Some(param_code) = &params.code {
        let code = AuthorizationCode::new(param_code.to_string());
        let received_state = CsrfToken::new(params.state);

        let _token = get_apple_client().exchange_code(code);

        if let Some(id_token) = params.id_token {
            if saved_state.is_none() || saved_state.clone().unwrap() == *received_state.secret() {
                tracing::error!(
                    "oauth2 state did not match: stored state: {:?}, received state: {:?}",
                    saved_state,
                    received_state.secret()
                );
                return Redirect::to("/login");
            }

            if let Ok(result) = sign_in_with_apple::validate::<AppleClaims>(
                &env::var("APPLE_CLIENT_ID")
                    .expect("Missing the APPLE_CLIENT_ID environment variable."),
                &id_token,
                false,
                Issuer::APPLE,
            )
            .await
            {
                let (first_name, last_name, mut email) = match serde_json::from_str::<AppleOAuthUser>(
                    &params.user.unwrap_or(String::from("")),
                ) {
                    Ok(apple_oauth_user) => (
                        apple_oauth_user.name.first_name.unwrap_or(String::from("")),
                        apple_oauth_user.name.last_name.unwrap_or(String::from("")),
                        apple_oauth_user.email.unwrap_or(String::from("")),
                    ),
                    _ => (String::from(""), String::from(""), String::from("")),
                };
                email = if email.is_empty() {
                    result.claims.email.unwrap_or(String::from(""))
                } else {
                    email
                };

                let sub = result.claims.sub;
                let iss = result.claims.iss;
                let nonce = result.claims.nonce.unwrap_or(String::from(""));

                if oauth2_nonce.is_some() && oauth2_nonce.clone().unwrap() != nonce {
                    tracing::error!(
                        "oauth2 nonce did not match: stored nonce: {:?}, received nonce: {:?}",
                        oauth2_nonce,
                        nonce
                    );
                    return Redirect::to("/login");
                }

                let timestamp = libhc::get_timestamp();
                match hc_create_oauth_user(
                    &state.hcdb,
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
                        session.clear(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                        if session.insert("user_id", user_id).is_ok()
                            && session.insert("username", user_name).is_ok()
                        {
                            return Redirect::to("/");
                        }
                    }
                    Err(Database(e)) => {
                        tracing::error!("oauth2 error logging in: {:?}", e);
                        //FlashMessage::error(e.to_string()).send();
                        //return Redirect::to("/login");
                    }
                    _ => (),
                }
            } else {
                tracing::error!("oauth2 token is not valid");
            }
        } else {
            tracing::error!("oauth2 params.id_token is none");
        }
    } else {
        tracing::error!("oauth2 code not set");
    }

    Redirect::to("/login")
}

pub async fn oauth_auth_google(
    session: Session,
    cookies: Cookies,
    State(state): State<AxumAppState>,
    extract::Form(params): extract::Form<AuthRequest>,
) -> impl IntoResponse {
    let saved_state = match cookies.get(OAUTH_COOKIE) {
        Some(v) => Some(v.value().to_string()),
        None => None,
    };
    cookies.remove(Cookie::new(OAUTH_COOKIE, ""));
    let oauth2_nonce = match cookies.get(OAUTH_COOKIE_NONCE) {
        Some(v) => Some(v.value().to_string()),
        None => None,
    };
    cookies.remove(Cookie::new(OAUTH_COOKIE_NONCE, ""));

    if let Some(param_code) = &params.code {
        let code = AuthorizationCode::new(param_code.to_string());
        let received_state = CsrfToken::new(params.state);
        //let user = params.user.clone(); //google doesn't send user this way

        let _token = get_google_client().exchange_code(code);

        if let Some(id_token) = params.id_token {
            if saved_state.is_none() || saved_state.clone().unwrap() == *received_state.secret() {
                tracing::error!(
                    "oauth2 state did not match: stored state: {:?}, received state: {:?}",
                    saved_state,
                    received_state.secret()
                );
                return Redirect::to("/login");
            }

            if let Ok(result) = sign_in_with_apple::validate::<GoogleClaims>(
                &env::var("GOOGLE_CLIENT_ID")
                    .expect("Missing the GOOGLE_CLIENT_ID environment variable."),
                &id_token,
                false,
                Issuer::GOOGLE,
            )
            .await
            {
                let first_name = String::from("");
                let last_name = String::from("");
                let email = result.claims.email.unwrap_or(String::from(""));

                let sub = result.claims.sub;
                let iss = result.claims.iss;
                let nonce = result.claims.nonce.unwrap_or(String::from(""));

                if oauth2_nonce.is_some() && oauth2_nonce.clone().unwrap() != nonce {
                    tracing::error!(
                        "oauth2 nonce did not match: stored nonce: {:?}, received nonce: {:?}",
                        oauth2_nonce,
                        nonce
                    );
                    return Redirect::to("/login");
                }

                let timestamp = libhc::get_timestamp();

                match hc_create_oauth_user(
                    &state.hcdb,
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
                        session.clear(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                        if session.insert("user_id", user_id).is_ok()
                            && session.insert("username", user_name).is_ok()
                        {
                            return Redirect::to("/");
                        }
                    }
                    Err(Database(e)) => {
                        tracing::error!("oauth2 error logging in: {:?}", e);
                        //FlashMessage::error(e.to_string()).send();
                        //return Redirect::to("/login");
                    }
                    _ => (),
                }
            } else {
                tracing::error!("oauth2 token is not valid");
            }
        } else {
            tracing::error!("oauth2 params.id_token is none");
        }
    } else {
        tracing::error!("oauth2 code not set");
    }

    Redirect::to("/login")
}
