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

//must set feature sqlite or postgres or both
#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("Either feature \"sqlite\" or \"postgres\" must be enabled for this crate.");

use argon2::password_hash::SaltString;
use argon2::Algorithm;
use argon2::Argon2;
use argon2::Params;
use argon2::PasswordHash;
use argon2::PasswordHasher;
use argon2::PasswordVerifier;
use argon2::Version;
use chrono::prelude::*;
use hoplite_verbs_rs::*;
use polytonic_greek::hgk_compare_multiple_forms;
use polytonic_greek::hgk_compare_sqlite; //note: this does not actually depend on sqlite
use rand::prelude::SliceRandom;
use secrecy::ExposeSecret;
use secrecy::Secret;
use std::collections::HashSet;
use tokio::task::spawn_blocking;
use uuid::Uuid;
#[cfg(feature = "postgres")]
pub mod dbpostgres;
#[cfg(feature = "sqlite")]
pub mod dbsqlite;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum HcError {
    Database(String),
    AuthenticationError,
    UnknownError,
}

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

impl std::fmt::Display for HcError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            HcError::Database(s) => write!(fmt, "HcError: database: {}", s),
            HcError::AuthenticationError => write!(fmt, "HcError: authentication error"),
            HcError::UnknownError => write!(fmt, "HcError: unknown error"),
        }
    }
}

pub fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

#[derive(Deserialize, Serialize)]
pub struct GetSessions {
    pub qtype: String,
    pub current_session: Option<Uuid>,
}

#[derive(Deserialize, Serialize)]
pub struct SessionsListResponse {
    pub response_to: String,
    pub sessions: Vec<SessionsListQuery>,
    pub success: bool,
    pub username: Option<String>,
    pub logged_in: bool,
    pub current_session: Option<SessionState>,
}

#[derive(Deserialize, Serialize)]
pub struct UserResult {
    user_id: Uuid,
    user_name: String,
    password: String,
    email: String,
    user_type: i32,
    timestamp: i64,
}

#[derive(Deserialize)]
pub struct AnswerQuery {
    #[allow(dead_code)]
    pub qtype: String,
    pub answer: String,
    pub time: String,
    pub mf_pressed: bool,
    pub timed_out: bool,
    pub session_id: Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct AskQuery {
    pub qtype: String,
    pub session_id: Uuid,
    pub person: i16,
    pub number: i16,
    pub tense: i16,
    pub voice: i16,
    pub mood: i16,
    pub verb: i32,
}

#[derive(Deserialize, Serialize)]
pub struct SessionResult {
    session_id: Uuid,
    challenger_user_id: Uuid,
    challenged_user_id: Option<Uuid>,
    // current_move is not currently used: it is here to hold a move id in the case that
    // I pre-populate db with a sequence of practice moves.
    // this will store the current location in that sequence
    current_move: Option<Uuid>,
    name: Option<String>,
    highest_unit: Option<i16>,
    custom_verbs: Option<String>,
    custom_params: Option<String>,
    max_changes: i16,
    challenger_score: Option<i32>,
    challenged_score: Option<i32>,
    practice_reps_per_verb: Option<i16>,
    timestamp: i64,
}

#[derive(Deserialize, Serialize)]
pub struct GetMoveQuery {
    pub qtype: String,
    pub session_id: Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct GetMovesQuery {
    pub qtype: String,
    pub session_id: Uuid,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct SessionState {
    pub session_id: Uuid,
    pub move_type: MoveType,
    pub myturn: bool,
    pub starting_form: Option<String>,
    pub answer: Option<String>,
    pub is_correct: Option<bool>,
    pub correct_answer: Option<String>,
    pub verb: Option<i32>,
    pub person: Option<i16>,
    pub number: Option<i16>,
    pub tense: Option<i16>,
    pub voice: Option<i16>,
    pub mood: Option<i16>,
    pub person_prev: Option<i16>,
    pub number_prev: Option<i16>,
    pub tense_prev: Option<i16>,
    pub voice_prev: Option<i16>,
    pub mood_prev: Option<i16>,
    pub time: Option<String>, //time for prev answer
    pub response_to: String,
    pub success: bool,
    pub mesg: Option<String>,
    pub verbs: Option<Vec<HCVerbOption>>,
}

#[derive(Deserialize, Serialize)]
pub struct MoveResult {
    move_id: Uuid,
    session_id: Uuid,
    ask_user_id: Option<Uuid>,
    answer_user_id: Option<Uuid>,
    verb_id: Option<i32>,
    person: Option<i16>,
    number: Option<i16>,
    tense: Option<i16>,
    mood: Option<i16>,
    voice: Option<i16>,
    answer: Option<String>,
    correct_answer: Option<String>,
    is_correct: Option<bool>,
    time: Option<String>,
    timed_out: Option<bool>,
    mf_pressed: Option<bool>,
    asktimestamp: i64,
    answeredtimestamp: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct CreateSessionQuery {
    pub qtype: String,
    pub name: Option<String>,
    pub verbs: Option<String>,
    pub units: Option<String>,
    pub params: Option<String>,
    pub highest_unit: Option<i16>,
    pub opponent: String,
    pub countdown: bool,
    pub practice_reps_per_verb: Option<i16>,
    pub max_changes: i16,
    pub max_time: i32,
}

#[derive(PartialEq, Debug, Eq, Deserialize, Serialize)]
pub struct SessionsListQuery {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub challenged: Option<Uuid>, //the one who didn't start the game, or null for practice
    //pub opponent: Option<Uuid>,
    pub opponent_name: Option<String>,
    pub timestamp: i64,
    pub myturn: bool,
    pub move_type: MoveType,
    pub my_score: Option<i32>,
    pub their_score: Option<i32>,
    pub countdown: i32,
    pub max_time: i32,
    pub max_changes: i16,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct HCVerbOption {
    pub id: i32,
    pub verb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MoveType {
    Practice,
    FirstMoveMyTurn,
    FirstMoveTheirTurn,

    AnswerMyTurn,
    AskTheirTurn,
    AskMyTurn,
    AnswerTheirTurn,

    GameOver,
}

use async_trait::async_trait;
#[async_trait]
pub trait HcDb {
    async fn begin_tx(&self) -> Result<Box<dyn HcTrx>, HcError>;
}

#[async_trait]
pub trait HcTrx {
    async fn commit_tx(self: Box<Self>) -> Result<(), HcError>;
    async fn rollback_tx(self: Box<Self>) -> Result<(), HcError>;

    async fn add_to_score(
        &mut self,
        session_id: Uuid,
        user_to_score: &str,
        points: i32,
    ) -> Result<(), HcError>;

    async fn get_user_id(&mut self, username: &str) -> Result<UserResult, HcError>;

    async fn insert_session_tx(
        &mut self,
        user_id: Uuid,
        highest_unit: Option<i16>,
        opponent_id: Option<Uuid>,
        info: &CreateSessionQuery,
        timestamp: i64,
    ) -> Result<Uuid, HcError>;

    async fn get_game_moves(&mut self, session_id: Uuid) -> Result<Vec<MoveResult>, HcError>;

    async fn get_sessions(&mut self, user_id: Uuid) -> Result<Vec<SessionsListQuery>, HcError>;

    async fn get_last_move_tx(&mut self, session_id: Uuid) -> Result<MoveResult, HcError>;

    async fn get_last_n_moves(
        &mut self,
        session_id: Uuid,
        n: u8,
    ) -> Result<Vec<MoveResult>, HcError>;

    async fn get_session_tx(&mut self, session_id: Uuid) -> Result<SessionResult, HcError>;

    async fn get_used_verbs(&mut self, session_id: Uuid) -> Result<Vec<i32>, HcError>;

    async fn insert_ask_move_tx(
        &mut self,
        user_id: Option<Uuid>,
        info: &AskQuery,
        timestamp: i64,
    ) -> Result<Uuid, HcError>;

    async fn update_answer_move_tx(
        &mut self,
        info: &AnswerQuery,
        user_id: Uuid,
        correct_answer: &str,
        is_correct: bool,
        mf_pressed: bool,
        timestamp: i64,
    ) -> Result<(), HcError>;

    async fn create_user(
        &mut self,
        oauth_iss: Option<String>,
        oauth_sub: Option<String>,
        username: Option<&str>,
        password: Secret<String>,
        email: &str,
        timestamp: i64,
    ) -> Result<Uuid, HcError>;

    async fn get_credentials(
        &mut self,
        username: &str,
    ) -> Result<Option<(uuid::Uuid, Secret<String>)>, HcError>;

    async fn get_oauth_user(
        &mut self,
        oauth_iss: &str,
        oauth_sub: &str,
    ) -> Result<Option<(uuid::Uuid, Option<String>)>, HcError>;

    async fn create_db(&mut self) -> Result<(), HcError>;
}

pub async fn hc_create_db(db: &dyn HcDb) -> Result<(), HcError> {
    let mut tx = db.begin_tx().await?;
    tx.create_db().await?;
    tx.commit_tx().await?;
    Ok(())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, HcError> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt);

    match password_hash {
        Ok(p) => Ok(Secret::new(p.to_string())),
        Err(_e) => Err(HcError::AuthenticationError),
    }
}

pub async fn hc_validate_credentials(
    db: &dyn HcDb,
    credentials: Credentials,
) -> Result<uuid::Uuid, HcError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    let mut tx = db.begin_tx().await?;
    if let Some((stored_user_id, stored_password_hash)) =
        tx.get_credentials(&credentials.username).await?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }
    tx.commit_tx().await?;

    spawn_blocking(move || {
        verify_password_hash(expected_password_hash, &credentials.password) //this will error and return if password does not match
    })
    .await
    .map_err(|_| HcError::AuthenticationError)??;
    match user_id {
        Some(id) => Ok(id),
        _ => Err(HcError::AuthenticationError),
    }
}

fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: &Secret<String>,
) -> Result<(), HcError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret());
    match expected_password_hash {
        Ok(p) => Argon2::default()
            .verify_password(password_candidate.expose_secret().as_bytes(), &p)
            .map_err(|_| HcError::AuthenticationError),
        Err(_) => Err(HcError::AuthenticationError),
    }
}

