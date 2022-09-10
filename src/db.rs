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

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqlitePool};
use sqlx::types::Uuid;
use crate::SessionsListQuery;
use crate::UserResult;
use crate::MoveResult;

//use unicode_normalization::UnicodeNormalization;

pub async fn get_user_id(
    pool: &SqlitePool,
    username:&str,
) -> Result<UserResult, sqlx::Error> {

    let query = "SELECT user_id,user_name,password,email,timestamp FROM users WHERE user_name = ? LIMIT 1;";
    let res:UserResult = sqlx::query_as(query)
        .bind(username)
        .fetch_one(pool)
        .await?;

    Ok(res)
}

pub async fn insert_session(
    pool: &SqlitePool,
    user_id: Uuid,
    unit: Option<u32>,
    opponent_id: Option<Uuid>,
    timestamp:i64,
) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let uuid = sqlx::types::Uuid::new_v4();

    let query = "INSERT INTO sessions VALUES (?,?,?,?);";
    let res = sqlx::query(query)
        .bind(uuid)
        .bind(user_id)
        .bind(opponent_id)
        .bind(timestamp)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(1)
}

pub async fn get_sessions(
    pool: &SqlitePool,
    user_id: sqlx::types::Uuid,
) -> Result<Vec<SessionsListQuery>, sqlx::Error> {
    //strftime('%Y-%m-%d %H:%M:%S', DATETIME(timestamp, 'unixepoch')) as timestamp, 
    //    ORDER BY updated DESC \
    let query = format!("SELECT session_id AS session_id, challenged_user_id AS opponent_user_id, b.user_name AS username, \
    strftime('%Y-%m-%d %H:%M:%S', DATETIME(a.timestamp, 'unixepoch')) as timestamp \
    FROM sessions a LEFT JOIN users b ON a.challenged_user_id = b.user_id \
    where challenger_user_id = ? \
    UNION SELECT session_id AS session_id, challenged_user_id AS opponent_user_id, b.user_name AS username, \
    strftime('%Y-%m-%d %H:%M:%S', DATETIME(a.timestamp, 'unixepoch')) as timestamp \
    FROM sessions a LEFT JOIN users b ON a.challenger_user_id = b.user_id \
    where challenged_user_id  = ? \
    ORDER BY timestamp DESC \
    LIMIT 20000;"
);
    //println!("query: {} {:?}", query, user_id);
    let mut res: Vec<SessionsListQuery> = sqlx::query(&query)
        .bind(user_id)
        .bind(user_id)
        .map(|rec: SqliteRow| {
            SessionsListQuery { session_id: rec.get("session_id"), opponent:rec.get("opponent_user_id"), opponent_name: rec.get("username"),timestamp:rec.get("timestamp"), myturn:false }
        })
        .fetch_all(pool)
        .await?;

    let query = "SELECT * FROM moves WHERE session_id = ? ORDER BY timestamp DESC LIMIT 1;";
    for r in &mut res {
        let subres:Result<MoveResult, sqlx::Error> = sqlx::query_as(query)
        .bind(r.session_id)
        .fetch_one(pool)
        .await;

        match subres {
            Ok(s) => { if s.ask_user_id == user_id { r.myturn = false } else { r.myturn = true } },
            Err(s) => r.myturn = false,
        }

    }    

    Ok(res)
}

pub async fn create_db(pool: &SqlitePool) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = r#"CREATE TABLE IF NOT EXISTS users ( 
user_id BLOB PRIMARY KEY NOT NULL, 
user_name TEXT, 
password TEXT, 
email TEXT,
timestamp INT NOT NULL DEFAULT 0,
UNIQUE(user_name)
);"#;

    let res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = r#"CREATE TABLE IF NOT EXISTS sessions ( 
session_id BLOB PRIMARY KEY NOT NULL, 
challenger_user_id BLOB, 
challenged_user_id BLOB, 
timestamp INT NOT NULL DEFAULT 0,
FOREIGN KEY (challenger_user_id) REFERENCES users(user_id), 
FOREIGN KEY (challenged_user_id) REFERENCES users(user_id)
);"#;
    let res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = r#"CREATE TABLE IF NOT EXISTS moves ( 
move_id BLOB PRIMARY KEY NOT NULL, 
session_id BLOB, 
ask_user_id BLOB, 
answer_user_id BLOB, 
verb_id INT, 
person INT, 
number INT, 
tense INT, 
mood INT, 
voice INT, 
time TEXT, 
timed_out INT, 
mf_pressed INT, 
timestamp INT NOT NULL DEFAULT 0, 
FOREIGN KEY (ask_user_id) REFERENCES users(user_id), 
FOREIGN KEY (answer_user_id) REFERENCES users(user_id), 
FOREIGN KEY (session_id) REFERENCES sessions(session_id) 
);"#;
    let res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = "INSERT INTO users VALUES (?,?,?,?,?);";
    let uuid = Uuid::from_u128(0x8CD36EFFDF5744FF953B29A473D12347);//sqlx::types::Uuid::new_v4();
    let res = sqlx::query(query)
        .bind(uuid)
        .bind("user1")
        .bind("1234")
        .bind("user1@email.com")
        .bind(0)
        .execute(&mut tx)
        .await?;

    let uuid = Uuid::from_u128(0xD75B0169E7C343838298136E3D63375C);//sqlx::types::Uuid::new_v4();
    let res = sqlx::query(query)
        .bind(uuid)
        .bind("user2")
        .bind("1234")
        .bind("user2@email.com")
        .bind(0)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(1)
}
