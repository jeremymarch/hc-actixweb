use axum::debug_handler;
use axum::extract;
use axum::extract::State;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Redirect;
use libhc::dbpostgres::HcDbPostgres;
use libhc::hc_validate_credentials;
use libhc::Credentials;
use secrecy::Secret;
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

pub async fn new_user_get() -> impl IntoResponse {
    let mut error_html = String::from("");
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
    session: Session,
    State(db): State<HcDbPostgres>,
    extract::Form(form): extract::Form<CreateUserFormData>,
) -> impl IntoResponse {
    let username = form.username;
    let password = form.password;
    let confirm_password = form.confirm_password;
    let email = form.email;

    let timestamp = libhc::get_timestamp();

    if username.len() > 1 && password.len() > 3 && email.len() > 6 && password == confirm_password {
        match libhc::hc_create_user(&db, &username, &password, &email, timestamp)
            .await
            //.map_err(map_hc_error)
        {
            Ok(_user_id) => {
                println!("here1");
                //session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
                //if session.insert("user_id", user_id).is_ok() {
                    Redirect::to("/login")
                //}
            }
            Err(e) => {
                println!("here2 {:?}", e);
                //session.purge();
                //FlashMessage::error(String::from("Create user error")).send();
                Redirect::to("/newuser")
            }
        }
    } else {
        println!("here3");
        //session.purge();
        //FlashMessage::error(String::from("Create user error")).send();
        Redirect::to("/newuser")
    }
}