pub async fn hc_create_user(
    db: &dyn HcDb,
    username: &str,
    password: &str,
    email: &str,
    timestamp: i64,
) -> Result<Uuid, HcError> {
    if username.len() < 2
        || username.len() > 30
        || password.len() < 8
        || password.len() > 60
        || email.len() < 6
        || email.len() > 120
    {
        return Err(HcError::UnknownError);
    }

    let secret_password = Secret::new(password.to_string());

    let password_hash = spawn_blocking(move || compute_password_hash(secret_password))
        .await
        .map_err(|_| HcError::AuthenticationError)??;

    let mut tx = db.begin_tx().await?;
    let user_id = tx
        .create_user(None, None, Some(username), password_hash, email, timestamp)
        .await?;
    tx.commit_tx().await?;
    Ok(user_id)
}

pub async fn hc_create_oauth_user(
    db: &dyn HcDb,
    oauth_iss: String,
    oauth_sub: String,
    _first_name: &str,
    _last_name: &str,
    email: &str,
    timestamp: i64,
) -> Result<(Uuid, Option<String>), HcError> {
    let mut tx = db.begin_tx().await?;

    let existing_user = tx.get_oauth_user(&oauth_iss, &oauth_sub).await?;

    match existing_user {
        Some((existing_user_id, existing_user_name)) => {
            tx.commit_tx().await?;
            Ok((existing_user_id, existing_user_name))
        }
        None => {
            //let user_name = format!("{}{}", first_name, last_name);
            let user_id = tx
                .create_user(
                    Some(oauth_iss),
                    Some(oauth_sub),
                    None,
                    Secret::new(String::from("")),
                    email,
                    timestamp,
                )
                .await?;
            tx.commit_tx().await?;
            Ok((user_id, None))
        }
    }
}

async fn hc_get_session_state_tx(
    tx: &mut Box<dyn HcTrx>,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<SessionState, HcError> {
    let res = tx.get_session_tx(session_id).await?;
    let m = tx.get_last_n_moves(session_id, 2).await?;

    let first = if !m.is_empty() { Some(&m[0]) } else { None };
    let (myturn, move_type) = hc_move_get_type(first, user_id, res.challenged_user_id);

    let asking_new_verb: bool = move_type == MoveType::FirstMoveMyTurn; //don't old show desc when *asking* a new verb
    let answering_new_verb = m.len() > 1 && m[0].verb_id != m[1].verb_id; //don't show old desc when *answering* a new verb

    let r = SessionState {
        session_id,
        move_type,
        myturn,
        starting_form: if m.len() == 2 && m[0].verb_id == m[1].verb_id {
            m[1].correct_answer.clone()
        } else {
            None
        },
        answer: if !m.is_empty() {
            m[0].answer.clone()
        } else {
            None
        },
        is_correct: if !m.is_empty() && m[0].is_correct.is_some() {
            Some(m[0].is_correct.unwrap())
        } else {
            None
        },
        correct_answer: if !m.is_empty() {
            m[0].correct_answer.clone()
        } else {
            None
        },
        verb: if !m.is_empty() { m[0].verb_id } else { None },
        person: if !m.is_empty() { m[0].person } else { None },
        number: if !m.is_empty() { m[0].number } else { None },
        tense: if !m.is_empty() { m[0].tense } else { None },
        voice: if !m.is_empty() { m[0].voice } else { None },
        mood: if !m.is_empty() { m[0].mood } else { None },
        person_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb {
            m[1].person
        } else {
            None
        },
        number_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb {
            m[1].number
        } else {
            None
        },
        tense_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb {
            m[1].tense
        } else {
            None
        },
        voice_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb {
            m[1].voice
        } else {
            None
        },
        mood_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb {
            m[1].mood
        } else {
            None
        },
        time: if !m.is_empty() {
            m[0].time.clone()
        } else {
            None
        },
        response_to: String::from(""),
        success: true,
        mesg: None,
        verbs: None,
    };

    Ok(r)
}

pub async fn hc_ask(
    db: &dyn HcDb,
    user_id: Uuid,
    info: &AskQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, HcError> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    //todo check that user_id == challenger_user_id if this is first move
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(HcError::UnknownError);
    }

    //prevent out-of-sequence asks
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id)
                || m.answer_user_id != Some(user_id)
                || m.is_correct.is_none()
            {
                return Err(HcError::UnknownError); //same user cannot ask twice in a row and ask user must be same as previous answer user and previous answer must be marked correct or incorrect
            } else {
                Ok(m)
            }
        }
        Err(m) => Err(m), //this is first move, nothing to check
    };

    //be sure this asktimestamp is at least one greater than previous, if there was a previous one
    // let new_time_stamp = if m.is_ok() && timestamp <= m.as_ref().unwrap().asktimestamp {
    //     m.unwrap().asktimestamp + 1
    // } else {
    //     timestamp
    // };
    let new_time_stamp = match m {
        Ok(m) => {
            if timestamp <= m.asktimestamp {
                m.asktimestamp + 1
            } else {
                timestamp
            }
        }
        _ => timestamp,
    };

    //get move seq and add one?

    let _ = tx
        .insert_ask_move_tx(Some(user_id), info, new_time_stamp)
        .await?;

    let mut res = hc_get_session_state_tx(&mut tx, user_id, info.session_id).await?;

    if res.starting_form.is_none()
        && res.verb.is_some()
        && (res.verb.unwrap() as usize) < verbs.len()
    {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = String::from("ask");
    res.success = true;
    res.mesg = None;
    res.verbs = None;

    tx.commit_tx().await?;

    Ok(res)
}

pub async fn hc_answer(
    db: &dyn HcDb,
    user_id: Uuid,
    info: &AnswerQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, HcError> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(HcError::UnknownError);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some() {
                return Err(HcError::UnknownError); //same user cannot answer question they asked and previous question must not already be answered
            } else {
                m
            }
        }
        Err(_) => {
            return Err(HcError::UnknownError);
        } //this is first move, nothing to answer
    };

    //test answer to get correct_answer and is_correct
    //let luw = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
    //let luwverb = Arc::new(HcGreekVerb::from_string(1, luw, REGULAR).unwrap());
    let idx = if m.verb_id.is_some() && (m.verb_id.unwrap() as usize) < verbs.len() {
        m.verb_id.unwrap() as usize
    } else {
        0
    };
    let prev_form = HcGreekVerbForm {
        verb: verbs[idx].clone(),
        person: HcPerson::from_i16(m.person.unwrap()),
        number: HcNumber::from_i16(m.number.unwrap()),
        tense: HcTense::from_i16(m.tense.unwrap()),
        voice: HcVoice::from_i16(m.voice.unwrap()),
        mood: HcMood::from_i16(m.mood.unwrap()),
        gender: None,
        case: None,
    };

    let correct_answer_result = prev_form.get_form(false);
    let correct_answer = match correct_answer_result {
        Ok(a) => a.last().unwrap().form.replace(" /", ","),
        Err(_) => String::from("—"),
    };

    let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));

    tx.update_answer_move_tx(
        info,
        user_id,
        &correct_answer,
        is_correct,
        info.mf_pressed,
        timestamp,
    )
    .await?;

    //if practice session, ask the next here
    if s.challenged_user_id.is_none() {
        hc_ask_practice(&mut tx, prev_form, &s, timestamp, m.asktimestamp, verbs).await?;
    } else {
        //add to other player's score if not practice and not correct
        if !is_correct {
            let user_to_score = if s.challenger_user_id == user_id {
                "challenged_score"
            } else {
                "challenger_score"
            };
            let points = 1;
            tx.add_to_score(info.session_id, user_to_score, points)
                .await?;
        }
    }

    let mut res = hc_get_session_state_tx(&mut tx, user_id, info.session_id).await?;
    //starting_form is 1st pp if new verb
    if res.starting_form.is_none()
        && res.verb.is_some()
        && (res.verb.unwrap() as usize) < verbs.len()
    {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    //if practice session, add in is_correct and correct_answer back into session state here
    if s.challenged_user_id.is_none() {
        res.is_correct = Some(is_correct);
        res.correct_answer = Some(correct_answer);
        res.response_to = String::from("answerresponsepractice");
    } else {
        res.response_to = String::from("answerresponse");
    }

    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct {
        Some(
            hc_get_available_verbs(&mut tx, user_id, info.session_id, s.highest_unit, verbs)
                .await
                .unwrap(),
        )
    } else {
        None
    };

    tx.commit_tx().await?;

    Ok(res)
}

