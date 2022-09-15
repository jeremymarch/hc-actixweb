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
use crate::SessionResult;
use crate::SessionState;

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
    let mut tx = pool.begin().await?;

    //strftime('%Y-%m-%d %H:%M:%S', DATETIME(timestamp, 'unixepoch')) as timestamp, 
    //    ORDER BY updated DESC \
    let query = format!("SELECT session_id AS session_id, challenged_user_id AS challenged, challenged_user_id AS opponent_user_id, b.user_name AS username, \
    strftime('%Y-%m-%d %H:%M:%S', DATETIME(a.timestamp, 'unixepoch')) as timestamp \
    FROM sessions a LEFT JOIN users b ON a.challenged_user_id = b.user_id \
    where challenger_user_id = ? \
    UNION SELECT session_id AS session_id, challenged_user_id AS challenged, challenged_user_id AS opponent_user_id, b.user_name AS username, \
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
            SessionsListQuery { session_id: rec.get("session_id"), challenged:rec.get("challenged"), opponent:rec.get("opponent_user_id"), opponent_name: rec.get("username"),timestamp:rec.get("timestamp"), myturn:false, move_type:0 }
        })
        .fetch_all(&mut tx)
        .await?;

    for r in &mut res {
        (r.myturn, r.move_type) = get_move_type(&mut tx, r.session_id, user_id, r.challenged).await;
    }   

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

    let query = format!("SELECT * \
    FROM moves \
    where session_id = ? \
    ORDER BY asktimestamp DESC \
    LIMIT 1;"
);
    //println!("query: {} {:?}", query, user_id);
    let res: MoveResult = sqlx::query_as(&query)
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        
    Ok(res)
}

pub async fn get_last_two_moves<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    session_id: sqlx::types::Uuid,
) -> Result<Vec<MoveResult>, sqlx::Error> {
    
    let query = format!("SELECT * \
        FROM moves \
        where session_id = ? \
        ORDER BY asktimestamp DESC \
        LIMIT 2;"
    );
    //println!("query: {} {:?}", query, user_id);
    let res: Vec<MoveResult> = sqlx::query_as(&query)
        .bind(session_id)
        .fetch_all(&mut *tx)
        .await?;
        
    Ok(res)
}

//0 practice, always my turn
//1 no moves have not been asked, I am challenger (I need to ask, it's my turn): nothing
//2 no moves have been asked: nothing
//3 q has not been answered by me: starting_form from -1, desc from -1, desc from 0
//4 q has not been asked by me: starting_form from -1, desc from -1, desc from 0, is_correct, correct answer, given answer, time, mf, timedout
//5 q has not been asked by you: starting_form from -1, desc from -1, desc from 0, is_correct, correct answer, given answer, time, mf, timedout
//6 q has not been answered by you: starting_form from -1, desc from -1, desc from 0,
//7 game has ended (no ones turn): starting_form from -1, desc from -1, desc from 0, is_correct, correct answer, given answer, time, mf, timedout
fn move_get_type(s:Option<&MoveResult>, user_id:Uuid, challenged_id:Option<Uuid>) -> (bool, u8) {
    let myturn:bool;
    let move_type:u8;

    match s {
        Some(s) => { 
            if challenged_id.is_none() { 
                myturn = true;
                move_type = 0; //practice, my turn always
            }
            else if s.ask_user_id == user_id { 
                if s.answer_user_id.is_some() { //xxxanswered, my turn to ask | I asked, they answered, their turn to ask (waiting for them to ask)
                    myturn = false;
                    move_type = 4;
                }
                else {
                    myturn = false; //unanswered, their turn to answer
                    move_type = 6;
                }
            } else { 
                if s.answer_user_id.is_some() { //xxxanswered, their turn to ask | they asked, I answered, my turn to ask
                    myturn = true;
                    move_type = 5;
                }
                else {
                    myturn = true; //unanswered, my turn to answer
                    move_type = 3;
                } 
            } 
        },
        None => {
            if challenged_id.is_some() { 
                if challenged_id.unwrap() == user_id {
                    myturn = false;
                    move_type = 2; //no moves yet, their turn to ask
                } 
                else {
                    myturn = true;
                    move_type = 1; //no moves yet, my turn to ask
                }
            }
            else {
                myturn = true;
                move_type = 0; //practice, my turn always (no moves yet)
            }
        },
    }
    (myturn, move_type)
}

