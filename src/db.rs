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

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use sqlx::types::Uuid;
use crate::SessionsListQuery;
use crate::UserResult;
use crate::MoveResult;
use crate::SessionResult;
use crate::MoveType;

pub async fn validate_login_db(
    pool: &SqlitePool,
    username:&str,
    password:&str,
) -> Result<Uuid, sqlx::Error> {

    let query = "SELECT user_id,user_name,password,email,user_type,timestamp FROM users WHERE user_name = ? AND password = ? LIMIT 1;";
    let res:UserResult = sqlx::query_as(query)
        .bind(username)
        .bind(password)
        .fetch_one(pool)
        .await?;

    Ok(res.user_id)
}

pub async fn get_user_id(
    pool: &SqlitePool,
    username:&str,
) -> Result<UserResult, sqlx::Error> {

    let query = "SELECT user_id,user_name,password,email,user_type,timestamp FROM users WHERE user_name = ? LIMIT 1;";
    let res:UserResult = sqlx::query_as(query)
        .bind(username)
        .fetch_one(pool)
        .await?;

    Ok(res)
}

pub async fn insert_session(
    pool: &SqlitePool,
    user_id: Uuid,
    highest_unit: Option<u32>,
    opponent_id: Option<Uuid>,
    max_changes: u8,
    timestamp: i64,
) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let uuid = sqlx::types::Uuid::new_v4();

    let query = r#"INSERT INTO sessions VALUES (?,?,?,?,"",?,0,0,?);"#;
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind(user_id)
        .bind(opponent_id)
        .bind(highest_unit)
        .bind(max_changes)
        .bind(timestamp)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(uuid)
}

pub async fn get_sessions(
    pool: &SqlitePool,
    user_id: sqlx::types::Uuid,
) -> Result<Vec<SessionsListQuery>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    //strftime('%Y-%m-%d %H:%M:%S', DATETIME(timestamp, 'unixepoch')) as timestamp, 
    //    ORDER BY updated DESC \
    let query = "SELECT session_id AS session_id, challenged_user_id AS challenged, challenged_user_id AS opponent_user_id, b.user_name AS username, \
    strftime('%Y-%m-%d %H:%M:%S', DATETIME(a.timestamp, 'unixepoch')) as timestamp \
    FROM sessions a LEFT JOIN users b ON a.challenged_user_id = b.user_id \
    where challenger_user_id = ? \
    UNION SELECT session_id AS session_id, challenged_user_id AS challenged, challenged_user_id AS opponent_user_id, b.user_name AS username, \
    strftime('%Y-%m-%d %H:%M:%S', DATETIME(a.timestamp, 'unixepoch')) as timestamp \
    FROM sessions a LEFT JOIN users b ON a.challenger_user_id = b.user_id \
    where challenged_user_id  = ? \
    ORDER BY timestamp DESC \
    LIMIT 20000;";

    //println!("query: {} {:?}", query, user_id);
    let res: Vec<SessionsListQuery> = sqlx::query(query)
        .bind(user_id)
        .bind(user_id)
        .map(|rec: SqliteRow| {
            SessionsListQuery { session_id: rec.get("session_id"), challenged:rec.get("challenged"), opponent:rec.get("opponent_user_id"), opponent_name: rec.get("username"),timestamp:rec.get("timestamp"), myturn:false, move_type:MoveType::Practice }
        })
        .fetch_all(&mut tx)
        .await?;

    tx.commit().await?;
        
    Ok(res)
}

pub async fn get_last_move(pool: &SqlitePool, session_id: sqlx::types::Uuid) -> Result<MoveResult, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let res = get_last_move_tx(&mut tx, session_id).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn get_last_move_tx<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    session_id: sqlx::types::Uuid,
) -> Result<MoveResult, sqlx::Error> {

    let query = "SELECT * \
    FROM moves \
    where session_id = ? \
    ORDER BY asktimestamp DESC \
    LIMIT 1;";

    //println!("query: {} {:?}", query, user_id);
    let res: MoveResult = sqlx::query_as(query)
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        
    Ok(res)
}

pub async fn get_last_two_moves<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    session_id: sqlx::types::Uuid,
) -> Result<Vec<MoveResult>, sqlx::Error> {
    
    let query = "SELECT * \
        FROM moves \
        where session_id = ? \
        ORDER BY asktimestamp DESC \
        LIMIT 2;";
    
    //println!("query: {} {:?}", query, user_id);
    let res: Vec<MoveResult> = sqlx::query_as(query)
        .bind(session_id)
        .fetch_all(&mut *tx)
        .await?;
        
    Ok(res)
}

pub async fn get_session(
    pool: &SqlitePool,
    session_id: sqlx::types::Uuid,
) -> Result<SessionResult, sqlx::Error> {

    let query = "SELECT * \
    FROM sessions \
    where session_id = ? \
    LIMIT 1;";

    let res: SessionResult = sqlx::query_as(query)
        .bind(session_id)
        .fetch_one(pool)
        .await?;

    Ok(res)
}