pub async fn hc_mf_pressed(
    db: &dyn HcDb,
    user_id: Uuid,
    info: &AnswerQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, HcError> {
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(HcError::UnknownError);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some() {
                return Err(HcError::UnknownError); //same user cannot answer question they asked and previous question must not already be answered
            } else {
                m
            }
        }
        Err(_) => {
            return Err(HcError::UnknownError);
        } //this is first move, nothing to answer
    };

    //test answer to get correct_answer and is_correct
    //let luw = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
    //let luwverb = Arc::new(HcGreekVerb::from_string(1, luw, REGULAR).unwrap());
    let idx = if m.verb_id.is_some() && (m.verb_id.unwrap() as usize) < verbs.len() {
        m.verb_id.unwrap() as usize
    } else {
        0
    };
    let prev_form = HcGreekVerbForm {
        verb: verbs[idx].clone(),
        person: HcPerson::from_i16(m.person.unwrap()),
        number: HcNumber::from_i16(m.number.unwrap()),
        tense: HcTense::from_i16(m.tense.unwrap()),
        voice: HcVoice::from_i16(m.voice.unwrap()),
        mood: HcMood::from_i16(m.mood.unwrap()),
        gender: None,
        case: None,
    };

    let correct_answer = prev_form
        .get_form(false)
        .unwrap()
        .last()
        .unwrap()
        .form
        .replace(" /", ",");

    if correct_answer.contains(',') {
        let mut res = hc_get_session_state_tx(&mut tx, user_id, info.session_id).await?;
        if res.starting_form.is_none()
            && res.verb.is_some()
            && (res.verb.unwrap() as usize) < verbs.len()
        {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = String::from("mfpressedresponse");
        res.success = true;
        res.mesg = Some(String::from("verb *does* have multiple forms"));
        res.verbs = None;

        tx.rollback_tx().await?;

        Ok(res)
    } else {
        let is_correct = false;
        tx.update_answer_move_tx(info, user_id, &correct_answer, is_correct, true, timestamp)
            .await?;

        //if practice session, ask the next here
        if s.challenged_user_id.is_none() {
            hc_ask_practice(&mut tx, prev_form, &s, timestamp, m.asktimestamp, verbs).await?;
        } else {
            //add to other player's score if not practice and not correct
            if !is_correct {
                let user_to_score = if s.challenger_user_id == user_id {
                    "challenged_score"
                } else {
                    "challenger_score"
                };
                let points = 1;
                tx.add_to_score(info.session_id, user_to_score, points)
                    .await?;
            }
        }

        let mut res = hc_get_session_state_tx(&mut tx, user_id, info.session_id).await?;
        //starting_form is 1st pp if new verb
        if res.starting_form.is_none()
            && res.verb.is_some()
            && (res.verb.unwrap() as usize) < verbs.len()
        {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }

        //if practice session, add in is_correct and correct_answer back into session state here
        if s.challenged_user_id.is_none() {
            res.is_correct = Some(is_correct);
            res.correct_answer = Some(correct_answer);
            res.response_to = String::from("mfpressedresponsepractice");
        } else {
            res.response_to = String::from("mfpressedresponse");
        }

        res.success = true;
        res.mesg = Some(String::from("verb does not have multiple forms"));
        res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct {
            Some(
                hc_get_available_verbs(&mut tx, user_id, info.session_id, s.highest_unit, verbs)
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        tx.commit_tx().await?;

        Ok(res)
    }
}

fn hc_get_available_verbs_practice(
    available_verbs_str: &Option<String>,
    used_verbs: &Vec<i32>,
    reps: usize,
) -> Vec<i32> {
    let available_verbs: HashSet<i32> = match available_verbs_str {
        Some(v) => v
            .split(',')
            .filter_map(|num| num.parse::<i32>().ok())
            .collect::<HashSet<i32>>(),
        _ => return vec![], //(1..127).filter(|&i: &i32| i != 78 && i != 79 && i != 122 && i != 127 ).collect::<HashSet<i32>>(),
    };

    if available_verbs.len() == 1 {
        return vec![*available_verbs.iter().next().unwrap()];
    }

    let remainder = used_verbs.len() % (available_verbs.len() * reps);
    //println!("remainder: {:?}", remainder);

    let mut filter = used_verbs[0..remainder]
        .iter()
        .cloned()
        .collect::<HashSet<i32>>();

    if remainder == 0 && !used_verbs.is_empty() {
        filter.insert(used_verbs[0]); //if all verbs have been used, do not allow next verb to be last one used
    }

    //println!("avail: {:?}, used: {:?}", available_verbs, used_verbs);
    available_verbs
        .difference(&filter)
        .cloned()
        .collect::<Vec<i32>>()
}

fn hc_change_verbs(verb_history: &Vec<i32>, reps: usize) -> bool {
    let len = verb_history.len();
    len == 0 || (len >= reps && verb_history[0] == verb_history[reps - 1])
}

async fn hc_ask_practice(
    tx: &mut Box<dyn HcTrx>,
    mut prev_form: HcGreekVerbForm,
    session: &SessionResult,
    timestamp: i64,
    asktimestamp: i64,
    verbs: &[Arc<HcGreekVerb>],
) -> Result<(), HcError> {
    let verb_params = VerbParameters::from_option(session.custom_params.clone());

    let max_per_verb = match session.practice_reps_per_verb {
        Some(r) => r,
        _ => 4,
    };

    let moves = tx.get_last_n_moves(session.session_id, 100).await?;
    let last_verb_ids = moves
        .iter()
        .filter_map(|m| m.verb_id.map(|_| m.verb_id.unwrap()))
        .collect::<Vec<i32>>();

    let verb_id: i32 = if hc_change_verbs(&last_verb_ids, max_per_verb as usize) {
        let verbs = hc_get_available_verbs_practice(
            &session.custom_verbs,
            &last_verb_ids,
            max_per_verb as usize,
        );
        let new_verb_id = verbs.choose(&mut rand::thread_rng());

        *new_verb_id.unwrap()
    } else {
        prev_form.verb.id as i32
    };

    prev_form.verb = verbs[verb_id as usize].clone();
    let pf = prev_form.random_form(
        session.max_changes.try_into().unwrap(),
        session.highest_unit,
        &verb_params,
    );

    //let vf = pf.get_form(false);
    //println!("form: {}",vf.unwrap().last().unwrap().form);

    //be sure this asktimestamp is at least one greater than previous one
    let new_time_stamp = if timestamp > asktimestamp {
        timestamp
    } else {
        asktimestamp + 1
    };
    //ask
    let aq = AskQuery {
        qtype: String::from("ask"),
        session_id: session.session_id,
        person: pf.person.to_i16(),
        number: pf.number.to_i16(),
        tense: pf.tense.to_i16(),
        voice: pf.voice.to_i16(),
        mood: pf.mood.to_i16(),
        verb: verb_id,
    };
    let _ = tx.insert_ask_move_tx(None, &aq, new_time_stamp).await?;
    Ok(())
}

pub async fn hc_get_sessions(
    db: &dyn HcDb,
    user_id: Uuid,
    verbs: &Vec<Arc<HcGreekVerb>>,
    username: Option<String>,
    info: &GetSessions,
) -> Result<SessionsListResponse, HcError> {
    let mut tx = db.begin_tx().await?;
    let current_session = match info.current_session {
        Some(r) => Some(hc_get_move_tr(&mut tx, user_id, false, r, verbs).await?),
        _ => None,
    };

    let res = Ok(SessionsListResponse {
        response_to: String::from("getsessions"),
        sessions: hc_get_sessions_tr(&mut tx, user_id).await?,
        success: true,
        username,
        logged_in: true,
        current_session,
    });
    tx.commit_tx().await?;
    res
}

pub async fn hc_get_move(
    db: &dyn HcDb,
    user_id: Uuid,
    opponent_id: bool,
    session_id: Uuid,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, HcError> {
    let mut tx = db.begin_tx().await?;
    let res = hc_get_move_tr(&mut tx, user_id, opponent_id, session_id, verbs).await?;
    tx.commit_tx().await?;
    Ok(res)
}

//opponent_id gets move status for opponent rather than user_id when true:
//we handle the case of s.challenged_user_id.is_none() here, but opponent_id should always be false for practice games
pub async fn hc_get_move_tr(
    tx: &mut Box<dyn HcTrx>,
    user_id: Uuid,
    opponent_id: bool,
    session_id: Uuid,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, HcError> {
    let s = tx.get_session_tx(session_id).await?;

    let real_user_id = if !opponent_id || s.challenged_user_id.is_none() {
        user_id
    } else if user_id == s.challenger_user_id {
        s.challenged_user_id.unwrap()
    } else {
        s.challenger_user_id
    };

    let mut res = hc_get_session_state_tx(tx, real_user_id, session_id).await?;

    //set starting_form to 1st pp of verb if verb is set, but starting form is None (i.e. we just changed verbs)
    if res.starting_form.is_none()
        && res.verb.is_some()
        && (res.verb.unwrap() as usize) < verbs.len()
    {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = String::from("getmoves");
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn {
        Some(
            hc_get_available_verbs(tx, real_user_id, session_id, s.highest_unit, verbs)
                .await
                .unwrap(),
        )
    } else {
        None
    };

    Ok(res)
}

fn hc_move_get_type(
    s: Option<&MoveResult>,
    user_id: Uuid,
    challenged_id: Option<Uuid>,
) -> (bool, MoveType) {
    let myturn: bool;
    let move_type: MoveType;

    let change_verb_on_incorrect = true;

    match s {
        Some(s) => {
            #[allow(clippy::collapsible_else_if)]
            if challenged_id.is_none() {
                myturn = true;
                move_type = MoveType::Practice; //practice, my turn always
            } else if s.ask_user_id == Some(user_id) {
                if s.answer_user_id.is_some() {
                    //xxxanswered, my turn to ask | I asked, they answered, their turn to ask (waiting for them to ask)
                    myturn = false;
                    move_type = MoveType::AskTheirTurn;
                } else {
                    myturn = false; //unanswered, their turn to answer
                    move_type = MoveType::AnswerTheirTurn;
                }
            } else {
                if s.answer_user_id.is_some() {
                    //xxxanswered, their turn to ask | they asked, I answered, my turn to ask
                    myturn = true;

                    if change_verb_on_incorrect && s.is_correct.is_some() && !s.is_correct.unwrap()
                    {
                        move_type = MoveType::FirstMoveMyTurn; //user must ask a new verb because answered incorrectly
                    } else {
                        move_type = MoveType::AskMyTurn;
                    }
                } else {
                    myturn = true; //unanswered, my turn to answer
                    move_type = MoveType::AnswerMyTurn;
                }
            }
        }
        None => {
            if let Some(cid) = challenged_id {
                if cid == user_id {
                    myturn = false;
                    move_type = MoveType::FirstMoveTheirTurn; //no moves yet, their turn to ask
                } else {
                    myturn = true;
                    move_type = MoveType::FirstMoveMyTurn; //no moves yet, my turn to ask
                }
            } else {
                myturn = true;
                move_type = MoveType::Practice; //practice, my turn always (no moves yet)
            }
        }
    }
    (myturn, move_type)
}

pub async fn hc_get_sessions_tr(
    tx: &mut Box<dyn HcTrx>,
    user_id: Uuid,
) -> Result<Vec<SessionsListQuery>, HcError> {
    let mut res = tx.get_sessions(user_id).await?;

    for r in &mut res {
        if let Ok(m) = tx.get_last_move_tx(r.session_id).await {
            (r.myturn, r.move_type) = hc_move_get_type(Some(&m), user_id, r.challenged);
        } else {
            (r.myturn, r.move_type) = hc_move_get_type(None, user_id, r.challenged);
        }
        //these were needed to tell whose turn, but no need to send these out to client
        r.challenged = None;
        //r.opponent = None;
    }
    Ok(res)
}

fn hc_get_verbs_by_unit(units: &str, verbs: &[Arc<HcGreekVerb>]) -> Option<String> {
    let u: Vec<u32> = units
        .split(',')
        .map(|x| x.parse::<u32>().unwrap())
        .collect();
    let mut verb_ids: Vec<u32> = vec![];
    for unit in u {
        for v in verbs {
            if v.hq_unit == unit {
                verb_ids.push(v.id);
            }
        }
    }
    if !verb_ids.is_empty() {
        Some(verb_ids.iter().map(|&i| i.to_string() + ",").collect())
    } else {
        None
    }
}

pub async fn hc_get_game_moves(
    db: &dyn HcDb,
    info: &GetMovesQuery,
) -> Result<Vec<MoveResult>, HcError> {
    let mut tx = db.begin_tx().await?;
    let res = tx.get_game_moves(info.session_id).await?;

    Ok(res)
}

pub async fn hc_insert_session(
    db: &dyn HcDb,
    user_id: Uuid,
    info: &mut CreateSessionQuery,
    verbs: &[Arc<HcGreekVerb>],
    timestamp: i64,
) -> Result<Uuid, HcError> {
    let mut tx = db.begin_tx().await?;

    let opponent_user_id: Option<Uuid>;
    if !info.opponent.is_empty() {
        let o = tx.get_user_id(&info.opponent).await?; //we want to return an error if len of info.opponent > 0 and not found, else it is practice game
        opponent_user_id = Some(o.user_id);
    } else {
        opponent_user_id = None;
    }

    //failed to find opponent or opponent is self
    if opponent_user_id.is_some() && opponent_user_id.unwrap() == user_id {
        return Err(HcError::UnknownError); //todo oops
    }

    // if custom verbs are set, use them, else change units into verbs
    if info.verbs.is_none() {
        match &info.units {
            Some(u) => info.verbs = hc_get_verbs_by_unit(u, verbs),
            None => return Err(HcError::UnknownError),
        }
    }

    // if still no verbs, abort
    if info.verbs.is_none() {
        return Err(HcError::UnknownError);
    }

    let highest_unit = match info.highest_unit {
        Some(r) => {
            if r < 2 {
                Some(2)
            } else if r > 20 {
                Some(20)
            } else {
                Some(r)
            }
        }
        _ => None,
    };

    //be sure max_time is always zero if countdown is false (i.e. elapsed timer)
    info.max_time = if info.countdown { info.max_time } else { 0 };

    match tx
        .insert_session_tx(user_id, highest_unit, opponent_user_id, info, timestamp)
        .await
    {
        Ok(session_uuid) => {
            //for practice sessions we should do the ask here
            if opponent_user_id.is_none() {
                let prev_form = HcGreekVerbForm {
                    verb: verbs[1].clone(),
                    person: HcPerson::First,
                    number: HcNumber::Singular,
                    tense: HcTense::Present,
                    voice: HcVoice::Active,
                    mood: HcMood::Indicative,
                    gender: None,
                    case: None,
                };

                let sesh = SessionResult {
                    session_id: session_uuid,
                    challenger_user_id: user_id,
                    challenged_user_id: None,
                    current_move: None,
                    name: None,
                    highest_unit,
                    custom_verbs: info.verbs.clone(),
                    custom_params: info.params.clone(),
                    max_changes: info.max_changes,
                    challenger_score: None,
                    challenged_score: None,
                    practice_reps_per_verb: info.practice_reps_per_verb,
                    timestamp,
                };
                hc_ask_practice(&mut tx, prev_form, &sesh, timestamp, 0, verbs).await?;
            }
            tx.commit_tx().await?;
            Ok(session_uuid)
        }
        Err(e) => Err(e),
    }
}

async fn hc_get_available_verbs(
    tx: &mut Box<dyn HcTrx>,
    _user_id: Uuid,
    session_id: Uuid,
    top_unit: Option<i16>,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<Vec<HCVerbOption>, HcError> {
    let mut res_verbs: Vec<HCVerbOption> = vec![];

    let used_verbs = tx.get_used_verbs(session_id).await?;
    //println!("used_verbs: {:?}", used_verbs);
    for v in verbs {
        if v.id == 0 {
            continue;
        }
        if (top_unit.is_none() || v.hq_unit <= top_unit.unwrap() as u32)
            && !used_verbs.contains(&(v.id as i32))
        {
            //&& verb_id_not_used()
            let newv = HCVerbOption {
                id: v.id as i32,
                verb: if v.pps[0] == "—" {
                    format!("—, {}", v.pps[1])
                } else {
                    v.pps[0].clone()
                },
            };
            res_verbs.push(newv);
        }
    }

    res_verbs.sort_by(|a, b| hgk_compare_sqlite(&a.verb, &b.verb));
    Ok(res_verbs)
}

static PPS: &str = include_str!("pp.txt");

pub fn hc_load_verbs(_path: &str) -> Vec<Arc<HcGreekVerb>> {
    let mut verbs = vec![];
    // if let Ok(pp_file) = File::open(path) {
    //     let pp_reader = BufReader::new(pp_file);
    //     for (idx, pp_line) in pp_reader.lines().enumerate() {
    //         if let Ok(line) = pp_line {
    //             if !line.starts_with('#') { //skip commented lines
    //                 verbs.push(Arc::new(HcGreekVerb::from_string_with_properties(idx as u32, &line).unwrap()));
    //             }
    //         }
    //     }
    // }
    verbs.push(Arc::new(
        HcGreekVerb::from_string_with_properties(0, "blank,blank,blank,blank,blank,blank % 0")
            .unwrap(),
    )); //so paideuw is at index 1
    let pp_lines = PPS.split('\n');
    for (idx, line) in pp_lines.enumerate() {
        if !line.starts_with('#') && !line.is_empty() {
            //skip commented lines
            //println!("line: {} {}", idx, line);
            verbs.push(Arc::new(
                HcGreekVerb::from_string_with_properties(idx as u32 + 1, line).unwrap(),
            ));
        }
    }

    verbs
}

/*
pub async fn hc_get_verbs(db: &HcDb, _user_id:Uuid, session_id:Uuid, top_unit:Option<i16>, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<Vec<HCVerbOption>, HcError> {
    let mut res_verbs:Vec<HCVerbOption> = vec![];

    let used_verbs = db.get_used_verbs(session_id).await?;

    for v in verbs {
        if top_unit.is_none() || v.hq_unit <= top_unit.unwrap() as u32 && !used_verbs.contains(&(v.id as i32))  { //&& verb_id_not_used()
            let newv = HCVerbOption {
                id: v.id as i32,
                verb: if v.pps[0] == "—" { format!("—, {}", v.pps[1]) } else { v.pps[0].clone() },
            };
            res_verbs.push(newv);
        }
    }

    res_verbs.sort_by(|a,b| hgk_compare_sqlite(&a.verb,&b.verb));
    Ok(res_verbs)
}
*/

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Executor;
    use tokio::sync::OnceCell;
    static ONCE: OnceCell<()> = OnceCell::const_new();

    //if both postgres AND sqlite are set, test with postgres
    //at least one or the other MUST be set, or both
    #[cfg(feature = "postgres")]
    use dbpostgres::HcDbPostgres;
    #[cfg(feature = "postgres")]
    use sqlx::postgres::PgPoolOptions;

    #[cfg(not(feature = "postgres"))]
    use crate::dbsqlite::HcDbSqlite;
    #[cfg(not(feature = "postgres"))]
    use sqlx::sqlite::SqliteConnectOptions;
    #[cfg(not(feature = "postgres"))]
    use sqlx::SqlitePool;
    #[cfg(not(feature = "postgres"))]
    use std::str::FromStr;

    #[cfg(feature = "postgres")]
    async fn get_db() -> HcDbPostgres {
        HcDbPostgres {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect("postgres://jwm:1234@localhost/hctest")
                .await
                .expect("Could not connect to db."),
        }
    }

    #[cfg(not(feature = "postgres"))]
    async fn get_db() -> HcDbSqlite {
        let db_path = ":memory:";
        let options = SqliteConnectOptions::from_str(db_path)
            .expect("Could not connect to db.")
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .read_only(false)
            .collation("PolytonicGreek", |l, r| {
                l.to_lowercase().cmp(&r.to_lowercase())
            });
        let db = HcDbSqlite {
            db: SqlitePool::connect_with(options)
                .await
                .expect("Could not connect to db."),
        };

        //need to call these here, setup_test_db() doesn't work for sqlite
        let _ = db.db.execute("DROP TABLE IF EXISTS moves;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS sessions;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS users;").await;

        let mut tx = db.begin_tx().await.unwrap();
        let res = tx.create_db().await;
        if res.is_err() {
            println!("error: {res:?}");
        }
        tx.commit_tx().await.unwrap();
        db
    }

    async fn setup_test_db() {
        let db = get_db().await;

        let _ = db.db.execute("DROP TABLE IF EXISTS moves;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS sessions;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS users;").await;

        let mut tx = db.begin_tx().await.unwrap();
        let res = tx.create_db().await;
        if res.is_err() {
            println!("error: {res:?}");
        }
        tx.commit_tx().await.unwrap();
    }

    pub async fn initialize_db_once() {
        ONCE.get_or_init(setup_test_db).await;
    }

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

    #[tokio::test]
    async fn test_oauth_json() {
        let mut first_name = String::from("");
        let mut last_name = String::from("");
        let mut email = String::from("");

        let user = Some("{\"name\":{\"firstName\":\"First\",\"lastName\":\"Last\"},\"email\":\"abc@gmail.com\"}");

        if let Some(user) = user {
            if let Ok(apple_oauth_user) = serde_json::from_str::<AppleOAuthUser>(user) {
                first_name = apple_oauth_user.name.first_name.unwrap_or(String::from(""));
                last_name = apple_oauth_user.name.last_name.unwrap_or(String::from(""));
                email = apple_oauth_user.email.unwrap_or(String::from(""));
            }
        }
        assert_eq!(first_name, "First");
        assert_eq!(last_name, "Last");
        assert_eq!(email, "abc@gmail.com");

        let user = Some("{\"name\":{\"lastName\":null},\"email\":\"abc@gmail.com\"}");

        if let Some(user) = user {
            if let Ok(apple_oauth_user) = serde_json::from_str::<AppleOAuthUser>(user) {
                first_name = apple_oauth_user.name.first_name.unwrap_or(String::from(""));
                last_name = apple_oauth_user.name.last_name.unwrap_or(String::from(""));
                email = apple_oauth_user.email.unwrap_or(String::from(""));
            }
        }
        assert_eq!(first_name, "");
        assert_eq!(last_name, "");
        assert_eq!(email, "abc@gmail.com");

        // let user = Some("{\"name\":{\"firstName\":\"First\",\"lastName\":\"Last\"},\"email\":\"abc@gmail.com\"}");

        // if let Some(user) = user {
        //     if let Ok(h) = serde_json::from_str::<HashMap<String, Value>>(user) {

        //         first_name = h.name.first_name.unwrap_or(String::from(""));
        //         last_name = apple_oauth_user.name.last_name.unwrap_or(String::from(""));
        //         email = apple_oauth_user.email.unwrap_or(String::from(""));
        //     }
        // }
        // assert_eq!(first_name, "First");
        // assert_eq!(last_name, "Last");
        // assert_eq!(email, "abc@gmail.com");
    }

    #[tokio::test]
    async fn test_get_available_verbs() {
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![], 1);
        a.sort();
        assert_eq!(vec![1, 2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![1], 1);
        a.sort();
        assert_eq!(vec![2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![1, 2], 1);
        a.sort();
        assert_eq!(vec![3], a);
        let mut a =
            hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![3, 1, 2], 1); //skip 3
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![2, 1, 2, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        );
        a.sort();
        assert_eq!(vec![1, 3], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![1, 3, 2, 1, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        );
        a.sort();
        assert_eq!(vec![2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![2, 1, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        ); //skip 2
        a.sort();
        assert_eq!(vec![1, 3], a);

        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![], 2);
        a.sort();
        assert_eq!(vec![1, 2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![1, 1], 2);
        a.sort();
        assert_eq!(vec![2, 3], a);
        let mut a =
            hc_get_available_verbs_practice(&Some(String::from("1,2,3")), &vec![1, 1, 2, 2], 2);
        a.sort();
        assert_eq!(vec![3], a);

        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![3, 3, 1, 1, 2, 2],
            2,
        ); //skip 3
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![
                3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 3, 3, 1, 1, 3, 3, 1, 1, 2, 2,
            ],
            2,
        );
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![
                1, 1, 2, 2, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 3, 3, 1, 1, 2, 2,
            ],
            2,
        );
        a.sort();
        assert_eq!(vec![3], a);
        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2,3")),
            &vec![
                2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 3, 3, 1, 1, 3, 3, 1, 1, 2, 2,
            ],
            2,
        ); //skip 2
        a.sort();
        assert_eq!(vec![1, 3], a);

        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2")), &vec![], 2);
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1,2")), &vec![1, 1], 2);
        a.sort();
        assert_eq!(vec![2], a);
        let mut a =
            hc_get_available_verbs_practice(&Some(String::from("1,2")), &vec![1, 1, 2, 2], 2);
        a.sort();
        assert_eq!(vec![2], a);

        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1,2")),
            &vec![2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2],
            2,
        );
        a.sort();
        assert_eq!(vec![1], a);

        let mut a = hc_get_available_verbs_practice(&Some(String::from("1")), &vec![], 1);
        a.sort();
        assert_eq!(vec![1], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1")), &vec![1, 1], 1);
        a.sort();
        assert_eq!(vec![1], a);
        let mut a = hc_get_available_verbs_practice(&Some(String::from("1")), &vec![1, 1, 1], 1);
        a.sort();
        assert_eq!(vec![1], a);

        let mut a = hc_get_available_verbs_practice(
            &Some(String::from("1")),
            &vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            1,
        );
        a.sort();
        assert_eq!(vec![1], a);
    }

    #[tokio::test]
    async fn test_change_verb() {
        assert!(hc_change_verbs(&vec![], 2));
        assert!(hc_change_verbs(&vec![1, 1, 2, 2], 2));
        assert!(!hc_change_verbs(&vec![1, 1, 2, 2, 2], 3));
        assert!(hc_change_verbs(&vec![1, 1, 1, 1, 1, 2, 2, 2, 2, 2], 5));
        assert!(!hc_change_verbs(&vec![1, 1, 1, 1, 2, 2, 2, 2, 2], 5));
    }

    #[tokio::test]
    async fn test_login() {
        initialize_db_once().await; //only works for postgres, sqlite initialized in get_db()
        let db = get_db().await;
        let timestamp = get_timestamp();

        let uuid1 = hc_create_user(&db, "testuser9", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();

        //failing credentials
        let credentials = Credentials {
            username: String::from("testuser9"),
            password: Secret::new("abcdabcdx".to_string()),
        };
        let res = hc_validate_credentials(&db, credentials).await;
        assert_eq!(res, Err(HcError::AuthenticationError));

        //passing credentials
        let credentials = Credentials {
            username: String::from("testuser9"),
            password: Secret::new("abcdabcd".to_string()),
        };
        let res = hc_validate_credentials(&db, credentials).await;
        assert_eq!(res.unwrap(), uuid1);

        //insert oauth user
        let res = hc_create_oauth_user(
            &db,
            "oauth_issuer".to_string(),
            "oauth_sub".to_string(),
            "first name",
            "last name",
            "blah@blah.com",
            timestamp,
        )
        .await;
        assert!(res.is_ok());

        //second time user will have the same user_id
        let res2 = hc_create_oauth_user(
            &db,
            "oauth_issuer".to_string(),
            "oauth_sub".to_string(),
            "first name",
            "last name",
            "blah@blah.com",
            timestamp,
        )
        .await;
        assert_eq!(res.unwrap().0, res2.unwrap().0);
    }

    #[tokio::test]
    async fn test_two_player() {
        initialize_db_once().await; //only works for postgres, sqlite initialized in get_db()
        let db = get_db().await;
        let verbs = hc_load_verbs("pp.txt");

        let mut timestamp = get_timestamp();

        let uuid1 = hc_create_user(&db, "testuser1", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let uuid2 = hc_create_user(&db, "testuser2", "abcdabcd", "user2@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid =
            hc_create_user(&db, "testuser3", "abcdabcd", "user3@blah.com", timestamp)
                .await
                .unwrap();

        let mut csq = CreateSessionQuery {
            qtype: String::from("abc"),
            name: None,
            verbs: Some(String::from("20")),
            units: None,
            params: None,
            highest_unit: None,
            opponent: String::from("testuser2"),
            countdown: true,
            practice_reps_per_verb: Some(4),
            max_changes: 4,
            max_time: 30,
        };

        let session_uuid = hc_insert_session(&db, uuid1, &mut csq, &verbs, timestamp).await;
        //assert!(res.is_ok());

        let aq = AskQuery {
            qtype: String::from("ask"),
            session_id: *session_uuid.as_ref().unwrap(),
            person: 0,
            number: 0,
            tense: 0,
            voice: 0,
            mood: 0,
            verb: 1,
        };

        //ask from invalid user should be blocked
        let ask = hc_ask(&db, invalid_uuid, &aq, timestamp, &verbs).await;
        assert!(ask.is_err());

        //a valid ask
        let ask = hc_ask(&db, uuid1, &aq, timestamp, &verbs).await;
        if ask.is_err() {
            println!("error {ask:?}");
        }
        assert!(ask.is_ok());

        let mut tx = db.begin_tx().await.unwrap();
        let s = hc_get_sessions_tr(&mut tx, uuid1).await;
        tx.commit_tx().await.unwrap();
        // let s_res = Ok([SessionsListQuery { session_id: 75d08792-ea12-40f6-a903-bd4e6aae2aad,
        //     challenged: Some(cffd0d33-6aab-45c0-9dc1-279ae4ecaafa),
        //     opponent: Some(cffd0d33-6aab-45c0-9dc1-279ae4ecaafa),
        //     opponent_name: Some("testuser2"),
        //     timestamp: 1665286722,
        //     myturn: false,
        //     move_type: AnswerTheirTurn,
        //     my_score: Some(0),
        //     their_score: Some(0) }]);

        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::AnswerTheirTurn);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(0));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(0));

        //check that we are preventing out-of-sequence asks
        let ask = hc_ask(&db, uuid1, &aq, timestamp, &verbs).await;
        assert!(ask.is_err());

        let m = GetMoveQuery {
            qtype: String::from("getmove"),
            session_id: *session_uuid.as_ref().unwrap(),
        };

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some(String::from("παιδεύω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(1),
            person: Some(0),
            number: Some(0),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move_tr(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some(String::from("παιδεύω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(1),
            person: Some(0),
            number: Some(0),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        let answerq = AnswerQuery {
            qtype: String::from("abc"),
            answer: String::from("παιδεύω"),
            time: String::from("25:01"),
            mf_pressed: false,
            timed_out: false,
            session_id: *session_uuid.as_ref().unwrap(),
        };

        //answer from invalid user should be blocked
        let answer = hc_answer(&db, invalid_uuid, &answerq, timestamp, &verbs).await;
        assert!(answer.is_err());

        //a valid answer
        let answer = hc_answer(&db, uuid2, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        assert!(answer.unwrap().is_correct.unwrap());

        //check that we are preventing out-of-sequence answers
        let answer = hc_answer(&db, uuid2, &answerq, timestamp, &verbs).await;
        assert!(answer.is_err());

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskTheirTurn,
            myturn: false,
            starting_form: Some(String::from("παιδεύω")),
            answer: Some(String::from("παιδεύω")),
            is_correct: Some(true),
            correct_answer: Some(String::from("παιδεύω")),
            verb: Some(1),
            person: Some(0),
            number: Some(0),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: Some(String::from("25:01")),
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move_tr(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskMyTurn,
            myturn: true,
            starting_form: Some(String::from("παιδεύω")),
            answer: Some(String::from("παιδεύω")),
            is_correct: Some(true),
            correct_answer: Some(String::from("παιδεύω")),
            verb: Some(1),
            person: Some(0),
            number: Some(0),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: Some(String::from("25:01")),
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        let aq2 = AskQuery {
            qtype: String::from("ask"),
            session_id: *session_uuid.as_ref().unwrap(),
            person: 1,
            number: 1,
            tense: 0,
            voice: 0,
            mood: 0,
            verb: 1,
        };

        timestamp += 1;
        //a valid ask
        let ask = hc_ask(&db, uuid2, &aq2, timestamp, &verbs).await;
        assert!(ask.is_ok());

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert!(ss.is_ok());
        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some(String::from("παιδεύω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(1),
            person: Some(1),
            number: Some(1),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: Some(0),
            number_prev: Some(0),
            tense_prev: Some(0),
            voice_prev: Some(0),
            mood_prev: Some(0),
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move_tr(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some(String::from("παιδεύω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(1),
            person: Some(1),
            number: Some(1),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: Some(0),
            number_prev: Some(0),
            tense_prev: Some(0),
            voice_prev: Some(0),
            mood_prev: Some(0),
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };
        assert!(ss2.unwrap() == ss_res2);

        //an incorrect answer
        timestamp += 1;
        let answerq = AnswerQuery {
            qtype: String::from("abc"),
            answer: String::from("παιδ"),
            time: String::from("25:01"),
            mf_pressed: false,
            timed_out: false,
            session_id: *session_uuid.as_ref().unwrap(),
        };

        //a valid answer
        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        assert!(!answer.unwrap().is_correct.unwrap());

        let mut tx = db.begin_tx().await.unwrap();
        let s = hc_get_sessions_tr(&mut tx, uuid1).await;
        tx.commit_tx().await.unwrap();
        // let s_res = Ok([SessionsListQuery {
        // session_id: c152c43f-d52c-496b-ab34-da44ab61275c,
        // challenged: Some(0faa61fe-b89a-4f76-b1f3-1c39da26903f),
        // opponent: Some(0faa61fe-b89a-4f76-b1f3-1c39da26903f),
        // opponent_name: Some("testuser2"),
        // timestamp: 1665287228,
        // myturn: true,
        // move_type: FirstMoveMyTurn,
        // my_score: Some(0),
        // their_score: Some(1) }]);

        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::FirstMoveMyTurn);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(0));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(1));

        let mut tx = db.begin_tx().await.unwrap();
        let s = hc_get_sessions_tr(&mut tx, uuid2).await;
        tx.commit_tx().await.unwrap();

        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::AskTheirTurn);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(1));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(0));

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::FirstMoveMyTurn,
            myturn: true,
            starting_form: Some(String::from("παιδεύω")),
            answer: Some(String::from("παιδ")),
            is_correct: Some(false),
            correct_answer: Some(String::from("παιδεύετε")),
            verb: Some(1),
            person: Some(1),
            number: Some(1),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: Some(String::from("25:01")),
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: Some(vec![
                /* take out paideuw: HCVerbOption { id: 1, verb: String::from("παιδεύω") },*/
                HCVerbOption {
                    id: 114,
                    verb: String::from("—, ἀνερήσομαι"),
                },
                HCVerbOption {
                    id: 115,
                    verb: String::from("—, ἐρήσομαι"),
                },
                HCVerbOption {
                    id: 30,
                    verb: String::from("ἀγγέλλω"),
                },
                HCVerbOption {
                    id: 24,
                    verb: String::from("ἄγω"),
                },
                HCVerbOption {
                    id: 26,
                    verb: String::from("ἀδικέω"),
                },
                HCVerbOption {
                    id: 74,
                    verb: String::from("αἱρέω"),
                },
                HCVerbOption {
                    id: 75,
                    verb: String::from("αἰσθάνομαι"),
                },
                HCVerbOption {
                    id: 111,
                    verb: String::from("αἰσχῡ\u{301}νομαι"),
                },
                HCVerbOption {
                    id: 36,
                    verb: String::from("ἀκούω"),
                },
                HCVerbOption {
                    id: 93,
                    verb: String::from("ἁμαρτάνω"),
                },
                HCVerbOption {
                    id: 84,
                    verb: String::from("ἀναβαίνω"),
                },
                HCVerbOption {
                    id: 43,
                    verb: String::from("ἀνατίθημι"),
                },
                HCVerbOption {
                    id: 31,
                    verb: String::from("ἀξιόω"),
                },
                HCVerbOption {
                    id: 37,
                    verb: String::from("ἀποδέχομαι"),
                },
                HCVerbOption {
                    id: 44,
                    verb: String::from("ἀποδίδωμι"),
                },
                HCVerbOption {
                    id: 100,
                    verb: String::from("ἀποθνῄσκω"),
                },
                HCVerbOption {
                    id: 112,
                    verb: String::from("ἀποκρῑ\u{301}νομαι"),
                },
                HCVerbOption {
                    id: 101,
                    verb: String::from("ἀποκτείνω"),
                },
                HCVerbOption {
                    id: 113,
                    verb: String::from("ἀπόλλῡμι"),
                },
                HCVerbOption {
                    id: 13,
                    verb: String::from("ἄρχω"),
                },
                HCVerbOption {
                    id: 102,
                    verb: String::from("ἀφῑ\u{301}ημι"),
                },
                HCVerbOption {
                    id: 121,
                    verb: String::from("ἀφικνέομαι"),
                },
                HCVerbOption {
                    id: 45,
                    verb: String::from("ἀφίστημι"),
                },
                HCVerbOption {
                    id: 85,
                    verb: String::from("βαίνω"),
                },
                HCVerbOption {
                    id: 38,
                    verb: String::from("βάλλω"),
                },
                HCVerbOption {
                    id: 14,
                    verb: String::from("βλάπτω"),
                },
                HCVerbOption {
                    id: 103,
                    verb: String::from("βουλεύω"),
                },
                HCVerbOption {
                    id: 39,
                    verb: String::from("βούλομαι"),
                },
                HCVerbOption {
                    id: 53,
                    verb: String::from("γίγνομαι"),
                },
                HCVerbOption {
                    id: 86,
                    verb: String::from("γιγνώσκω"),
                },
                HCVerbOption {
                    id: 5,
                    verb: String::from("γράφω"),
                },
                HCVerbOption {
                    id: 122,
                    verb: String::from("δεῖ"),
                },
                HCVerbOption {
                    id: 61,
                    verb: String::from("δείκνῡμι"),
                },
                HCVerbOption {
                    id: 40,
                    verb: String::from("δέχομαι"),
                },
                HCVerbOption {
                    id: 32,
                    verb: String::from("δηλόω"),
                },
                HCVerbOption {
                    id: 76,
                    verb: String::from("διαφέρω"),
                },
                HCVerbOption {
                    id: 9,
                    verb: String::from("διδάσκω"),
                },
                HCVerbOption {
                    id: 46,
                    verb: String::from("δίδωμι"),
                },
                HCVerbOption {
                    id: 94,
                    verb: String::from("δοκέω"),
                },
                HCVerbOption {
                    id: 17,
                    verb: String::from("δουλεύω"),
                },
                HCVerbOption {
                    id: 95,
                    verb: String::from("δύναμαι"),
                },
                HCVerbOption {
                    id: 10,
                    verb: String::from("ἐθέλω"),
                },
                HCVerbOption {
                    id: 77,
                    verb: String::from("εἰμί"),
                },
                HCVerbOption {
                    id: 96,
                    verb: String::from("εἶμι"),
                },
                HCVerbOption {
                    id: 87,
                    verb: String::from("ἐκπῑ\u{301}πτω"),
                },
                HCVerbOption {
                    id: 97,
                    verb: String::from("ἐλαύνω"),
                },
                HCVerbOption {
                    id: 79,
                    verb: String::from("ἔξεστι(ν)"),
                },
                HCVerbOption {
                    id: 62,
                    verb: String::from("ἐπανίσταμαι"),
                },
                HCVerbOption {
                    id: 104,
                    verb: String::from("ἐπιβουλεύω"),
                },
                HCVerbOption {
                    id: 63,
                    verb: String::from("ἐπιδείκνυμαι"),
                },
                HCVerbOption {
                    id: 98,
                    verb: String::from("ἐπίσταμαι"),
                },
                HCVerbOption {
                    id: 80,
                    verb: String::from("ἕπομαι"),
                },
                HCVerbOption {
                    id: 54,
                    verb: String::from("ἔρχομαι"),
                },
                HCVerbOption {
                    id: 64,
                    verb: String::from("ἐρωτάω"),
                },
                HCVerbOption {
                    id: 78,
                    verb: String::from("ἔστι(ν)"),
                },
                HCVerbOption {
                    id: 116,
                    verb: String::from("εὑρίσκω"),
                },
                HCVerbOption {
                    id: 99,
                    verb: String::from("ἔχω"),
                },
                HCVerbOption {
                    id: 105,
                    verb: String::from("ζητέω"),
                },
                HCVerbOption {
                    id: 117,
                    verb: String::from("ἡγέομαι"),
                },
                HCVerbOption {
                    id: 25,
                    verb: String::from("ἥκω"),
                },
                HCVerbOption {
                    id: 11,
                    verb: String::from("θάπτω"),
                },
                HCVerbOption {
                    id: 6,
                    verb: String::from("θῡ\u{301}ω"),
                },
                HCVerbOption {
                    id: 106,
                    verb: String::from("ῑ\u{314}\u{301}ημι"),
                },
                HCVerbOption {
                    id: 47,
                    verb: String::from("ἵστημι"),
                },
                HCVerbOption {
                    id: 48,
                    verb: String::from("καθίστημι"),
                },
                HCVerbOption {
                    id: 33,
                    verb: String::from("καλέω"),
                },
                HCVerbOption {
                    id: 49,
                    verb: String::from("καταλῡ\u{301}ω"),
                },
                HCVerbOption {
                    id: 123,
                    verb: String::from("κεῖμαι"),
                },
                HCVerbOption {
                    id: 3,
                    verb: String::from("κελεύω"),
                },
                HCVerbOption {
                    id: 21,
                    verb: String::from("κλέπτω"),
                },
                HCVerbOption {
                    id: 118,
                    verb: String::from("κρῑ\u{301}νω"),
                },
                HCVerbOption {
                    id: 18,
                    verb: String::from("κωλῡ\u{301}ω"),
                },
                HCVerbOption {
                    id: 41,
                    verb: String::from("λαμβάνω"),
                },
                HCVerbOption {
                    id: 65,
                    verb: String::from("λανθάνω"),
                },
                HCVerbOption {
                    id: 88,
                    verb: String::from("λέγω"),
                },
                HCVerbOption {
                    id: 22,
                    verb: String::from("λείπω"),
                },
                HCVerbOption {
                    id: 4,
                    verb: String::from("λῡ\u{301}ω"),
                },
                HCVerbOption {
                    id: 55,
                    verb: String::from("μανθάνω"),
                },
                HCVerbOption {
                    id: 56,
                    verb: String::from("μάχομαι"),
                },
                HCVerbOption {
                    id: 107,
                    verb: String::from("μέλλω"),
                },
                HCVerbOption {
                    id: 34,
                    verb: String::from("μένω"),
                },
                HCVerbOption {
                    id: 57,
                    verb: String::from("μεταδίδωμι"),
                },
                HCVerbOption {
                    id: 58,
                    verb: String::from("μετανίσταμαι"),
                },
                HCVerbOption {
                    id: 59,
                    verb: String::from("μηχανάομαι"),
                },
                HCVerbOption {
                    id: 27,
                    verb: String::from("νῑκάω"),
                },
                HCVerbOption {
                    id: 89,
                    verb: String::from("νομίζω"),
                },
                HCVerbOption {
                    id: 119,
                    verb: String::from("οἶδα"),
                },
                HCVerbOption {
                    id: 81,
                    verb: String::from("ὁράω"),
                },
                HCVerbOption {
                    id: 66,
                    verb: String::from("παραγίγνομαι"),
                },
                HCVerbOption {
                    id: 67,
                    verb: String::from("παραδίδωμι"),
                },
                HCVerbOption {
                    id: 68,
                    verb: String::from("παραμένω"),
                },
                HCVerbOption {
                    id: 42,
                    verb: String::from("πάσχω"),
                },
                HCVerbOption {
                    id: 7,
                    verb: String::from("παύω"),
                },
                HCVerbOption {
                    id: 15,
                    verb: String::from("πείθω"),
                },
                HCVerbOption {
                    id: 2,
                    verb: String::from("πέμπω"),
                },
                HCVerbOption {
                    id: 90,
                    verb: String::from("πῑ\u{301}πτω"),
                },
                HCVerbOption {
                    id: 108,
                    verb: String::from("πιστεύω"),
                },
                HCVerbOption {
                    id: 28,
                    verb: String::from("ποιέω"),
                },
                HCVerbOption {
                    id: 19,
                    verb: String::from("πολῑτεύω"),
                },
                HCVerbOption {
                    id: 16,
                    verb: String::from("πρᾱ\u{301}ττω"),
                },
                HCVerbOption {
                    id: 91,
                    verb: String::from("προδίδωμι"),
                },
                HCVerbOption {
                    id: 124,
                    verb: String::from("πυνθάνομαι"),
                },
                HCVerbOption {
                    id: 109,
                    verb: String::from("συμβουλεύω"),
                },
                HCVerbOption {
                    id: 82,
                    verb: String::from("συμφέρω"),
                },
                HCVerbOption {
                    id: 110,
                    verb: String::from("συνῑ\u{301}ημι"),
                },
                HCVerbOption {
                    id: 120,
                    verb: String::from("σύνοιδα"),
                },
                HCVerbOption {
                    id: 23,
                    verb: String::from("σῴζω"),
                },
                HCVerbOption {
                    id: 12,
                    verb: String::from("τάττω"),
                },
                HCVerbOption {
                    id: 35,
                    verb: String::from("τελευτάω"),
                },
                HCVerbOption {
                    id: 50,
                    verb: String::from("τίθημι"),
                },
                HCVerbOption {
                    id: 29,
                    verb: String::from("τῑμάω"),
                },
                HCVerbOption {
                    id: 125,
                    verb: String::from("τρέπω"),
                },
                HCVerbOption {
                    id: 69,
                    verb: String::from("τυγχάνω"),
                },
                HCVerbOption {
                    id: 70,
                    verb: String::from("ὑπακούω"),
                },
                HCVerbOption {
                    id: 71,
                    verb: String::from("ὑπομένω"),
                },
                HCVerbOption {
                    id: 126,
                    verb: String::from("φαίνω"),
                },
                HCVerbOption {
                    id: 83,
                    verb: String::from("φέρω"),
                },
                HCVerbOption {
                    id: 60,
                    verb: String::from("φεύγω"),
                },
                HCVerbOption {
                    id: 92,
                    verb: String::from("φημί"),
                },
                HCVerbOption {
                    id: 72,
                    verb: String::from("φθάνω"),
                },
                HCVerbOption {
                    id: 51,
                    verb: String::from("φιλέω"),
                },
                HCVerbOption {
                    id: 52,
                    verb: String::from("φοβέομαι"),
                },
                HCVerbOption {
                    id: 8,
                    verb: String::from("φυλάττω"),
                },
                HCVerbOption {
                    id: 73,
                    verb: String::from("χαίρω"),
                },
                HCVerbOption {
                    id: 20,
                    verb: String::from("χορεύω"),
                },
                HCVerbOption {
                    id: 127,
                    verb: String::from("χρή"),
                },
            ]),
        };
        //println!("{:?}\n\n{:?}", ss_res, ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move_tr(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskTheirTurn,
            myturn: false,
            starting_form: Some(String::from("παιδεύω")),
            answer: Some(String::from("παιδ")),
            is_correct: Some(false),
            correct_answer: Some(String::from("παιδεύετε")),
            verb: Some(1),
            person: Some(1),
            number: Some(1),
            tense: Some(0),
            voice: Some(0),
            mood: Some(0),
            person_prev: Some(0),
            number_prev: Some(0),
            tense_prev: Some(0),
            voice_prev: Some(0),
            mood_prev: Some(0),
            time: Some(String::from("25:01")),
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        //ask new verb after incorrect result
        let aq3 = AskQuery {
            qtype: String::from("ask"),
            session_id: *session_uuid.as_ref().unwrap(),
            person: 0,
            number: 0,
            tense: 1,
            voice: 1,
            mood: 1,
            verb: 2,
        };

        timestamp += 1;
        //a valid ask
        let ask = hc_ask(&db, uuid1, &aq3, timestamp, &verbs).await;
        assert!(ask.is_ok());

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert!(ss.is_ok());
        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some(String::from("πέμπω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(2),
            person: Some(0),
            number: Some(0),
            tense: Some(1),
            voice: Some(1),
            mood: Some(1),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move_tr(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some(String::from("πέμπω")),
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: Some(2),
            person: Some(0),
            number: Some(0),
            tense: Some(1),
            voice: Some(1),
            mood: Some(1),
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None,
            response_to: String::from("getmoves"),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss2.as_ref().unwrap());
        //println!("2: {:?}", ss_res2);
        assert!(ss2.unwrap() == ss_res2);
    }

    #[tokio::test]
    async fn test_practice() {
        initialize_db_once().await; //only works for postgres, sqlite initialized in get_db()
        let db = get_db().await;
        let verbs = hc_load_verbs("pp.txt");

        let timestamp = get_timestamp();

        let uuid1 = hc_create_user(&db, "testuser4", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid =
            hc_create_user(&db, "testuser6", "abcdabcd", "user3@blah.com", timestamp)
                .await
                .unwrap();

        let mut csq = CreateSessionQuery {
            qtype: String::from("abc"),
            name: None,
            verbs: Some(String::from("20")),
            units: None,
            params: None,
            highest_unit: None,
            opponent: String::from(""),
            countdown: true,
            practice_reps_per_verb: Some(4),
            max_changes: 4,
            max_time: 30,
        };

        let session_uuid = hc_insert_session(&db, uuid1, &mut csq, &verbs, timestamp).await;
        //assert!(res.is_ok());

        let aq = AskQuery {
            qtype: String::from("ask"),
            session_id: *session_uuid.as_ref().unwrap(),
            person: 0,
            number: 0,
            tense: 0,
            voice: 0,
            mood: 0,
            verb: 0,
        };

        //ask from invalid user should be blocked
        let ask = hc_ask(&db, invalid_uuid, &aq, timestamp, &verbs).await;
        assert!(ask.is_err());

        //a valid ask
        let ask = hc_ask(&db, uuid1, &aq, timestamp, &verbs).await;
        assert!(ask.is_err());

        let mut tx = db.begin_tx().await.unwrap();
        let s = hc_get_sessions_tr(&mut tx, uuid1).await;
        tx.commit_tx().await.unwrap();

        // let s_res = Ok([SessionsListQuery { session_id: 75d08792-ea12-40f6-a903-bd4e6aae2aad,
        //     challenged: Some(cffd0d33-6aab-45c0-9dc1-279ae4ecaafa),
        //     opponent: Some(cffd0d33-6aab-45c0-9dc1-279ae4ecaafa),
        //     opponent_name: Some("testuser2"),
        //     timestamp: 1665286722,
        //     myturn: false,
        //     move_type: AnswerTheirTurn,
        //     my_score: Some(0),
        //     their_score: Some(0) }]);

        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::Practice);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(0));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(0));

        let m = GetMoveQuery {
            qtype: String::from("getmove"),
            session_id: *session_uuid.as_ref().unwrap(),
        };
        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move_tr(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert_eq!(ss.as_ref().unwrap().move_type, MoveType::Practice);
        assert!(ss.as_ref().unwrap().myturn);

        // let ss_res = SessionState {
        //     session_id: *session_uuid.as_ref().unwrap(),
        //     move_type: MoveType::Practice,
        //     myturn: true,
        //     starting_form: Some(String::from("παιδεύω")),
        //     answer: None,
        //     is_correct: None,
        //     correct_answer: None,
        //     verb: Some(0),
        //     person: Some(0),
        //     number: Some(0),
        //     tense: Some(0),
        //     voice: Some(0),
        //     mood: Some(0),
        //     person_prev: None,
        //     number_prev: None,
        //     tense_prev: None,
        //     voice_prev: None,
        //     mood_prev: None,
        //     time: None,
        //     response_to: String::from("getmoves"),
        //     success: true,
        //     mesg: None,
        //     verbs: None,
        // };

        let answerq = AnswerQuery {
            qtype: String::from("abc"),
            answer: String::from("παιδεύω"),
            time: String::from("25:01"),
            mf_pressed: false,
            timed_out: false,
            session_id: *session_uuid.as_ref().unwrap(),
        };

        //answer from invalid user should be blocked
        let answer = hc_answer(&db, invalid_uuid, &answerq, timestamp, &verbs).await;
        assert!(answer.is_err());

        //a valid answer
        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        // Ok(SessionState { session_id: 1835f2a1-c896-4e7d-b526-b46855b95e23,
        //     move_type: Practice,
        //     myturn: true,
        //     starting_form: Some("παιδεύωμαι"),
        //     answer: None,
        //     is_correct: Some(false),
        //     correct_answer: Some("παιδεύωμαι"),
        //     verb: Some(0),
        //     person: Some(2), number: Some(0), tense: Some(0), voice: Some(0), mood: Some(1),
        //     person_prev: Some(0), number_prev: Some(0), tense_prev: Some(0), voice_prev: Some(2), mood_prev: Some(1),
        //     time: None, response_to: "answerresponsepractice", success: true, mesg: None, verbs: None })
        //println!("{:?}", answer);
        assert_eq!(answer.as_ref().unwrap().move_type, MoveType::Practice);
        assert!(answer.as_ref().unwrap().myturn);

        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());

        //let ss = hc_get_move_tr(&db, uuid1, false, m.session_id, &verbs).await;
    }
}