pub async fn get_session_state(
    pool: &SqlitePool,
    user_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
) -> Result<SessionState, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = format!("SELECT * \
    FROM sessions \
    where session_id = ? \
    LIMIT 1;"
);
    //println!("query: {} {:?}", query, user_id);
    let res: SessionResult = sqlx::query_as(&query)
        .bind(session_id)
        .fetch_one(&mut tx)
        .await?;

    let m = get_last_two_moves(&mut tx, session_id).await?;
    let first = if m.len() > 0 { Some(&m[0]) } else { None };
    let (myturn, move_type) = move_get_type(first, user_id, res.challenged_user_id);
    //let res = get_move_type(&mut tx, session_id, user_id, res.challenged_user_id).await;

    let r = SessionState {
        session_id: session_id,
        move_type: move_type,
        myturn: myturn,
        starting_form: if m.len() == 2 && m[0].verb_id == m[1].verb_id { m[1].answer.clone() } else { Some("first pp of m[1].verb".to_string()) },
        answer: if m.len() > 0 { m[0].answer.clone() } else { None },
        is_correct: if m.len() > 0 && m[0].is_correct.is_some() { Some(m[0].is_correct.unwrap() != 0) } else { None },
        correct_answer: if m.len() > 0 { m[0].correct_answer.clone() } else { None },
        verb: if m.len() > 0 { m[0].verb_id } else { None },
        person: if m.len() > 0 { m[0].person } else { None },
        number: if m.len() > 0 { m[0].number } else { None },
        tense: if m.len() > 0 { m[0].tense } else { None },
        voice: if m.len() > 0 { m[0].voice } else { None },
        mood: if m.len() > 0 { m[0].mood } else { None },
        person_prev: if m.len() == 2 { m[1].person } else { None },
        number_prev: if m.len() == 2 { m[1].number } else { None },
        tense_prev: if m.len() == 2 { m[1].tense } else { None },
        voice_prev: if m.len() == 2 { m[1].voice } else { None },
        mood_prev: if m.len() == 2 { m[1].mood } else { None },
        time: if m.len() > 0 { m[0].time.clone() } else { None },
        response_to:"".to_string(),
        success:true,
        mesg:None,
    };
        
    tx.commit().await?;
    Ok(r)
}

pub async fn get_move_type<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>, session_id:Uuid, user_id: Uuid, challenged_id:Option<Uuid>) -> (bool, u8) {
    let query = "SELECT * FROM moves WHERE session_id = ? ORDER BY asktimestamp DESC LIMIT 1;";
    let subres:Result<MoveResult, sqlx::Error> = sqlx::query_as(query)
    .bind(session_id)
    .fetch_one(&mut *tx)
    .await;

    match subres {
        Ok(s) => {
            move_get_type(Some(&s), user_id, challenged_id)
        },
        Err(_) => {
            move_get_type(None, user_id, challenged_id)
        }
    }
}

pub async fn insert_ask_move(
    pool: &SqlitePool,
    user_id: Uuid,
    session_id: Uuid,
    person: u8,
    number: u8,
    tense: u8,
    mood: u8,
    voice: u8,
    verb_id: u32,
    timestamp:i64,
) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let uuid = sqlx::types::Uuid::new_v4();

    let query = "INSERT INTO moves VALUES (?,?,?,NULL,?,?,?,?,?,?,NULL,NULL,NULL,NULL,NULL,NULL,?, NULL);";
    let res = sqlx::query(query)
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

        // move_id BLOB PRIMARY KEY NOT NULL, 
        // session_id BLOB, 
        // ask_user_id BLOB, 
        // answer_user_id BLOB, 
        // verb_id INT, 
        // person INT, 
        // number INT, 
        // tense INT, 
        // mood INT, 
        // voice INT, 
        // time TEXT, 
        // timed_out INT, 
        // mf_pressed INT, 
        // timestamp INT NOT NULL DEFAULT 0, 

    tx.commit().await?;

    Ok(1)
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
    let res = sqlx::query(query)
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
    let res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = "CREATE INDEX IF NOT EXISTS move_session_id_idx ON moves (session_id);";
    let res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    let query = "REPLACE INTO users VALUES (?,?,?,?,?);";
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
