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

use crate::AnswerQuery;
use crate::AskQuery;
use crate::CreateSessionQuery;
use crate::GreekSynopsisResult;
use crate::HcDb;
use crate::HcError;
use crate::HcTrx;
use crate::MoveResult;
use crate::MoveType;
use crate::SessionResult;
use crate::SessionsListQuery;
use crate::SynopsisSaverRequest;
use crate::UserResult;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::sqlite::SqliteRow;
use sqlx::types::Uuid;
use sqlx::Transaction;
use sqlx::{Row, SqlitePool};

fn map_sqlx_error(err: sqlx::Error) -> HcError {
    match err {
        sqlx::Error::Configuration(e) => HcError::Database(format!("sqlx Configuration: {}", e)),
        sqlx::Error::Database(e) => HcError::Database(format!("sqlx Database: {}", e)),
        sqlx::Error::Io(e) => HcError::Database(format!("sqlx Io: {}", e)),
        sqlx::Error::Tls(e) => HcError::Database(format!("sqlx Tls: {}", e)),
        sqlx::Error::Protocol(e) => HcError::Database(format!("sqlx Protocol: {}", e)),
        sqlx::Error::RowNotFound => HcError::Database(String::from("sqlx RowNotFound")),
        sqlx::Error::TypeNotFound { .. } => HcError::Database(String::from("sqlx TypeNotFound")),
        sqlx::Error::ColumnIndexOutOfBounds { .. } => {
            HcError::Database(String::from("sqlx ColumnIndexOutOfBounds"))
        }
        sqlx::Error::ColumnNotFound(e) => HcError::Database(format!("sqlx ColumnNotFound: {}", e)),
        sqlx::Error::ColumnDecode { .. } => HcError::Database(String::from("sqlx ColumnDecode")),
        sqlx::Error::Decode(e) => HcError::Database(format!("sqlx Decode: {}", e)),
        sqlx::Error::PoolTimedOut => HcError::Database(String::from("sqlx PoolTimedOut")),
        sqlx::Error::PoolClosed => HcError::Database(String::from("sqlx PoolClosed")),
        sqlx::Error::WorkerCrashed => HcError::Database(String::from("sqlx WorkerCrashed")),
        sqlx::Error::Migrate(e) => HcError::Database(format!("sqlx Migrate: {}", e)),
        _ => HcError::Database(String::from("sqlx unknown error")),
    }
}

#[derive(Clone, Debug)]
pub struct HcDbSqlite {
    pub db: SqlitePool,
}

#[derive(Debug)]
pub struct HcDbSqliteTrx<'a> {
    pub tx: Transaction<'a, sqlx::Sqlite>,
}

use async_trait::async_trait;

#[async_trait]
impl HcDb for HcDbSqlite {
    async fn begin_tx(&self) -> Result<Box<dyn HcTrx>, HcError> {
        Ok(Box::new(HcDbSqliteTrx {
            tx: self.db.begin().await.map_err(map_sqlx_error)?,
        }))
    }
}

