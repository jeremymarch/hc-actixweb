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

//use sqlx::sqlite::SqliteRow;
use crate::AnswerQuery;
use crate::AskQuery;
use crate::CreateSessionQuery;
use crate::HcDb;
use crate::MoveResult;
use crate::MoveType;
use crate::SessionResult;
use crate::SessionsListQuery;
use crate::UserResult;
use sqlx::types::Uuid;
use sqlx::Row;

use sqlx::postgres::PgRow;
use sqlx::Postgres;

impl HcDb {
    // pub async fn begin_tx(&self) -> Result<Transaction<sqlx::Sqlite>, sqlx::Error> {
    //     let mut tx = self.db.begin().await?;
    //     Ok(tx)
    // }
    // pub async fn commit_tx<'a, 'b>(&mut self, tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>) -> Result<(), sqlx::Error> {
    //     tx.commit().await
    // }

    pub async fn add_to_score<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        session_id: Uuid,
        user_to_score: &str,
        points: i32,
    ) -> Result<u32, sqlx::Error> {
        let query = format!(
            "UPDATE sessions SET {user_to_score} = {user_to_score} + $1 WHERE session_id = $2;"
        );
        let _res = sqlx::query(&query)
            .bind(points)
            .bind(session_id)
            .execute(&mut *tx)
            .await?;

        Ok(1)
    }

    pub async fn validate_login_db(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Uuid, sqlx::Error> {
        let query = "SELECT user_id,user_name,password,email,user_type,timestamp FROM users WHERE user_name = $1 AND password = $2 LIMIT 1;";
        let res: UserResult = sqlx::query_as(query)
            .bind(username)
            .bind(password)
            .fetch_one(&self.db)
            .await?;

        Ok(res.user_id)
    }

    pub async fn get_user_id(&self, username: &str) -> Result<UserResult, sqlx::Error> {
        let query = "SELECT user_id,user_name,password,email,user_type,timestamp FROM users WHERE user_name = $1 LIMIT 1;";
        let res: UserResult = sqlx::query_as(query)
            .bind(username)
            .fetch_one(&self.db)
            .await?;

        Ok(res)
    }

    pub async fn insert_session(
        &self,
        user_id: Uuid,
        highest_unit: Option<i16>,
        opponent_id: Option<Uuid>,
        info: &CreateSessionQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error> {
        let mut tx = self.db.begin().await?;

        let uuid = self
            .insert_session_tx(&mut tx, user_id, highest_unit, opponent_id, info, timestamp)
            .await?;

        tx.commit().await?;

        Ok(uuid)
    }

    pub async fn insert_session_tx<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        user_id: Uuid,
        highest_unit: Option<i16>,
        opponent_id: Option<Uuid>,
        info: &CreateSessionQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error> {
        let uuid = sqlx::types::Uuid::new_v4();

        let query = r#"INSERT INTO sessions (
            session_id,
            challenger_user_id,
            challenged_user_id,
            current_move,
            name,
            highest_unit,
            custom_verbs,
            custom_params,
            max_changes,
            challenger_score,
            challenged_score,
            practice_reps_per_verb,
            countdown,
            max_time,
            timestamp,
            status) VALUES ($1,$2,$3,NULL,NULL,$4,$5,$6,$7,0,0,$8,$9,$10,$11,1);"#;
        let _res = sqlx::query(query)
            .bind(uuid)
            .bind(user_id)
            .bind(opponent_id)
            .bind(highest_unit)
            .bind(&info.verbs)
            .bind(&info.params)
            .bind(info.max_changes)
            .bind(info.practice_reps_per_verb)
            .bind(info.countdown as i32)
            .bind(info.max_time)
            .bind(timestamp)
            .execute(&mut *tx)
            .await?;

        Ok(uuid)
    }

    pub async fn get_sessions(
        &self,
        user_id: sqlx::types::Uuid,
    ) -> Result<Vec<SessionsListQuery>, sqlx::Error> {
        let mut tx = self.db.begin().await?;

        //strftime('%Y-%m-%d %H:%M:%S', DATETIME(timestamp, 'unixepoch')) as timestamp,
        //    ORDER BY updated DESC \
        let query = "SELECT session_id AS session_id, challenged_user_id AS challenged, b.user_name AS username, challenger_score as myscore, challenged_score as theirscore, \
        a.timestamp as timestamp, countdown, max_time, max_changes \
        FROM sessions a LEFT JOIN users b ON a.challenged_user_id = b.user_id \
        where challenger_user_id = $1 \
        UNION SELECT session_id AS session_id, challenged_user_id AS challenged, b.user_name AS username, challenged_score as myscore, challenger_score as theirscore, \
        a.timestamp as timestamp, countdown, max_time, max_changes \
        FROM sessions a LEFT JOIN users b ON a.challenger_user_id = b.user_id \
        where challenged_user_id  = $2 \
        ORDER BY timestamp DESC \
        LIMIT 20000;";

        //println!("query: {} {:?}", query, user_id);
        let res: Vec<SessionsListQuery> = sqlx::query(query)
            .bind(user_id)
            .bind(user_id)
            .map(|rec: PgRow| {
                SessionsListQuery {
                    session_id: rec.get("session_id"),
                    challenged: rec.get("challenged"), /*opponent:rec.get("opponent_user_id"),*/
                    opponent_name: rec.get("username"),
                    timestamp: rec.get("timestamp"),
                    myturn: false,
                    move_type: MoveType::Practice,
                    my_score: rec.get("myscore"),
                    their_score: rec.get("theirscore"),
                    countdown: rec.get("countdown"),
                    max_time: rec.get("max_time"),
                    max_changes: rec.get("max_changes"),
                }
            })
            .fetch_all(&mut tx)
            .await?;

        /*let res2 = match res {
            Ok(e) => e,
            Err(e) => {println!("error: {:?}", e); return Err(e); },
        };*/

        tx.commit().await?;

        Ok(res)
    }

    pub async fn get_last_move(
        &self,
        session_id: sqlx::types::Uuid,
    ) -> Result<MoveResult, sqlx::Error> {
        let mut tx = self.db.begin().await?;
        let res = self.get_last_move_tx(&mut tx, session_id).await?;
        tx.commit().await?;
        Ok(res)
    }

    pub async fn get_last_move_tx<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        session_id: sqlx::types::Uuid,
    ) -> Result<MoveResult, sqlx::Error> {
        let query = "SELECT * \
        FROM moves \
        where session_id = $1 \
        ORDER BY asktimestamp DESC \
        LIMIT $2;";

        //println!("query: {} {:?}", query, user_id);
        let res: MoveResult = sqlx::query_as(query)
            .bind(session_id)
            .bind(1)
            .fetch_one(&mut *tx)
            .await?;

        Ok(res)
    }

    pub async fn get_last_n_moves<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        session_id: sqlx::types::Uuid,
        n: u8,
    ) -> Result<Vec<MoveResult>, sqlx::Error> {
        let query = "SELECT * \
            FROM moves \
            where session_id = $1 \
            ORDER BY asktimestamp DESC \
            LIMIT $2;";

        //println!("query: {} {:?}", query, user_id);
        let res: Vec<MoveResult> = sqlx::query_as(query)
            .bind(session_id)
            .bind(n as i32)
            .fetch_all(&mut *tx)
            .await?;

        Ok(res)
    }

    pub async fn get_session(
        &self,
        session_id: sqlx::types::Uuid,
    ) -> Result<SessionResult, sqlx::Error> {
        let query = "SELECT * \
        FROM sessions \
        where session_id = $1 \
        LIMIT 1;";

        let res: SessionResult = sqlx::query_as(query)
            .bind(session_id)
            .fetch_one(&self.db)
            .await?;

        Ok(res)
    }

    pub async fn get_session_tx<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        session_id: sqlx::types::Uuid,
    ) -> Result<SessionResult, sqlx::Error> {
        let query = "SELECT * \
        FROM sessions \
        where session_id = $1 \
        LIMIT 1;";

        let res: SessionResult = sqlx::query_as(query)
            .bind(session_id)
            .fetch_one(&mut *tx)
            .await?;

        Ok(res)
    }

    pub async fn get_used_verbs(
        &self,
        session_id: sqlx::types::Uuid,
    ) -> Result<Vec<i32>, sqlx::Error> {
        let query = "SELECT verb_id \
        FROM moves \
        where verb_id IS NOT NULL AND session_id = $1;";

        let res: Vec<i32> = sqlx::query(query)
            .bind(session_id)
            .map(|rec: PgRow| rec.get("verb_id"))
            .fetch_all(&self.db)
            .await?;

        Ok(res)
    }

    pub async fn insert_ask_move(
        &self,
        user_id: Option<Uuid>,
        info: &AskQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error> {
        let mut tx = self.db.begin().await?;

        let uuid = self
            .insert_ask_move_tx(&mut tx, user_id, info, timestamp)
            .await?;

        tx.commit().await?;

        Ok(uuid)
    }

    pub async fn insert_ask_move_tx<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        user_id: Option<Uuid>,
        info: &AskQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error> {
        let uuid = sqlx::types::Uuid::new_v4();

        let query = "INSERT INTO moves VALUES ($1,$2,$3,NULL,$4,$5,$6,$7,$8,$9,NULL,NULL,NULL,NULL,NULL,NULL,$10, NULL);";
        let _res = sqlx::query(query)
            .bind(uuid)
            .bind(info.session_id)
            .bind(user_id)
            .bind(info.verb)
            .bind(info.person)
            .bind(info.number)
            .bind(info.tense)
            .bind(info.mood)
            .bind(info.voice)
            .bind(timestamp)
            //answer timestamp
            .execute(&mut *tx)
            .await?;

        Ok(uuid)
    }

    pub async fn update_answer_move(
        &self,
        info: &AnswerQuery,
        user_id: Uuid,
        correct_answer: &str,
        is_correct: bool,
        mf_pressed: bool,
        timestamp: i64,
    ) -> Result<u32, sqlx::Error> {
        let mut tx = self.db.begin().await?;

        let a = self
            .update_answer_move_tx(
                &mut tx,
                info,
                user_id,
                correct_answer,
                is_correct,
                mf_pressed,
                timestamp,
            )
            .await?;

        tx.commit().await?;

        Ok(a)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_answer_move_tx<'a, 'b>(
        &self,
        tx: &'a mut sqlx::Transaction<'b, Postgres>,
        info: &AnswerQuery,
        user_id: Uuid,
        correct_answer: &str,
        is_correct: bool,
        mf_pressed: bool,
        timestamp: i64,
    ) -> Result<u32, sqlx::Error> {
        let m = self.get_last_move_tx(&mut *tx, info.session_id).await?;

        let query = "UPDATE moves SET answer_user_id=$1, answer=$2, correct_answer=$3, is_correct=$4, time=$5, mf_pressed=$6, timed_out=$7, answeredtimestamp=$8 WHERE move_id=$9;";
        let _res = sqlx::query(query)
            .bind(user_id)
            .bind(info.answer.clone())
            .bind(correct_answer)
            .bind(is_correct)
            .bind(info.time.clone())
            .bind(mf_pressed)
            .bind(info.timed_out)
            .bind(timestamp)
            .bind(m.move_id)
            .execute(&mut *tx)
            .await?;

        Ok(1)
    }

    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
        email: &str,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error> {
        if username.len() < 2
            || username.len() > 30
            || password.len() < 8
            || password.len() > 60
            || email.len() < 6
            || email.len() > 120
        {
            return Err(sqlx::Error::RowNotFound);
        }

        let uuid = sqlx::types::Uuid::new_v4();
        let query = "INSERT INTO users VALUES ($1,$2,$3,$4,0,$5);";
        let _res = sqlx::query(query)
            .bind(uuid)
            .bind(username)
            .bind(password)
            .bind(email)
            .bind(timestamp)
            .execute(&self.db)
            .await?;

        Ok(uuid)
    }

    pub async fn create_db(&self) -> Result<u32, sqlx::Error> {
        let mut tx = self.db.begin().await?;

        let query = r#"CREATE TABLE IF NOT EXISTS users ( 
    user_id UUID PRIMARY KEY NOT NULL, 
    user_name TEXT, 
    password TEXT, 
    email TEXT,
    user_type INT NOT NULL DEFAULT 0,
    timestamp BIGINT NOT NULL DEFAULT 0,
    UNIQUE(user_name)
    );"#;

        let _res = sqlx::query(query).execute(&mut tx).await?;

        let query = r#"CREATE TABLE IF NOT EXISTS sessions ( 
    session_id UUID PRIMARY KEY NOT NULL, 
    challenger_user_id UUID NOT NULL, 
    challenged_user_id UUID DEFAULT NULL, 
    current_move UUID DEFAULT NULL,
    name TEXT DEFAULT NULL,
    highest_unit SMALLINT,
    custom_verbs TEXT, 
    custom_params TEXT, 
    max_changes SMALLINT,
    challenger_score INT,
    challenged_score INT,
    practice_reps_per_verb SMALLINT,
    countdown INT,
    max_time INT,
    timestamp BIGINT NOT NULL DEFAULT 0,
    status INT NOT NULL DEFAULT 1,
    FOREIGN KEY (challenger_user_id) REFERENCES users(user_id), 
    FOREIGN KEY (challenged_user_id) REFERENCES users(user_id)
    );"#;
        let _res = sqlx::query(query).execute(&mut tx).await?;

        let query = r#"CREATE TABLE IF NOT EXISTS moves ( 
    move_id UUID PRIMARY KEY NOT NULL, 
    session_id UUID NOT NULL,
    ask_user_id UUID, 
    answer_user_id UUID, 
    verb_id INT, 
    person SMALLINT, 
    number SMALLINT, 
    tense SMALLINT, 
    mood SMALLINT, 
    voice SMALLINT, 
    answer VARCHAR(1024),
    correct_answer VARCHAR(1024),
    is_correct BOOL,
    time VARCHAR(255), 
    timed_out BOOL, 
    mf_pressed BOOL, 
    asktimestamp BIGINT NOT NULL DEFAULT 0, 
    answeredtimestamp BIGINT, 
    FOREIGN KEY (ask_user_id) REFERENCES users(user_id), 
    FOREIGN KEY (answer_user_id) REFERENCES users(user_id), 
    FOREIGN KEY (session_id) REFERENCES sessions(session_id) 
    );"#;
        let _res = sqlx::query(query).execute(&mut tx).await?;

        let query = "CREATE INDEX IF NOT EXISTS move_session_id_idx ON moves (session_id);";
        let _res = sqlx::query(query).execute(&mut tx).await?;

        tx.commit().await?;

        Ok(1)
    }
}