pub async fn get_used_verbs(
    pool: &SqlitePool,
    session_id: sqlx::types::Uuid,
) -> Result<Vec<u32>, sqlx::Error> {

    let query = "SELECT verb_id \
    FROM moves \
    where verb_id IS NOT NULL AND session_id = ?;";

    let res: Vec<u32> = sqlx::query(query)
        .bind(session_id)
        .map(|rec: SqliteRow| {
            rec.get("verb_id")
        })
        .fetch_all(pool)
        .await?;

    Ok(res)
}

pub async fn insert_ask_move(
    pool: &SqlitePool,
    user_id: Option<Uuid>,
    session_id: Uuid,
    person: u8,
    number: u8,
    tense: u8,
    mood: u8,
    voice: u8,
    verb_id: u32,
    timestamp:i64,
) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let uuid = sqlx::types::Uuid::new_v4();

    let query = "INSERT INTO moves VALUES (?,?,?,NULL,?,?,?,?,?,?,NULL,NULL,NULL,NULL,NULL,NULL,?, NULL);";
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind(session_id)
        .bind(user_id)
        .bind(verb_id)
        .bind(person)
        .bind(number)
        .bind(tense)
        .bind(mood)
        .bind(voice)
        .bind(timestamp)
        //answer timestamp
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(uuid)
}

pub async fn update_answer_move(
    pool: &SqlitePool,
    session_id: Uuid,
    user_id: Uuid,
    answer: &str,
    correct_answer:&str,
    is_correct:bool,
    time: &str,
    mf_pressed:bool,
    timed_out:bool,
    timestamp:i64,
) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let m = get_last_move_tx(&mut tx, session_id).await?;

    let query = "UPDATE moves SET answer_user_id=?, answer=?, correct_answer=?, is_correct=?, time=?, mf_pressed=?, timed_out=?, answeredtimestamp=? WHERE move_id=?;";
    let _res = sqlx::query(query)
        .bind(user_id)
        .bind(answer)
        .bind(correct_answer)
        .bind(is_correct)
        .bind(time)
        .bind(mf_pressed)
        .bind(timed_out)
        .bind(timestamp)
        .bind(m.move_id)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(1)
}

pub async fn create_user(pool: &SqlitePool, username:&str, password:&str, email:&str, timestamp:i64) -> Result<Uuid, sqlx::Error> {

    if username.len() < 2 || username.len() > 30 || password.len() < 8 || password.len() > 60 || email.len() < 6 || email.len() > 120 {
        return Err(sqlx::Error::RowNotFound);
    }

    let uuid = sqlx::types::Uuid::new_v4();
    let query = "INSERT INTO users VALUES (?,?,?,?,0,?);";
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind(username)
        .bind(password)
        .bind(email)
        .bind(timestamp)
        .execute(pool)
        .await?;

    Ok(uuid)
}

pub async fn create_db(pool: &SqlitePool) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = r#"CREATE TABLE IF NOT EXISTS users ( 
user_id BLOB PRIMARY KEY NOT NULL, 
user_name TEXT, 
password TEXT, 
email TEXT,
user_type INT NOT NULL DEFAULT 0,
timestamp INT NOT NULL DEFAULT 0,
UNIQUE(user_name)
);"#;

    let _res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = r#"CREATE TABLE IF NOT EXISTS sessions ( 
session_id BLOB PRIMARY KEY NOT NULL, 
challenger_user_id BLOB, 
challenged_user_id BLOB, 
highest_unit INT,
custom_verbs TEXT, 
max_changes INT,
challenger_score INT,
challenged_score INT,
timestamp INT NOT NULL DEFAULT 0,
FOREIGN KEY (challenger_user_id) REFERENCES users(user_id), 
FOREIGN KEY (challenged_user_id) REFERENCES users(user_id)
);"#;
    let _res = sqlx::query(query)
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
answer TEXT,
correct_answer TEXT,
is_correct INT,
time TEXT, 
timed_out INT, 
mf_pressed INT, 
asktimestamp INT NOT NULL DEFAULT 0, 
answeredtimestamp INT, 
FOREIGN KEY (ask_user_id) REFERENCES users(user_id), 
FOREIGN KEY (answer_user_id) REFERENCES users(user_id), 
FOREIGN KEY (session_id) REFERENCES sessions(session_id) 
);"#;
    let _res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = "CREATE INDEX IF NOT EXISTS move_session_id_idx ON moves (session_id);";
    let _res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = "REPLACE INTO users VALUES (?,?,?,?,0,?);";
    let uuid = Uuid::from_u128(0x8CD36EFFDF5744FF953B29A473D12347);//sqlx::types::Uuid::new_v4();
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind("user1")
        .bind("1234")
        .bind("user1@email.com")
        .bind(0)
        .execute(&mut tx)
        .await?;

    let uuid = Uuid::from_u128(0xD75B0169E7C343838298136E3D63375C);//sqlx::types::Uuid::new_v4();
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind("user2")
        .bind("1234")
        .bind("user2@email.com")
        .bind(0)
        .execute(&mut tx)
        .await?;

    //to test invalid user
    let uuid = Uuid::from_u128(0x00000000000000000000000000000001);//sqlx::types::Uuid::new_v4();
    let _res = sqlx::query(query)
        .bind(uuid)
        .bind("user3")
        .bind("1234")
        .bind("user2@email.com")
        .bind(0)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(1)
}
