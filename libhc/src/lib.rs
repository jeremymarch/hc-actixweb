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

use chrono::prelude::*;
use hoplite_verbs_rs::*;
use polytonic_greek::hgk_compare_multiple_forms;
use polytonic_greek::hgk_compare_sqlite;
use rand::prelude::SliceRandom;
use sqlx::types::Uuid;
use std::collections::HashSet;
pub mod db;
pub mod dbsqlite;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::Arc;

pub fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

#[derive(Deserialize, Serialize)]
pub struct GetSessions {
    pub qtype: String,
    pub current_session: Option<sqlx::types::Uuid>,
}

#[derive(Deserialize, Serialize)]
pub struct SessionsListResponse {
    pub response_to: String,
    pub sessions: Vec<SessionsListQuery>,
    pub success: bool,
    pub username: Option<String>,
    pub current_session: Option<SessionState>,
}

#[derive(Deserialize, Serialize, FromRow)]
pub struct UserResult {
    user_id: sqlx::types::Uuid,
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

#[derive(Deserialize, Serialize, FromRow)]
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
    pub session_id: sqlx::types::Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct GetMovesQuery {
    pub qtype: String,
    pub session_id: sqlx::types::Uuid,
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

#[derive(Deserialize, Serialize, FromRow)]
pub struct MoveResult {
    move_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
    ask_user_id: Option<sqlx::types::Uuid>,
    answer_user_id: Option<sqlx::types::Uuid>,
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

#[derive(PartialEq, Debug, Eq, Deserialize, Serialize, FromRow)]
pub struct SessionsListQuery {
    pub session_id: sqlx::types::Uuid,
    pub name: Option<String>,
    pub challenged: Option<sqlx::types::Uuid>, //the one who didn't start the game, or null for practice
    //pub opponent: Option<sqlx::types::Uuid>,
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
pub trait HcDbTrait {
    async fn begin_tx(&self) -> Result<Box<dyn HcTrx>, sqlx::Error>;
}

#[async_trait]
pub trait HcTrx {
    async fn commit_tx(self: Box<Self>) -> Result<(), sqlx::Error>;
    async fn rollback_tx(self: Box<Self>) -> Result<(), sqlx::Error>;
    //async fn get_tx(mut self) -> &dyn

    async fn add_to_score<'a, 'b>(
        &mut self,
        session_id: Uuid,
        user_to_score: &str,
        points: i32,
    ) -> Result<u32, sqlx::Error>;

    async fn validate_login_db(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<Uuid, sqlx::Error>;

    async fn get_user_id(&mut self, username: &str) -> Result<UserResult, sqlx::Error>;

    // async fn insert_session(
    //     &mut self,
    //     user_id: Uuid,
    //     highest_unit: Option<i16>,
    //     opponent_id: Option<Uuid>,
    //     info: &CreateSessionQuery,
    //     timestamp: i64,
    // ) -> Result<Uuid, sqlx::Error>;

    async fn insert_session_tx(
        &mut self,
        user_id: Uuid,
        highest_unit: Option<i16>,
        opponent_id: Option<Uuid>,
        info: &CreateSessionQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error>;

    async fn get_game_moves(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<Vec<MoveResult>, sqlx::Error>;

    async fn get_sessions(
        &mut self,
        user_id: sqlx::types::Uuid,
    ) -> Result<Vec<SessionsListQuery>, sqlx::Error>;

    // async fn get_last_move(&mut self, session_id: sqlx::types::Uuid)
    //     -> Result<MoveResult, sqlx::Error>;

    async fn get_last_move_tx(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<MoveResult, sqlx::Error>;

    async fn get_last_n_moves(
        &mut self,
        session_id: sqlx::types::Uuid,
        n: u8,
    ) -> Result<Vec<MoveResult>, sqlx::Error>;

    // async fn get_session(
    //     &mut self,
    //     session_id: sqlx::types::Uuid,
    // ) -> Result<SessionResult, sqlx::Error>;

    async fn get_session_tx(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<SessionResult, sqlx::Error>;

    async fn get_used_verbs(
        &mut self,
        session_id: sqlx::types::Uuid,
    ) -> Result<Vec<i32>, sqlx::Error>;

    // async fn insert_ask_move(
    //     &mut self,
    //     user_id: Option<Uuid>,
    //     info: &AskQuery,
    //     timestamp: i64,
    // ) -> Result<Uuid, sqlx::Error>;

    async fn insert_ask_move_tx(
        &mut self,
        user_id: Option<Uuid>,
        info: &AskQuery,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error>;

    // async fn update_answer_move(
    //     &self,
    //     info: &AnswerQuery,
    //     user_id: Uuid,
    //     correct_answer: &str,
    //     is_correct: bool,
    //     mf_pressed: bool,
    //     timestamp: i64,
    // ) -> Result<u32, sqlx::Error>;

    #[allow(clippy::too_many_arguments)]
    async fn update_answer_move_tx<'a, 'b>(
        &mut self,
        info: &AnswerQuery,
        user_id: Uuid,
        correct_answer: &str,
        is_correct: bool,
        mf_pressed: bool,
        timestamp: i64,
    ) -> Result<u32, sqlx::Error>;

    async fn create_user(
        &mut self,
        username: &str,
        password: &str,
        email: &str,
        timestamp: i64,
    ) -> Result<Uuid, sqlx::Error>;

    async fn create_db(&mut self) -> Result<u32, sqlx::Error>;
}

// pub async fn get_session_state(
//     db: &dyn HcDbTrait,
//     user_id: sqlx::types::Uuid,
//     session_id: sqlx::types::Uuid,
// ) -> Result<SessionState, sqlx::Error> {
//     //let mut tx = db.db.begin().await?;
//     let mut tx = db.begin_tx().await?;

//     let r = get_session_state_tx(&mut tx, db, user_id, session_id).await?;

//     tx.commit().await?;
//     Ok(r)
// }

pub async fn get_session_state_tx(
    tx: &mut Box<dyn HcTrx>,
    user_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
) -> Result<SessionState, sqlx::Error> {
    //let mut tx = self.db.begin_tx().await?;

    let res = tx.get_session_tx(session_id).await?;
    let m = tx.get_last_n_moves(session_id, 2).await?;

    let first = if !m.is_empty() { Some(&m[0]) } else { None };
    let (myturn, move_type) = move_get_type(first, user_id, res.challenged_user_id);

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
        response_to: "".to_string(),
        success: true,
        mesg: None,
        verbs: None,
    };

    Ok(r)
}

pub async fn hc_ask(
    db: &dyn HcDbTrait,
    user_id: Uuid,
    info: &AskQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, sqlx::Error> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    //todo check that user_id == challenger_user_id if this is first move
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //prevent out-of-sequence asks
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id)
                || m.answer_user_id != Some(user_id)
                || m.is_correct.is_none()
            {
                return Err(sqlx::Error::RowNotFound); //same user cannot ask twice in a row and ask user must be same as previous answer user and previous answer must be marked correct or incorrect
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

    let mut res = get_session_state_tx(&mut tx, user_id, info.session_id).await?;

    if res.starting_form.is_none()
        && res.verb.is_some()
        && (res.verb.unwrap() as usize) < verbs.len()
    {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "ask".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = None;

    tx.commit_tx().await?;

    Ok(res)
}

pub async fn hc_answer(
    db: &dyn HcDbTrait,
    user_id: Uuid,
    info: &AnswerQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, sqlx::Error> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    //let mut tx = db.db.begin().await?;
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some() {
                return Err(sqlx::Error::RowNotFound); //same user cannot answer question they asked and previous question must not already be answered
            } else {
                m
            }
        }
        Err(_) => {
            return Err(sqlx::Error::RowNotFound);
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
        Err(_) => "—".to_string(),
    };

    let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));

    let _res = tx
        .update_answer_move_tx(
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
        ask_practice(&mut tx, prev_form, &s, timestamp, m.asktimestamp, verbs).await?;
    } else {
        //add to other player's score if not practice and not correct
        if !is_correct {
            let user_to_score = if s.challenger_user_id == user_id {
                "challenged_score"
            } else {
                "challenger_score"
            };
            let points = 1;
            let _ = tx
                .add_to_score(info.session_id, user_to_score, points)
                .await?;
        }
    }

    let mut res = get_session_state_tx(&mut tx, user_id, info.session_id).await?;
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
        res.response_to = "answerresponsepractice".to_string();
    } else {
        res.response_to = "answerresponse".to_string();
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
    db: &dyn HcDbTrait,
    user_id: Uuid,
    info: &AnswerQuery,
    timestamp: i64,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, sqlx::Error> {
    //let mut tx = db.db.begin().await?;
    let mut tx = db.begin_tx().await?;

    let s = tx.get_session_tx(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match tx.get_last_move_tx(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some() {
                return Err(sqlx::Error::RowNotFound); //same user cannot answer question they asked and previous question must not already be answered
            } else {
                m
            }
        }
        Err(_) => {
            return Err(sqlx::Error::RowNotFound);
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
        let mut res = get_session_state_tx(&mut tx, user_id, info.session_id).await?;
        if res.starting_form.is_none()
            && res.verb.is_some()
            && (res.verb.unwrap() as usize) < verbs.len()
        {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "mfpressedresponse".to_string();
        res.success = true;
        res.mesg = Some("verb *does* have multiple forms".to_string());
        res.verbs = None;

        tx.rollback_tx().await?;

        Ok(res)
    } else {
        let is_correct = false;
        let _res = tx
            .update_answer_move_tx(info, user_id, &correct_answer, is_correct, true, timestamp)
            .await?;

        //if practice session, ask the next here
        if s.challenged_user_id.is_none() {
            ask_practice(&mut tx, prev_form, &s, timestamp, m.asktimestamp, verbs).await?;
        } else {
            //add to other player's score if not practice and not correct
            if !is_correct {
                let user_to_score = if s.challenger_user_id == user_id {
                    "challenged_score"
                } else {
                    "challenger_score"
                };
                let points = 1;
                let _ = tx
                    .add_to_score(info.session_id, user_to_score, points)
                    .await?;
            }
        }

        let mut res = get_session_state_tx(&mut tx, user_id, info.session_id).await?;
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
            res.response_to = "mfpressedresponsepractice".to_string();
        } else {
            res.response_to = "mfpressedresponse".to_string();
        }

        res.success = true;
        res.mesg = Some("verb does not have multiple forms".to_string());
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

pub fn hc_get_available_verbs_practice(
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

pub fn hc_change_verbs(verb_history: &Vec<i32>, reps: usize) -> bool {
    let len = verb_history.len();
    len == 0 || (len >= reps && verb_history[0] == verb_history[reps - 1])
}

async fn ask_practice(
    tx: &mut Box<dyn HcTrx>,
    mut prev_form: HcGreekVerbForm,
    session: &SessionResult,
    timestamp: i64,
    asktimestamp: i64,
    verbs: &[Arc<HcGreekVerb>],
) -> Result<(), sqlx::Error> {
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
        qtype: "ask".to_string(),
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

pub async fn get_sessions_real(
    db: &dyn HcDbTrait,
    user_id: Uuid,
    verbs: &Vec<Arc<HcGreekVerb>>,
    username: Option<String>,
    info: &GetSessions,
) -> Result<SessionsListResponse, sqlx::Error> {
    let mut tx = db.begin_tx().await?;
    let current_session = match info.current_session {
        Some(r) => Some(hc_get_move(&mut tx, user_id, false, r, verbs).await?),
        _ => None,
    };

    Ok(SessionsListResponse {
        response_to: "getsessions".to_string(),
        sessions: hc_get_sessions(&mut tx, user_id).await?,
        success: true,
        username,
        current_session,
    })
}

//opponent_id gets move status for opponent rather than user_id when true:
//we handle the case of s.challenged_user_id.is_none() here, but opponent_id should always be false for practice games
pub async fn hc_get_move(
    tx: &mut Box<dyn HcTrx>,
    user_id: Uuid,
    opponent_id: bool,
    session_id: Uuid,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<SessionState, sqlx::Error> {
    //let mut tx = db.begin_tx().await?;
    let s = tx.get_session_tx(session_id).await?;

    let real_user_id = if !opponent_id || s.challenged_user_id.is_none() {
        user_id
    } else if user_id == s.challenger_user_id {
        s.challenged_user_id.unwrap()
    } else {
        s.challenger_user_id
    };

    let mut res = get_session_state_tx(tx, real_user_id, session_id).await?;

    //set starting_form to 1st pp of verb if verb is set, but starting form is None (i.e. we just changed verbs)
    if res.starting_form.is_none()
        && res.verb.is_some()
        && (res.verb.unwrap() as usize) < verbs.len()
    {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = "getmoves".to_string();
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

fn move_get_type(
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

pub async fn hc_get_sessions(
    tx: &mut Box<dyn HcTrx>,
    user_id: Uuid,
) -> Result<Vec<SessionsListQuery>, sqlx::Error> {
    //let mut db.begin_tx().await?;
    let mut res = tx.get_sessions(user_id).await?;

    for r in &mut res {
        if let Ok(m) = tx.get_last_move_tx(r.session_id).await {
            (r.myturn, r.move_type) = move_get_type(Some(&m), user_id, r.challenged);
        } else {
            (r.myturn, r.move_type) = move_get_type(None, user_id, r.challenged);
        }
        //these were needed to tell whose turn, but no need to send these out to client
        r.challenged = None;
        //r.opponent = None;
    }
    Ok(res)
}

fn get_verbs_by_unit(units: &str, verbs: &[Arc<HcGreekVerb>]) -> Option<String> {
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
    db: &dyn HcDbTrait,
    info: &GetMovesQuery,
) -> Result<Vec<MoveResult>, sqlx::Error> {
    let mut tx = db.begin_tx().await?;
    let res = tx.get_game_moves(info.session_id).await?;

    Ok(res)
}

pub async fn hc_insert_session(
    db: &dyn HcDbTrait,
    user_id: Uuid,
    info: &mut CreateSessionQuery,
    verbs: &[Arc<HcGreekVerb>],
    timestamp: i64,
) -> Result<Uuid, sqlx::Error> {
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
        return Err(sqlx::Error::RowNotFound); //todo oops
    }

    // if custom verbs are set, use them, else change units into verbs
    if info.verbs.is_none() {
        match &info.units {
            Some(u) => info.verbs = get_verbs_by_unit(u, verbs),
            None => return Err(sqlx::Error::RowNotFound),
        }
    }

    // if still no verbs, abort
    if info.verbs.is_none() {
        return Err(sqlx::Error::RowNotFound);
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

    //let mut tx = db.db.begin().await?;
    //let mut tx = db.begin_tx().await.unwrap();
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
                ask_practice(&mut tx, prev_form, &sesh, timestamp, 0, verbs).await?;
            }
            tx.commit_tx().await?;
            Ok(session_uuid)
        }
        Err(e) => Err(e),
    }
}

pub async fn hc_get_available_verbs(
    tx: &mut Box<dyn HcTrx>,
    _user_id: Uuid,
    session_id: Uuid,
    top_unit: Option<i16>,
    verbs: &Vec<Arc<HcGreekVerb>>,
) -> Result<Vec<HCVerbOption>, sqlx::Error> {
    let mut res_verbs: Vec<HCVerbOption> = vec![];
    //let mut tx = db.begin_tx().await?;

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

pub fn load_verbs(_path: &str) -> Vec<Arc<HcGreekVerb>> {
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
text_id, gloss_id, count

pub async fn hc_get_verbs(db: &HcDb, _user_id:Uuid, session_id:Uuid, top_unit:Option<i16>, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<Vec<HCVerbOption>, sqlx::Error> {
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
    use sqlx::postgres::PgPoolOptions;
    use sqlx::Executor;
    use tokio::sync::OnceCell;
    static ONCE: OnceCell<()> = OnceCell::const_new();
    use crate::dbsqlite::HcDbSqlite;
    use db::HcDb;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::SqlitePool;
    use std::str::FromStr;

    async fn get_postgres() -> HcDb {
        HcDb {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect("postgres://jwm:1234@localhost/hctest")
                .await
                .expect("Could not connect to db."),
        }
    }

    async fn _get_sqlite() -> HcDbSqlite {
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
        let db = get_postgres().await;
        //let db = get_sqlite().await;

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

    #[tokio::test]
    async fn test_available() {
        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![], 1);
        a.sort();
        assert_eq!(vec![1, 2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![1], 1);
        a.sort();
        assert_eq!(vec![2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![1, 2], 1);
        a.sort();
        assert_eq!(vec![3], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![3, 1, 2], 1); //skip 3
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![2, 1, 2, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        );
        a.sort();
        assert_eq!(vec![1, 3], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![1, 3, 2, 1, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        );
        a.sort();
        assert_eq!(vec![2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![2, 1, 3, 2, 1, 3, 2, 3, 1, 3, 1, 2],
            1,
        ); //skip 2
        a.sort();
        assert_eq!(vec![1, 3], a);

        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![], 2);
        a.sort();
        assert_eq!(vec![1, 2, 3], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![1, 1], 2);
        a.sort();
        assert_eq!(vec![2, 3], a);
        let mut a =
            hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![1, 1, 2, 2], 2);
        a.sort();
        assert_eq!(vec![3], a);

        let mut a =
            hc_get_available_verbs_practice(&Some("1,2,3".to_string()), &vec![3, 3, 1, 1, 2, 2], 2); //skip 3
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![
                3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 3, 3, 1, 1, 3, 3, 1, 1, 2, 2,
            ],
            2,
        );
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![
                1, 1, 2, 2, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 3, 3, 1, 1, 2, 2,
            ],
            2,
        );
        a.sort();
        assert_eq!(vec![3], a);
        let mut a = hc_get_available_verbs_practice(
            &Some("1,2,3".to_string()),
            &vec![
                2, 2, 1, 1, 3, 3, 2, 2, 1, 1, 3, 3, 2, 2, 3, 3, 1, 1, 3, 3, 1, 1, 2, 2,
            ],
            2,
        ); //skip 2
        a.sort();
        assert_eq!(vec![1, 3], a);

        let mut a = hc_get_available_verbs_practice(&Some("1,2".to_string()), &vec![], 2);
        a.sort();
        assert_eq!(vec![1, 2], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2".to_string()), &vec![1, 1], 2);
        a.sort();
        assert_eq!(vec![2], a);
        let mut a = hc_get_available_verbs_practice(&Some("1,2".to_string()), &vec![1, 1, 2, 2], 2);
        a.sort();
        assert_eq!(vec![2], a);

        let mut a = hc_get_available_verbs_practice(
            &Some("1,2".to_string()),
            &vec![2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2],
            2,
        );
        a.sort();
        assert_eq!(vec![1], a);

        let mut a = hc_get_available_verbs_practice(&Some("1".to_string()), &vec![], 1);
        a.sort();
        assert_eq!(vec![1], a);
        let mut a = hc_get_available_verbs_practice(&Some("1".to_string()), &vec![1, 1], 1);
        a.sort();
        assert_eq!(vec![1], a);
        let mut a = hc_get_available_verbs_practice(&Some("1".to_string()), &vec![1, 1, 1], 1);
        a.sort();
        assert_eq!(vec![1], a);

        let mut a = hc_get_available_verbs_practice(
            &Some("1".to_string()),
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
    async fn test_two_player() {
        initialize_db_once().await;

        let db = get_postgres().await;
        //let db = get_sqlite().await;

        let verbs = load_verbs("pp.txt");

        let mut timestamp = get_timestamp();

        let mut tx = db.begin_tx().await.unwrap();
        let uuid1 = tx
            .create_user("testuser1", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let uuid2 = tx
            .create_user("testuser2", "abcdabcd", "user2@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid = tx
            .create_user("testuser3", "abcdabcd", "user3@blah.com", timestamp)
            .await
            .unwrap();
        tx.commit_tx().await.unwrap();

        let mut csq = CreateSessionQuery {
            qtype: "abc".to_string(),
            name: None,
            verbs: Some("20".to_string()),
            units: None,
            params: None,
            highest_unit: None,
            opponent: "testuser2".to_string(),
            countdown: true,
            practice_reps_per_verb: Some(4),
            max_changes: 4,
            max_time: 30,
        };

        let session_uuid = hc_insert_session(&db, uuid1, &mut csq, &verbs, timestamp).await;
        //assert!(res.is_ok());

        let aq = AskQuery {
            qtype: "ask".to_string(),
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
        let s = hc_get_sessions(&mut tx, uuid1).await;
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
            qtype: "getmove".to_string(),
            session_id: *session_uuid.as_ref().unwrap(),
        };

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some("παιδεύω".to_string()),
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
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some("παιδεύω".to_string()),
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
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        let answerq = AnswerQuery {
            qtype: "abc".to_string(),
            answer: "παιδεύω".to_string(),
            time: "25:01".to_string(),
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
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskTheirTurn,
            myturn: false,
            starting_form: Some("παιδεύω".to_string()),
            answer: Some("παιδεύω".to_string()),
            is_correct: Some(true),
            correct_answer: Some("παιδεύω".to_string()),
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
            time: Some("25:01".to_string()),
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("{:?}", ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskMyTurn,
            myturn: true,
            starting_form: Some("παιδεύω".to_string()),
            answer: Some("παιδεύω".to_string()),
            is_correct: Some(true),
            correct_answer: Some("παιδεύω".to_string()),
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
            time: Some("25:01".to_string()),
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        let aq2 = AskQuery {
            qtype: "ask".to_string(),
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
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert!(ss.is_ok());
        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some("παιδεύω".to_string()),
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
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some("παιδεύω".to_string()),
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
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };
        assert!(ss2.unwrap() == ss_res2);

        //an incorrect answer
        timestamp += 1;
        let answerq = AnswerQuery {
            qtype: "abc".to_string(),
            answer: "παιδ".to_string(),
            time: "25:01".to_string(),
            mf_pressed: false,
            timed_out: false,
            session_id: *session_uuid.as_ref().unwrap(),
        };

        //a valid answer
        let answer = hc_answer(&db, uuid1, &answerq, timestamp, &verbs).await;
        assert!(answer.is_ok());
        assert!(!answer.unwrap().is_correct.unwrap());

        let mut tx = db.begin_tx().await.unwrap();
        let s = hc_get_sessions(&mut tx, uuid1).await;
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
        let s = hc_get_sessions(&mut tx, uuid2).await;
        tx.commit_tx().await.unwrap();

        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::AskTheirTurn);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(1));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(0));

        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::FirstMoveMyTurn,
            myturn: true,
            starting_form: Some("παιδεύω".to_string()),
            answer: Some("παιδ".to_string()),
            is_correct: Some(false),
            correct_answer: Some("παιδεύετε".to_string()),
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
            time: Some("25:01".to_string()),
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: Some(vec![
                /* take out paideuw: HCVerbOption { id: 1, verb: "παιδεύω".to_string() },*/
                HCVerbOption {
                    id: 114,
                    verb: "—, ἀνερήσομαι".to_string(),
                },
                HCVerbOption {
                    id: 115,
                    verb: "—, ἐρήσομαι".to_string(),
                },
                HCVerbOption {
                    id: 30,
                    verb: "ἀγγέλλω".to_string(),
                },
                HCVerbOption {
                    id: 24,
                    verb: "ἄγω".to_string(),
                },
                HCVerbOption {
                    id: 26,
                    verb: "ἀδικέω".to_string(),
                },
                HCVerbOption {
                    id: 74,
                    verb: "αἱρέω".to_string(),
                },
                HCVerbOption {
                    id: 75,
                    verb: "αἰσθάνομαι".to_string(),
                },
                HCVerbOption {
                    id: 111,
                    verb: "αἰσχῡ\u{301}νομαι".to_string(),
                },
                HCVerbOption {
                    id: 36,
                    verb: "ἀκούω".to_string(),
                },
                HCVerbOption {
                    id: 93,
                    verb: "ἁμαρτάνω".to_string(),
                },
                HCVerbOption {
                    id: 84,
                    verb: "ἀναβαίνω".to_string(),
                },
                HCVerbOption {
                    id: 43,
                    verb: "ἀνατίθημι".to_string(),
                },
                HCVerbOption {
                    id: 31,
                    verb: "ἀξιόω".to_string(),
                },
                HCVerbOption {
                    id: 37,
                    verb: "ἀποδέχομαι".to_string(),
                },
                HCVerbOption {
                    id: 44,
                    verb: "ἀποδίδωμι".to_string(),
                },
                HCVerbOption {
                    id: 100,
                    verb: "ἀποθνῄσκω".to_string(),
                },
                HCVerbOption {
                    id: 112,
                    verb: "ἀποκρῑ\u{301}νομαι".to_string(),
                },
                HCVerbOption {
                    id: 101,
                    verb: "ἀποκτείνω".to_string(),
                },
                HCVerbOption {
                    id: 113,
                    verb: "ἀπόλλῡμι".to_string(),
                },
                HCVerbOption {
                    id: 13,
                    verb: "ἄρχω".to_string(),
                },
                HCVerbOption {
                    id: 102,
                    verb: "ἀφῑ\u{301}ημι".to_string(),
                },
                HCVerbOption {
                    id: 121,
                    verb: "ἀφικνέομαι".to_string(),
                },
                HCVerbOption {
                    id: 45,
                    verb: "ἀφίστημι".to_string(),
                },
                HCVerbOption {
                    id: 85,
                    verb: "βαίνω".to_string(),
                },
                HCVerbOption {
                    id: 38,
                    verb: "βάλλω".to_string(),
                },
                HCVerbOption {
                    id: 14,
                    verb: "βλάπτω".to_string(),
                },
                HCVerbOption {
                    id: 103,
                    verb: "βουλεύω".to_string(),
                },
                HCVerbOption {
                    id: 39,
                    verb: "βούλομαι".to_string(),
                },
                HCVerbOption {
                    id: 53,
                    verb: "γίγνομαι".to_string(),
                },
                HCVerbOption {
                    id: 86,
                    verb: "γιγνώσκω".to_string(),
                },
                HCVerbOption {
                    id: 5,
                    verb: "γράφω".to_string(),
                },
                HCVerbOption {
                    id: 122,
                    verb: "δεῖ".to_string(),
                },
                HCVerbOption {
                    id: 61,
                    verb: "δείκνῡμι".to_string(),
                },
                HCVerbOption {
                    id: 40,
                    verb: "δέχομαι".to_string(),
                },
                HCVerbOption {
                    id: 32,
                    verb: "δηλόω".to_string(),
                },
                HCVerbOption {
                    id: 76,
                    verb: "διαφέρω".to_string(),
                },
                HCVerbOption {
                    id: 9,
                    verb: "διδάσκω".to_string(),
                },
                HCVerbOption {
                    id: 46,
                    verb: "δίδωμι".to_string(),
                },
                HCVerbOption {
                    id: 94,
                    verb: "δοκέω".to_string(),
                },
                HCVerbOption {
                    id: 17,
                    verb: "δουλεύω".to_string(),
                },
                HCVerbOption {
                    id: 95,
                    verb: "δύναμαι".to_string(),
                },
                HCVerbOption {
                    id: 10,
                    verb: "ἐθέλω".to_string(),
                },
                HCVerbOption {
                    id: 77,
                    verb: "εἰμί".to_string(),
                },
                HCVerbOption {
                    id: 96,
                    verb: "εἶμι".to_string(),
                },
                HCVerbOption {
                    id: 87,
                    verb: "ἐκπῑ\u{301}πτω".to_string(),
                },
                HCVerbOption {
                    id: 97,
                    verb: "ἐλαύνω".to_string(),
                },
                HCVerbOption {
                    id: 79,
                    verb: "ἔξεστι(ν)".to_string(),
                },
                HCVerbOption {
                    id: 62,
                    verb: "ἐπανίσταμαι".to_string(),
                },
                HCVerbOption {
                    id: 104,
                    verb: "ἐπιβουλεύω".to_string(),
                },
                HCVerbOption {
                    id: 63,
                    verb: "ἐπιδείκνυμαι".to_string(),
                },
                HCVerbOption {
                    id: 98,
                    verb: "ἐπίσταμαι".to_string(),
                },
                HCVerbOption {
                    id: 80,
                    verb: "ἕπομαι".to_string(),
                },
                HCVerbOption {
                    id: 54,
                    verb: "ἔρχομαι".to_string(),
                },
                HCVerbOption {
                    id: 64,
                    verb: "ἐρωτάω".to_string(),
                },
                HCVerbOption {
                    id: 78,
                    verb: "ἔστι(ν)".to_string(),
                },
                HCVerbOption {
                    id: 116,
                    verb: "εὑρίσκω".to_string(),
                },
                HCVerbOption {
                    id: 99,
                    verb: "ἔχω".to_string(),
                },
                HCVerbOption {
                    id: 105,
                    verb: "ζητέω".to_string(),
                },
                HCVerbOption {
                    id: 117,
                    verb: "ἡγέομαι".to_string(),
                },
                HCVerbOption {
                    id: 25,
                    verb: "ἥκω".to_string(),
                },
                HCVerbOption {
                    id: 11,
                    verb: "θάπτω".to_string(),
                },
                HCVerbOption {
                    id: 6,
                    verb: "θῡ\u{301}ω".to_string(),
                },
                HCVerbOption {
                    id: 106,
                    verb: "ῑ\u{314}\u{301}ημι".to_string(),
                },
                HCVerbOption {
                    id: 47,
                    verb: "ἵστημι".to_string(),
                },
                HCVerbOption {
                    id: 48,
                    verb: "καθίστημι".to_string(),
                },
                HCVerbOption {
                    id: 33,
                    verb: "καλέω".to_string(),
                },
                HCVerbOption {
                    id: 49,
                    verb: "καταλῡ\u{301}ω".to_string(),
                },
                HCVerbOption {
                    id: 123,
                    verb: "κεῖμαι".to_string(),
                },
                HCVerbOption {
                    id: 3,
                    verb: "κελεύω".to_string(),
                },
                HCVerbOption {
                    id: 21,
                    verb: "κλέπτω".to_string(),
                },
                HCVerbOption {
                    id: 118,
                    verb: "κρῑ\u{301}νω".to_string(),
                },
                HCVerbOption {
                    id: 18,
                    verb: "κωλῡ\u{301}ω".to_string(),
                },
                HCVerbOption {
                    id: 41,
                    verb: "λαμβάνω".to_string(),
                },
                HCVerbOption {
                    id: 65,
                    verb: "λανθάνω".to_string(),
                },
                HCVerbOption {
                    id: 88,
                    verb: "λέγω".to_string(),
                },
                HCVerbOption {
                    id: 22,
                    verb: "λείπω".to_string(),
                },
                HCVerbOption {
                    id: 4,
                    verb: "λῡ\u{301}ω".to_string(),
                },
                HCVerbOption {
                    id: 55,
                    verb: "μανθάνω".to_string(),
                },
                HCVerbOption {
                    id: 56,
                    verb: "μάχομαι".to_string(),
                },
                HCVerbOption {
                    id: 107,
                    verb: "μέλλω".to_string(),
                },
                HCVerbOption {
                    id: 34,
                    verb: "μένω".to_string(),
                },
                HCVerbOption {
                    id: 57,
                    verb: "μεταδίδωμι".to_string(),
                },
                HCVerbOption {
                    id: 58,
                    verb: "μετανίσταμαι".to_string(),
                },
                HCVerbOption {
                    id: 59,
                    verb: "μηχανάομαι".to_string(),
                },
                HCVerbOption {
                    id: 27,
                    verb: "νῑκάω".to_string(),
                },
                HCVerbOption {
                    id: 89,
                    verb: "νομίζω".to_string(),
                },
                HCVerbOption {
                    id: 119,
                    verb: "οἶδα".to_string(),
                },
                HCVerbOption {
                    id: 81,
                    verb: "ὁράω".to_string(),
                },
                HCVerbOption {
                    id: 66,
                    verb: "παραγίγνομαι".to_string(),
                },
                HCVerbOption {
                    id: 67,
                    verb: "παραδίδωμι".to_string(),
                },
                HCVerbOption {
                    id: 68,
                    verb: "παραμένω".to_string(),
                },
                HCVerbOption {
                    id: 42,
                    verb: "πάσχω".to_string(),
                },
                HCVerbOption {
                    id: 7,
                    verb: "παύω".to_string(),
                },
                HCVerbOption {
                    id: 15,
                    verb: "πείθω".to_string(),
                },
                HCVerbOption {
                    id: 2,
                    verb: "πέμπω".to_string(),
                },
                HCVerbOption {
                    id: 90,
                    verb: "πῑ\u{301}πτω".to_string(),
                },
                HCVerbOption {
                    id: 108,
                    verb: "πιστεύω".to_string(),
                },
                HCVerbOption {
                    id: 28,
                    verb: "ποιέω".to_string(),
                },
                HCVerbOption {
                    id: 19,
                    verb: "πολῑτεύω".to_string(),
                },
                HCVerbOption {
                    id: 16,
                    verb: "πρᾱ\u{301}ττω".to_string(),
                },
                HCVerbOption {
                    id: 91,
                    verb: "προδίδωμι".to_string(),
                },
                HCVerbOption {
                    id: 124,
                    verb: "πυνθάνομαι".to_string(),
                },
                HCVerbOption {
                    id: 109,
                    verb: "συμβουλεύω".to_string(),
                },
                HCVerbOption {
                    id: 82,
                    verb: "συμφέρω".to_string(),
                },
                HCVerbOption {
                    id: 110,
                    verb: "συνῑ\u{301}ημι".to_string(),
                },
                HCVerbOption {
                    id: 120,
                    verb: "σύνοιδα".to_string(),
                },
                HCVerbOption {
                    id: 23,
                    verb: "σῴζω".to_string(),
                },
                HCVerbOption {
                    id: 12,
                    verb: "τάττω".to_string(),
                },
                HCVerbOption {
                    id: 35,
                    verb: "τελευτάω".to_string(),
                },
                HCVerbOption {
                    id: 50,
                    verb: "τίθημι".to_string(),
                },
                HCVerbOption {
                    id: 29,
                    verb: "τῑμάω".to_string(),
                },
                HCVerbOption {
                    id: 125,
                    verb: "τρέπω".to_string(),
                },
                HCVerbOption {
                    id: 69,
                    verb: "τυγχάνω".to_string(),
                },
                HCVerbOption {
                    id: 70,
                    verb: "ὑπακούω".to_string(),
                },
                HCVerbOption {
                    id: 71,
                    verb: "ὑπομένω".to_string(),
                },
                HCVerbOption {
                    id: 126,
                    verb: "φαίνω".to_string(),
                },
                HCVerbOption {
                    id: 83,
                    verb: "φέρω".to_string(),
                },
                HCVerbOption {
                    id: 60,
                    verb: "φεύγω".to_string(),
                },
                HCVerbOption {
                    id: 92,
                    verb: "φημί".to_string(),
                },
                HCVerbOption {
                    id: 72,
                    verb: "φθάνω".to_string(),
                },
                HCVerbOption {
                    id: 51,
                    verb: "φιλέω".to_string(),
                },
                HCVerbOption {
                    id: 52,
                    verb: "φοβέομαι".to_string(),
                },
                HCVerbOption {
                    id: 8,
                    verb: "φυλάττω".to_string(),
                },
                HCVerbOption {
                    id: 73,
                    verb: "χαίρω".to_string(),
                },
                HCVerbOption {
                    id: 20,
                    verb: "χορεύω".to_string(),
                },
                HCVerbOption {
                    id: 127,
                    verb: "χρή".to_string(),
                },
            ]),
        };
        //println!("{:?}\n\n{:?}", ss_res, ss.as_ref().unwrap());
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AskTheirTurn,
            myturn: false,
            starting_form: Some("παιδεύω".to_string()),
            answer: Some("παιδ".to_string()),
            is_correct: Some(false),
            correct_answer: Some("παιδεύετε".to_string()),
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
            time: Some("25:01".to_string()),
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };

        //println!("{:?}", ss2.as_ref().unwrap());
        assert!(ss2.unwrap() == ss_res2);

        //ask new verb after incorrect result
        let aq3 = AskQuery {
            qtype: "ask".to_string(),
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
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert!(ss.is_ok());
        let ss_res = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerTheirTurn,
            myturn: false,
            starting_form: Some("πέμπω".to_string()),
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
            response_to: "getmoves".to_string(),
            success: true,
            mesg: None,
            verbs: None,
        };
        //println!("1: {:?}", ss.as_ref().unwrap());
        //println!("2: {:?}", ss_res);
        assert!(ss.unwrap() == ss_res);

        let mut tx = db.begin_tx().await.unwrap();
        let ss2 = hc_get_move(&mut tx, uuid2, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        let ss_res2 = SessionState {
            session_id: *session_uuid.as_ref().unwrap(),
            move_type: MoveType::AnswerMyTurn,
            myturn: true,
            starting_form: Some("πέμπω".to_string()),
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
            response_to: "getmoves".to_string(),
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
        initialize_db_once().await;
        let verbs = load_verbs("pp.txt");

        let db = get_postgres().await;
        //let db = get_sqlite().await;

        let timestamp = get_timestamp();

        let mut tx = db.begin_tx().await.unwrap();
        let uuid1 = tx
            .create_user("testuser4", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid = tx
            .create_user("testuser6", "abcdabcd", "user3@blah.com", timestamp)
            .await
            .unwrap();
        tx.commit_tx().await.unwrap();

        let mut csq = CreateSessionQuery {
            qtype: "abc".to_string(),
            name: None,
            verbs: Some("20".to_string()),
            units: None,
            params: None,
            highest_unit: None,
            opponent: "".to_string(),
            countdown: true,
            practice_reps_per_verb: Some(4),
            max_changes: 4,
            max_time: 30,
        };

        let session_uuid = hc_insert_session(&db, uuid1, &mut csq, &verbs, timestamp).await;
        //assert!(res.is_ok());

        let aq = AskQuery {
            qtype: "ask".to_string(),
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
        let s = hc_get_sessions(&mut tx, uuid1).await;
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
            qtype: "getmove".to_string(),
            session_id: *session_uuid.as_ref().unwrap(),
        };
        let mut tx = db.begin_tx().await.unwrap();
        let ss = hc_get_move(&mut tx, uuid1, false, m.session_id, &verbs).await;
        tx.commit_tx().await.unwrap();

        assert_eq!(ss.as_ref().unwrap().move_type, MoveType::Practice);
        assert!(ss.as_ref().unwrap().myturn);

        // let ss_res = SessionState {
        //     session_id: *session_uuid.as_ref().unwrap(),
        //     move_type: MoveType::Practice,
        //     myturn: true,
        //     starting_form: Some("παιδεύω".to_string()),
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
        //     response_to: "getmoves".to_string(),
        //     success: true,
        //     mesg: None,
        //     verbs: None,
        // };

        let answerq = AnswerQuery {
            qtype: "abc".to_string(),
            answer: "παιδεύω".to_string(),
            time: "25:01".to_string(),
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

        //let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;
    }
}