#[async_trait]
impl HcTrx for HcDbSqliteTrx<'_> {
    async fn commit_tx(self: Box<Self>) -> Result<(), HcError> {
        self.tx.commit().await.map_err(map_sqlx_error)
    }
    async fn rollback_tx(self: Box<Self>) -> Result<(), HcError> {
        self.tx.rollback().await.map_err(map_sqlx_error)
    }

    async fn greek_get_synopsis_list(
        &mut self,
        user_id: Option<Uuid>,
    ) -> Result<Vec<(Uuid, chrono::NaiveDateTime, Option<String>, String, String)>, HcError> {
        let res: Vec<(Uuid, chrono::NaiveDateTime, Option<String>, String, String)> = if user_id
            .is_some()
        {
            let query =
                "SELECT id, updated, sname, advisor, selectedverb FROM greeksynopsisresults WHERE user_id = $1 ORDER BY updated DESC;";
            sqlx::query_as(query)
                .bind(user_id)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?
        } else {
            let query =
            "SELECT id, updated, b.user_name, advisor, selectedverb FROM greeksynopsisresults a LEFT JOIN users b ON a.user_id = b.user_id ORDER BY updated DESC;";
            sqlx::query_as(query)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?
        };

        Ok(res)
    }

    async fn greek_get_synopsis_list_all(
        &mut self,
    ) -> Result<Vec<(Uuid, chrono::NaiveDateTime, Option<String>, String, String)>, HcError> {
        let query_all = "SELECT id, updated, b.user_name, advisor, selectedverb FROM greeksynopsisresults a LEFT JOIN users b ON a.user_id = b.user_id ORDER BY updated DESC;";

        let res: Vec<(Uuid, chrono::NaiveDateTime, Option<String>, String, String)> =
            sqlx::query_as(query_all)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;
        Ok(res)
    }

    async fn greek_get_synopsis_result(
        &mut self,
        id: Uuid,
    ) -> Result<GreekSynopsisResult, HcError> {
        let query = r#"SELECT * FROM greeksynopsisresults WHERE id = $1;"#;
        let res: GreekSynopsisResult = sqlx::query_as(query)
            .bind(id)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn greek_insert_synopsis(
        &mut self,
        user_id: Option<sqlx::types::Uuid>,
        info: &SynopsisSaverRequest,
        // ip: &str,
        // agent: &str,
    ) -> Result<(), HcError> {
        let ip = "";
        let agent = "";
        let uuid = sqlx::types::Uuid::new_v4();
        let query = format!("INSERT INTO greeksynopsisresults VALUES ($1, $2, DEFAULT, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, '{}')",
            info.r.join("', '"));
        //println!("aaa: {}", query);
        sqlx::query(&query)
            .bind(uuid)
            .bind(user_id)
            //.bind(accessed)
            .bind(info.sname.clone())
            .bind(info.advisor.clone())
            .bind(info.unit)
            .bind(info.verb)
            .bind(info.pp.clone())
            .bind(info.number)
            .bind(info.person)
            .bind(info.ptcgender.unwrap_or(999))
            .bind(info.ptcnumber.unwrap_or(999))
            .bind(info.ptccase.unwrap_or(999))
            .bind(ip)
            .bind(agent)
            .bind(1)
            .bind("0")
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }
    /*
    pub async fn latin_get_synopsis_list(
        pool: &SqlitePool,
    ) -> Result<Vec<(i64, i64, String, String, String)>, sqlx::Error> {
        let query =
            "SELECT id, updated, sname, advisor, selectedverb FROM latinsynopsisresults ORDER BY updated DESC;";
        let res: Vec<(i64, i64, String, String, String)> =
            sqlx::query_as(query).fetch_all(pool).await?;

        Ok(res)
    }

    pub async fn latin_get_synopsis_result(
        pool: &SqlitePool,
        id: u32,
    ) -> Result<Vec<LatinSynopsisResult>, sqlx::Error> {
        let query = format!(
            "SELECT * FROM latinsynopsisresults WHERE id={} ORDER BY updated DESC;",
            id
        );
        let res: Vec<LatinSynopsisResult> = sqlx::query_as::<_, LatinSynopsisResult>(&query)
            .fetch_all(pool)
            .await?;

        Ok(res)
    }

    // let rec = sqlx::query_as::<_, DefRow>(query)
    //         .bind(word)
    //         .bind(table)
    //         .fetch_one(pool)
    //         .await?;

    pub async fn latin_insert_synopsis(
        pool: &SqlitePool,
        info: &SynopsisSaverRequest,
        accessed: u128,
        ip: &str,
        agent: &str,
    ) -> Result<u32, sqlx::Error> {
        let query = format!("INSERT INTO latinsynopsisresults VALUES (NULL, {}, '{}', '{}', {}, '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', {}, '{}')",
            accessed, info.sname, info.advisor, info.unit, info.verb, info.pp,
            info.number, info.person, info.ptcgender.unwrap_or(999), info.ptcnumber.unwrap_or(999), info.ptccase.unwrap_or(999), ip, agent, 1,
            info.r.join("', '"));
        sqlx::query(&query).execute(pool).await?;

        Ok(1)
    }
    */

    async fn add_to_score(
        &mut self,
        session_id: Uuid,
        user_to_score: &str,
        points: i32,
    ) -> Result<(), HcError> {
        let query = format!(
            "UPDATE sessions SET {user_to_score} = {user_to_score} + $1 WHERE session_id = $2;"
        );
        let _res = sqlx::query(&query)
            .bind(points)
            .bind(session_id)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn get_user_id(&mut self, username: &str) -> Result<UserResult, HcError> {
        let query = "SELECT user_id,user_name,password,email,user_type,timestamp FROM users WHERE user_name = $1 LIMIT 1;";
        let res: UserResult = sqlx::query(query)
            .bind(username)
            .map(|rec: SqliteRow| UserResult {
                user_id: rec.get("user_id"),
                user_name: rec.get("user_name"),
                password: rec.get("password"),
                email: rec.get("email"),
                user_type: rec.get("user_type"),
                timestamp: rec.get("timestamp"),
            })
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn insert_session_tx(
        &mut self,
        user_id: Uuid,
        highest_unit: Option<i16>,
        opponent_id: Option<Uuid>,
        info: &CreateSessionQuery,
        timestamp: i64,
    ) -> Result<Uuid, HcError> {
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
            updated,
            status) VALUES ($1,$2,$3,NULL,$4,$5,$6,$7,$8,0,0,$9,$10,$11,$12,$13,1);"#;
        let _res = sqlx::query(query)
            .bind(uuid)
            .bind(user_id)
            .bind(opponent_id)
            .bind(&info.name)
            .bind(highest_unit)
            .bind(&info.verbs)
            .bind(&info.params)
            .bind(info.max_changes)
            .bind(info.practice_reps_per_verb)
            .bind(info.countdown as i32)
            .bind(info.max_time)
            .bind(timestamp)
            .bind(timestamp)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(uuid)
    }

    async fn get_game_moves(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<Vec<MoveResult>, HcError> {
        let query = "SELECT * \
        FROM moves \
        where session_id = $1 \
        ORDER BY asktimestamp DESC;";

        let res: Vec<MoveResult> = sqlx::query(query)
            .bind(session_id)
            .map(|rec: SqliteRow| MoveResult {
                move_id: rec.get("move_id"),
                session_id: rec.get("session_id"),
                ask_user_id: rec.get("ask_user_id"),
                answer_user_id: rec.get("answer_user_id"),
                verb_id: rec.get("verb_id"),
                person: rec.get("person"),
                number: rec.get("number"),
                tense: rec.get("tense"),
                mood: rec.get("mood"),
                voice: rec.get("voice"),
                answer: rec.get("answer"),
                correct_answer: rec.get("correct_answer"),
                is_correct: rec.get("is_correct"),
                time: rec.get("time"),
                timed_out: rec.get("timed_out"),
                mf_pressed: rec.get("mf_pressed"),
                asktimestamp: rec.get("asktimestamp"),
                answeredtimestamp: rec.get("answeredtimestamp"),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn get_sessions(
        &mut self,
        user_id: sqlx::types::Uuid,
    ) -> Result<Vec<SessionsListQuery>, HcError> {
        //strftime('%Y-%m-%d %H:%M:%S', DATETIME(timestamp, 'unixepoch')) as timestamp,
        //    ORDER BY updated DESC \
        let query = "SELECT session_id AS session_id, name, challenged_user_id AS challenged, b.user_name AS username, challenger_score as myscore, challenged_score as theirscore, \
        a.timestamp as timestamp, a.updated as updated, countdown, max_time, max_changes \
        FROM sessions a LEFT JOIN users b ON a.challenged_user_id = b.user_id \
        where challenger_user_id = $1 \
        UNION SELECT session_id AS session_id, name, challenged_user_id AS challenged, b.user_name AS username, challenged_score as myscore, challenger_score as theirscore, \
        a.timestamp as timestamp, a.updated as updated, countdown, max_time, max_changes \
        FROM sessions a LEFT JOIN users b ON a.challenger_user_id = b.user_id \
        where challenged_user_id  = $2 \
        ORDER BY updated DESC \
        LIMIT 20000;";

        //println!("query: {} {:?}", query, user_id);
        let res: Vec<SessionsListQuery> = sqlx::query(query)
            .bind(user_id)
            .bind(user_id)
            .map(|rec: SqliteRow| {
                SessionsListQuery {
                    session_id: rec.get("session_id"),
                    name: rec.get("name"),
                    challenged: rec.get("challenged"), /*opponent:rec.get("opponent_user_id"),*/
                    opponent_name: rec.get("username"),
                    timestamp: rec.get("timestamp"),
                    updated: rec.get("updated"),
                    myturn: false,
                    move_type: MoveType::Practice,
                    my_score: rec.get("myscore"),
                    their_score: rec.get("theirscore"),
                    countdown: rec.get("countdown"),
                    max_time: rec.get("max_time"),
                    max_changes: rec.get("max_changes"),
                }
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        /*let res2 = match res {
            Ok(e) => e,
            Err(e) => {println!("error: {:?}", e); return Err(e); },
        };*/

        Ok(res)
    }

    async fn get_last_move_tx(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<MoveResult, HcError> {
        let query = "SELECT * \
        FROM moves \
        where session_id = $1 \
        ORDER BY asktimestamp DESC \
        LIMIT $2;";

        //println!("query: {} {:?}", query, user_id);
        let res: MoveResult = sqlx::query(query)
            .bind(session_id)
            .bind(1)
            .map(|rec: SqliteRow| MoveResult {
                move_id: rec.get("move_id"),
                session_id: rec.get("session_id"),
                ask_user_id: rec.get("ask_user_id"),
                answer_user_id: rec.get("answer_user_id"),
                verb_id: rec.get("verb_id"),
                person: rec.get("person"),
                number: rec.get("number"),
                tense: rec.get("tense"),
                mood: rec.get("mood"),
                voice: rec.get("voice"),
                answer: rec.get("answer"),
                correct_answer: rec.get("correct_answer"),
                is_correct: rec.get("is_correct"),
                time: rec.get("time"),
                timed_out: rec.get("timed_out"),
                mf_pressed: rec.get("mf_pressed"),
                asktimestamp: rec.get("asktimestamp"),
                answeredtimestamp: rec.get("answeredtimestamp"),
            })
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn get_last_n_moves(
        &mut self,
        session_id: sqlx::types::Uuid,
        n: u8,
    ) -> Result<Vec<MoveResult>, HcError> {
        let query = "SELECT * \
            FROM moves \
            where session_id = $1 \
            ORDER BY asktimestamp DESC \
            LIMIT $2;";

        //println!("query: {} {:?}", query, user_id);
        let res: Vec<MoveResult> = sqlx::query(query)
            .bind(session_id)
            .bind(n as i32)
            .map(|rec: SqliteRow| MoveResult {
                move_id: rec.get("move_id"),
                session_id: rec.get("session_id"),
                ask_user_id: rec.get("ask_user_id"),
                answer_user_id: rec.get("answer_user_id"),
                verb_id: rec.get("verb_id"),
                person: rec.get("person"),
                number: rec.get("number"),
                tense: rec.get("tense"),
                mood: rec.get("mood"),
                voice: rec.get("voice"),
                answer: rec.get("answer"),
                correct_answer: rec.get("correct_answer"),
                is_correct: rec.get("is_correct"),
                time: rec.get("time"),
                timed_out: rec.get("timed_out"),
                mf_pressed: rec.get("mf_pressed"),
                asktimestamp: rec.get("asktimestamp"),
                answeredtimestamp: rec.get("answeredtimestamp"),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn get_session_tx(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<SessionResult, HcError> {
        let query = "SELECT * \
        FROM sessions \
        where session_id = $1 \
        LIMIT 1;";

        let res: SessionResult = sqlx::query(query)
            .bind(session_id)
            .map(|rec: SqliteRow| SessionResult {
                session_id: rec.get("session_id"),
                challenger_user_id: rec.get("challenger_user_id"),
                challenged_user_id: rec.get("challenged_user_id"),
                current_move: rec.get("current_move"),
                name: rec.get("name"),
                highest_unit: rec.get("highest_unit"),
                custom_verbs: rec.get("custom_verbs"),
                custom_params: rec.get("custom_params"),
                max_changes: rec.get("max_changes"),
                challenger_score: rec.get("challenger_score"),
                challenged_score: rec.get("challenged_score"),
                practice_reps_per_verb: rec.get("practice_reps_per_verb"),
                timestamp: rec.get("timestamp"),
            })
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn get_used_verbs(&mut self, session_id: sqlx::types::Uuid) -> Result<Vec<i32>, HcError> {
        let query = "SELECT verb_id \
        FROM moves \
        where verb_id IS NOT NULL AND session_id = $1;";

        let res: Vec<i32> = sqlx::query(query)
            .bind(session_id)
            .map(|rec: SqliteRow| rec.get("verb_id"))
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res)
    }

    async fn insert_ask_move_tx(
        &mut self,
        user_id: Option<Uuid>,
        info: &AskQuery,
        timestamp: i64,
    ) -> Result<Uuid, HcError> {
        let query = "UPDATE sessions SET updated=$1 WHERE session_id=$2;";
        let _res = sqlx::query(query)
            .bind(timestamp)
            .bind(info.session_id)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

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
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(uuid)
    }

    async fn update_answer_move_tx(
        &mut self,
        info: &AnswerQuery,
        user_id: Uuid,
        correct_answer: &str,
        is_correct: bool,
        mf_pressed: bool,
        timestamp: i64,
    ) -> Result<(), HcError> {
        let m = self.get_last_move_tx(info.session_id).await?;

        let query = "UPDATE sessions SET updated=$1 WHERE session_id=$2;";
        let _res = sqlx::query(query)
            .bind(timestamp)
            .bind(info.session_id)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

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
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn create_user(
        &mut self,
        google_oauth_sub: Option<&str>,
        apple_oauth_sub: Option<&str>,
        username: Option<&str>,
        password: Secret<String>,
        email: &str,
        first_name: &str,
        last_name: &str,
        timestamp: i64,
    ) -> Result<Uuid, HcError> {
        let uuid = sqlx::types::Uuid::new_v4();
        let query = "INSERT INTO users VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0, $9, $10);";
        let _res = sqlx::query(query)
            .bind(uuid)
            .bind(google_oauth_sub)
            .bind(apple_oauth_sub)
            .bind(username)
            .bind(password.expose_secret())
            .bind(email)
            .bind(first_name)
            .bind(last_name)
            .bind(timestamp)
            .bind(timestamp)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(uuid)
    }

    async fn get_credentials(
        &mut self,
        username: &str,
    ) -> Result<Option<(uuid::Uuid, Secret<String>)>, HcError> {
        let row = sqlx::query(
            r#"
            SELECT user_id, password
            FROM users
            WHERE user_name = $1
            "#,
        )
        .bind(username)
        .map(|row: SqliteRow| (row.get("user_id"), Secret::new(row.get("password"))))
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row)
    }

    async fn get_oauth_user(
        &mut self,
        oauth_iss: &str,
        oauth_sub: &str,
    ) -> Result<Option<(uuid::Uuid, Option<String>)>, HcError> {
        // https://accounts.google.com
        let query = format!(
            r#"
            SELECT user_id, user_name
            FROM users
            WHERE {} = $1;"#,
            if oauth_iss == "https://appleid.apple.com" {
                "apple_oauth_sub"
            } else {
                "google_oauth_sub"
            }
        );

        let row = sqlx::query(&query)
            .bind(oauth_sub)
            .map(|row: SqliteRow| (row.get("user_id"), row.get("user_name")))
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(row)
    }

    async fn create_db(&mut self) -> Result<(), HcError> {
        let query = r#"CREATE TABLE IF NOT EXISTS users (
    user_id BLOB PRIMARY KEY NOT NULL,
    google_oauth_sub TEXT,
    apple_oauth_sub TEXT,
    user_name TEXT,
    password TEXT,
    email TEXT,
    first_name TEXT,
    last_name TEXT,
    user_type INT NOT NULL DEFAULT 0,
    timestamp INT NOT NULL DEFAULT 0,
    created INT NOT NULL DEFAULT 0,
    UNIQUE(user_name),
    UNIQUE(google_oauth_sub),
    UNIQUE(apple_oauth_sub)
    ) STRICT;"#;

        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let query = r#"CREATE TABLE IF NOT EXISTS sessions (
    session_id BLOB PRIMARY KEY NOT NULL,
    challenger_user_id BLOB NOT NULL,
    challenged_user_id BLOB DEFAULT NULL,
    current_move BLOB DEFAULT NULL,
    name TEXT DEFAULT NULL,
    highest_unit INT,
    custom_verbs TEXT,
    custom_params TEXT,
    max_changes INT,
    challenger_score INT,
    challenged_score INT,
    practice_reps_per_verb INT,
    countdown INT,
    max_time INT,
    timestamp INT NOT NULL DEFAULT 0,
    updated INT NOT NULL DEFAULT 0,
    status INT NOT NULL DEFAULT 1,
    FOREIGN KEY (challenger_user_id) REFERENCES users(user_id),
    FOREIGN KEY (challenged_user_id) REFERENCES users(user_id)
    ) STRICT;"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let query = r#"CREATE TABLE IF NOT EXISTS moves (
    move_id BLOB PRIMARY KEY NOT NULL,
    session_id BLOB NOT NULL,
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
    ) STRICT;"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let query = "CREATE INDEX IF NOT EXISTS move_session_id_idx ON moves (session_id);";
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let query = r#"CREATE TABLE IF NOT EXISTS greeksynopsisresults (
                id UUID PRIMARY KEY NOT NULL,
                user_id UUID,
                updated timestamp DEFAULT (datetime('now')),
                sname TEXT NOT NULL,
                advisor TEXT NOT NULL,
                sgiday INTEGER NOT NULL,
                selectedverb TEXT NOT NULL,
                pp TEXT NOT NULL,
                verbnumber TEXT NOT NULL,
                verbperson TEXT NOT NULL,
                verbptcgender TEXT NOT NULL,
                verbptcnumber TEXT NOT NULL,
                verbptccase TEXT NOT NULL,
                ip TEXT NOT NULL,
                ua TEXT NOT NULL,
                status INTEGER NOT NULL,
                score TEXT NOT NULL,
                f0 TEXT NOT NULL, a0 TEXT NOT NULL, c0 BOOLEAN NOT NULL,
                f1 TEXT NOT NULL, a1 TEXT NOT NULL, c1 BOOLEAN NOT NULL,
                f2 TEXT NOT NULL, a2 TEXT NOT NULL, c2 BOOLEAN NOT NULL,
                f3 TEXT NOT NULL, a3 TEXT NOT NULL, c3 BOOLEAN NOT NULL,
                f4 TEXT NOT NULL, a4 TEXT NOT NULL, c4 BOOLEAN NOT NULL,
                f5 TEXT NOT NULL, a5 TEXT NOT NULL, c5 BOOLEAN NOT NULL,
                f6 TEXT NOT NULL, a6 TEXT NOT NULL, c6 BOOLEAN NOT NULL,
                f7 TEXT NOT NULL, a7 TEXT NOT NULL, c7 BOOLEAN NOT NULL,
                f8 TEXT NOT NULL, a8 TEXT NOT NULL, c8 BOOLEAN NOT NULL,
                f9 TEXT NOT NULL, a9 TEXT NOT NULL, c9 BOOLEAN NOT NULL,
                f10 TEXT NOT NULL, a10 TEXT NOT NULL, c10 BOOLEAN NOT NULL,
                f11 TEXT NOT NULL, a11 TEXT NOT NULL, c11 BOOLEAN NOT NULL,
                f12 TEXT NOT NULL, a12 TEXT NOT NULL, c12 BOOLEAN NOT NULL,
                f13 TEXT NOT NULL, a13 TEXT NOT NULL, c13 BOOLEAN NOT NULL,
                f14 TEXT NOT NULL, a14 TEXT NOT NULL, c14 BOOLEAN NOT NULL,
                f15 TEXT NOT NULL, a15 TEXT NOT NULL, c15 BOOLEAN NOT NULL,
                f16 TEXT NOT NULL, a16 TEXT NOT NULL, c16 BOOLEAN NOT NULL,
                f17 TEXT NOT NULL, a17 TEXT NOT NULL, c17 BOOLEAN NOT NULL,
                f18 TEXT NOT NULL, a18 TEXT NOT NULL, c18 BOOLEAN NOT NULL,
                f19 TEXT NOT NULL, a19 TEXT NOT NULL, c19 BOOLEAN NOT NULL,
                f20 TEXT NOT NULL, a20 TEXT NOT NULL, c20 BOOLEAN NOT NULL,
                f21 TEXT NOT NULL, a21 TEXT NOT NULL, c21 BOOLEAN NOT NULL,
                f22 TEXT NOT NULL, a22 TEXT NOT NULL, c22 BOOLEAN NOT NULL,
                f23 TEXT NOT NULL, a23 TEXT NOT NULL, c23 BOOLEAN NOT NULL,
                f24 TEXT NOT NULL, a24 TEXT NOT NULL, c24 BOOLEAN NOT NULL,
                f25 TEXT NOT NULL, a25 TEXT NOT NULL, c25 BOOLEAN NOT NULL,
                f26 TEXT NOT NULL, a26 TEXT NOT NULL, c26 BOOLEAN NOT NULL,
                f27 TEXT NOT NULL, a27 TEXT NOT NULL, c27 BOOLEAN NOT NULL,
                f28 TEXT NOT NULL, a28 TEXT NOT NULL, c28 BOOLEAN NOT NULL,
                f29 TEXT NOT NULL, a29 TEXT NOT NULL, c29 BOOLEAN NOT NULL,
                f30 TEXT NOT NULL, a30 TEXT NOT NULL, c30 BOOLEAN NOT NULL,
                f31 TEXT NOT NULL, a31 TEXT NOT NULL, c31 BOOLEAN NOT NULL,
                f32 TEXT NOT NULL, a32 TEXT NOT NULL, c32 BOOLEAN NOT NULL,
                f33 TEXT NOT NULL, a33 TEXT NOT NULL, c33 BOOLEAN NOT NULL,
                f34 TEXT NOT NULL, a34 TEXT NOT NULL, c34 BOOLEAN NOT NULL,
                f35 TEXT NOT NULL, a35 TEXT NOT NULL, c35 BOOLEAN NOT NULL,
                f36 TEXT NOT NULL, a36 TEXT NOT NULL, c36 BOOLEAN NOT NULL,
                f37 TEXT NOT NULL, a37 TEXT NOT NULL, c37 BOOLEAN NOT NULL,
                f38 TEXT NOT NULL, a38 TEXT NOT NULL, c38 BOOLEAN NOT NULL,
                f39 TEXT NOT NULL, a39 TEXT NOT NULL, c39 BOOLEAN NOT NULL,
                f40 TEXT NOT NULL, a40 TEXT NOT NULL, c40 BOOLEAN NOT NULL,
                f41 TEXT NOT NULL, a41 TEXT NOT NULL, c41 BOOLEAN NOT NULL,
                f42 TEXT NOT NULL, a42 TEXT NOT NULL, c42 BOOLEAN NOT NULL,
                f43 TEXT NOT NULL, a43 TEXT NOT NULL, c43 BOOLEAN NOT NULL,
                f44 TEXT NOT NULL, a44 TEXT NOT NULL, c44 BOOLEAN NOT NULL,
                f45 TEXT NOT NULL, a45 TEXT NOT NULL, c45 BOOLEAN NOT NULL,
                f46 TEXT NOT NULL, a46 TEXT NOT NULL, c46 BOOLEAN NOT NULL,
                f47 TEXT NOT NULL, a47 TEXT NOT NULL, c47 BOOLEAN NOT NULL,
                f48 TEXT NOT NULL, a48 TEXT NOT NULL, c48 BOOLEAN NOT NULL,
                f49 TEXT NOT NULL, a49 TEXT NOT NULL, c49 BOOLEAN NOT NULL,
                f50 TEXT NOT NULL, a50 TEXT NOT NULL, c50 BOOLEAN NOT NULL,
                f51 TEXT NOT NULL, a51 TEXT NOT NULL, c51 BOOLEAN NOT NULL,
                f52 TEXT NOT NULL, a52 TEXT NOT NULL, c52 BOOLEAN NOT NULL,
                f53 TEXT NOT NULL, a53 TEXT NOT NULL, c53 BOOLEAN NOT NULL,
                f54 TEXT NOT NULL, a54 TEXT NOT NULL, c54 BOOLEAN NOT NULL,
                f55 TEXT NOT NULL, a55 TEXT NOT NULL, c55 BOOLEAN NOT NULL,
                f56 TEXT NOT NULL, a56 TEXT NOT NULL, c56 BOOLEAN NOT NULL,
                f57 TEXT NOT NULL, a57 TEXT NOT NULL, c57 BOOLEAN NOT NULL,
                f58 TEXT NOT NULL, a58 TEXT NOT NULL, c58 BOOLEAN NOT NULL,
                f59 TEXT NOT NULL, a59 TEXT NOT NULL, c59 BOOLEAN NOT NULL,
                f60 TEXT NOT NULL, a60 TEXT NOT NULL, c60 BOOLEAN NOT NULL,
                f61 TEXT NOT NULL, a61 TEXT NOT NULL, c61 BOOLEAN NOT NULL,
                f62 TEXT NOT NULL, a62 TEXT NOT NULL, c62 BOOLEAN NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(user_id) );"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }
}
