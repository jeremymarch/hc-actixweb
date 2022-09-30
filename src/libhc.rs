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

use super::*;
use polytonic_greek::hgk_compare_sqlite;

pub async fn get_session_state(
    pool: &SqlitePool,
    user_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
) -> Result<SessionState, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let res = db::get_session(pool, session_id).await?;

    let m = db::get_last_two_moves(&mut tx, session_id).await?;
    let first = if !m.is_empty() { Some(&m[0]) } else { None };
    let (myturn, move_type) = move_get_type(first, user_id, res.challenged_user_id);

    let asking_new_verb:bool = move_type == MoveType::FirstMoveMyTurn; //don't old show desc when *asking* a new verb
    let answering_new_verb = m.len() > 1 && m[0].verb_id != m[1].verb_id; //don't show old desc when *answering* a new verb

    let r = SessionState {
        session_id: session_id,
        move_type: move_type,
        myturn: myturn,
        starting_form: if m.len() == 2 && m[0].verb_id == m[1].verb_id { m[1].correct_answer.clone() } else { None },
        answer: if !m.is_empty() { m[0].answer.clone() } else { None },
        is_correct: if !m.is_empty() && m[0].is_correct.is_some() { Some(m[0].is_correct.unwrap() != 0) } else { None },
        correct_answer: if !m.is_empty() { m[0].correct_answer.clone() } else { None },
        verb: if !m.is_empty() { m[0].verb_id } else { None },
        person: if !m.is_empty() { m[0].person } else { None },
        number: if !m.is_empty() { m[0].number } else { None },
        tense: if !m.is_empty() { m[0].tense } else { None },
        voice: if !m.is_empty() { m[0].voice } else { None },
        mood: if !m.is_empty() { m[0].mood } else { None },
        person_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb { m[1].person } else { None },
        number_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb { m[1].number } else { None },
        tense_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb { m[1].tense } else { None },
        voice_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb { m[1].voice } else { None },
        mood_prev: if m.len() == 2 && !asking_new_verb && !answering_new_verb { m[1].mood } else { None },
        time: if !m.is_empty() { m[0].time.clone() } else { None },
        response_to:"".to_string(),
        success:true,
        mesg:None,
        verbs: None,
    };
        
    tx.commit().await?;
    Ok(r)
}

pub async fn hc_mf_pressed(db: &SqlitePool, user_id:Uuid, info:&AnswerQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> {
    let s = db::get_session(db, info.session_id).await?;

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match db::get_last_move(db, info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == user_id {
                return Err(sqlx::Error::RowNotFound);//same user cannot answer question they asked
            }
            else if m.is_correct.is_some() {
                return Err(sqlx::Error::RowNotFound);//previous question must not already be answered
            }
            else {
                m
            }
            },
        Err(_) => { return Err(sqlx::Error::RowNotFound); } //this is first move, nothing to answer
    };

    //test answer to get correct_answer and is_correct
    //let luw = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
    //let luwverb = Arc::new(HcGreekVerb::from_string(1, luw, REGULAR).unwrap());
    let idx = if m.verb_id.is_some() && (m.verb_id.unwrap() as usize) < verbs.len() { m.verb_id.unwrap() as usize } else { 0 };
    let prev_form = HcGreekVerbForm {verb:verbs[idx].clone(), person:HcPerson::from_u8(m.person.unwrap()), number:HcNumber::from_u8(m.number.unwrap()), tense:HcTense::from_u8(m.tense.unwrap()), voice:HcVoice::from_u8(m.voice.unwrap()), mood:HcMood::from_u8(m.mood.unwrap()), gender:None, case:None};

    let correct_answer = prev_form.get_form(false).unwrap().last().unwrap().form.replace(" /", ",");

    if correct_answer.contains(',') {

        let mut res = get_session_state(db, user_id, info.session_id).await?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "mfpressedresponse".to_string();
        res.success = true;
        res.mesg = Some("verb does have multiple forms".to_string());
        res.verbs = None;

        Ok(res)
    }
    else {
        let is_correct = false;
        let _res = update_answer_move(
            db,
            info.session_id,
            user_id,
            &info.answer,
            &correct_answer,
            is_correct,
            &info.time,
            true,
            info.timed_out,
            timestamp).await?;
    
        let mut res = get_session_state(db, user_id, info.session_id).await?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "mfpressedresponse".to_string();
        res.success = true;
        res.mesg = Some("verb does not have multiple forms".to_string());
        res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(db, user_id, info.session_id, s.highest_unit, verbs).await.unwrap()) } else { None };
    
        Ok(res)
    }
}

pub async fn hc_ask(db: &SqlitePool, user_id:Uuid, info:&AskQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    //todo check that user_id == challenger_user_id if this is first move

    let s = db::get_session(db, info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }
    
    //prevent out-of-sequence asks
    match db::get_last_move(db, info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == user_id {
                return Err(sqlx::Error::RowNotFound);//same user cannot ask twice in a row
            }
            else if m.answer_user_id != Some(user_id) {
                return Err(sqlx::Error::RowNotFound);//ask user must be same as previous answer user
            }
            else if m.is_correct.is_none() {
                return Err(sqlx::Error::RowNotFound);//previous answer must be marked correct or incorrect
            }
         },
        Err(_) => () //this is first move, nothing to check
    }

    //get move seq and add one?
    
    let _ = db::insert_ask_move(db, Some(user_id), info.session_id, info.person, info.number, info.tense, info.mood, info.voice, info.verb, timestamp).await?;

    let mut res = get_session_state(db, user_id, info.session_id).await?;

    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "ask".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = None;

    Ok(res)
}

pub async fn hc_answer(db: &SqlitePool, user_id:Uuid, info:&AnswerQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    //todo check that user_id is either challenger_user_id or challenged_user_id
    let s = db::get_session(db, info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match db::get_last_move(db, info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == user_id {
                return Err(sqlx::Error::RowNotFound);//same user cannot answer question they asked
            }
            else if m.is_correct.is_some() {
                return Err(sqlx::Error::RowNotFound);//previous question must not already be answered
            }
            else {
                m
            }
         },
        Err(_) => { return Err(sqlx::Error::RowNotFound); } //this is first move, nothing to answer
    };

    //test answer to get correct_answer and is_correct
    //let luw = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
    //let luwverb = Arc::new(HcGreekVerb::from_string(1, luw, REGULAR).unwrap());
    let idx = if m.verb_id.is_some() && (m.verb_id.unwrap() as usize) < verbs.len() { m.verb_id.unwrap() as usize } else { 0 };
    let mut prev_form = HcGreekVerbForm {verb:verbs[idx].clone(), person:HcPerson::from_u8(m.person.unwrap()), number:HcNumber::from_u8(m.number.unwrap()), tense:HcTense::from_u8(m.tense.unwrap()), voice:HcVoice::from_u8(m.voice.unwrap()), mood:HcMood::from_u8(m.mood.unwrap()), gender:None, case:None};

    let correct_answer_result = prev_form.get_form(false);
    let correct_answer = match correct_answer_result {
        Ok(a) => a.last().unwrap().form.replace(" /", ","),
        Err(_) => "—".to_string(),
    };
    let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));

    let _res = update_answer_move(
        db,
        info.session_id,
        user_id,
        &info.answer,
        &correct_answer,
        is_correct,
        &info.time,
        info.mf_pressed,
        info.timed_out,
        timestamp).await?;

    //for practice sessions we should do the ask here
    if s.challenged_user_id.is_none() {
        let persons = vec![HcPerson::First, HcPerson::Second, HcPerson::Third];
        let numbers = vec![HcNumber::Singular, HcNumber::Plural];
        let tenses = vec![HcTense::Present, HcTense::Imperfect, HcTense::Future, HcTense::Aorist, HcTense::Perfect, HcTense::Pluperfect];
        let moods = vec![HcMood::Indicative, HcMood::Subjunctive, HcMood::Optative, HcMood::Imperative];
        let voices = vec![HcVoice::Active, HcVoice::Middle, HcVoice::Passive];

        //a = HcGreekVerbForm { verb: verbs[idx].clone(), person, number, tense, voice, mood, gender: None, case: None};


        prev_form.change_params(2, &persons, &numbers, &tenses, &voices, &moods);

        // let person = *persons.choose(&mut rand::thread_rng()).unwrap();
        // let number = *numbers.choose(&mut rand::thread_rng()).unwrap();
        // let tense = *tenses.choose(&mut rand::thread_rng()).unwrap();
        // let voice = *voices.choose(&mut rand::thread_rng()).unwrap();
        // let mood = *moods.choose(&mut rand::thread_rng()).unwrap();
        //ask
        let _ = db::insert_ask_move(db, None, info.session_id, prev_form.person.to_u8(), prev_form.number.to_u8(), prev_form.tense.to_u8(), 
            prev_form.mood.to_u8(), prev_form.voice.to_u8(), prev_form.verb.id, timestamp + 1).await?;
    }

    let mut res = get_session_state(db, user_id, info.session_id).await?;
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "answerresponse".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(db, user_id, info.session_id, s.highest_unit, verbs).await.unwrap()) } else { None };

    Ok(res)
}

pub async fn hc_get_move(db: &SqlitePool, user_id:Uuid, info:&GetMoveQuery, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    let s = db::get_session(db, info.session_id).await?;
    let mut res = get_session_state(db, user_id, info.session_id).await?;

    //set starting_form to 1st pp of verb if verb is set, but starting form is None (i.e. we just changed verbs)
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = "getmoves".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn { Some(hc_get_available_verbs(db, user_id, info.session_id, s.highest_unit, verbs).await.unwrap()) } else {None};

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
fn move_get_type(s:Option<&MoveResult>, user_id:Uuid, challenged_id:Option<Uuid>) -> (bool, MoveType) {
    let myturn:bool;
    let move_type:MoveType;

    let change_verb_on_incorrect = true;

    match s {
        Some(s) => { 
            if challenged_id.is_none() {
                myturn = true;
                move_type = MoveType::Practice; //practice, my turn always
            }
            else if s.ask_user_id == user_id { 
                if s.answer_user_id.is_some() { //xxxanswered, my turn to ask | I asked, they answered, their turn to ask (waiting for them to ask)
                    myturn = false;
                    move_type = MoveType::AskTheirTurn;
                }
                else {
                    myturn = false; //unanswered, their turn to answer
                    move_type = MoveType::AnswerTheirTurn;
                }
            } else { 
                if s.answer_user_id.is_some() { //xxxanswered, their turn to ask | they asked, I answered, my turn to ask
                    myturn = true;
                    
                    if change_verb_on_incorrect && s.is_correct.is_some() && s.is_correct.unwrap() == 0 {
                        move_type = MoveType::FirstMoveMyTurn; //user must ask a new verb because answered incorrectly
                    }
                    else {
                        move_type = MoveType::AskMyTurn;
                    }
                }
                else {
                    myturn = true; //unanswered, my turn to answer
                    move_type = MoveType::AnswerMyTurn;
                } 
            } 
        },
        None => {
            if let Some(cid) = challenged_id {
                if cid == user_id {
                    myturn = false;
                    move_type = MoveType::FirstMoveTheirTurn; //no moves yet, their turn to ask
                } 
                else {
                    myturn = true;
                    move_type = MoveType::FirstMoveMyTurn; //no moves yet, my turn to ask
                }
            }
            else {
                myturn = true;
                move_type = MoveType::Practice; //practice, my turn always (no moves yet)
            }
        },
    }
    (myturn, move_type)
}

pub async fn hc_get_sessions(db: &SqlitePool, user_id:Uuid) -> Result<Vec<SessionsListQuery>, sqlx::Error> { 
    let mut res = db::get_sessions(db, user_id).await?;

    for r in &mut res {
        if let Ok(m) = db::get_last_move(db, r.session_id).await {
            (r.myturn, r.move_type) = move_get_type(Some(&m), user_id, r.challenged);
        }
        else {
            (r.myturn, r.move_type) = move_get_type(None, user_id, r.challenged);
        }
    }
    Ok(res) 
}

pub async fn hc_insert_session(db: &SqlitePool, user_id:Uuid, info:&CreateSessionQuery, timestamp:i64) -> Result<Uuid, sqlx::Error> { 
    let opponent_user_id:Option<Uuid>;
    if !info.opponent.is_empty() {
        let o = db::get_user_id(db, &info.opponent).await?; //we want to return an error if len of info.opponent > 0 and not found, else it is practice game
        opponent_user_id = Some(o.user_id);
    }
    else {
        opponent_user_id = None;
    }

    //failed to find opponent or opponent is self
    if opponent_user_id.is_some() && opponent_user_id.unwrap() == user_id {
        return Err(sqlx::Error::RowNotFound); //todo oops
    }

    let highest_unit = if let Ok(v) = info.unit.parse::<u32>() { Some(v) } else { None };
    let max_changes = 2;

    match db::insert_session(db, user_id, highest_unit, opponent_user_id, max_changes, timestamp).await {
        Ok(session_uuid) => {
            Ok(session_uuid)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn hc_get_available_verbs(db: &SqlitePool, _user_id:Uuid, session_id:Uuid, top_unit:Option<u32>, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<Vec<HCVerbOption>, sqlx::Error> { 
    let mut res_verbs:Vec<HCVerbOption> = vec![];

    let used_verbs = db::get_used_verbs(db, session_id).await?;

    for v in verbs {
        if top_unit.is_none() || v.hq_unit <= top_unit.unwrap() && !used_verbs.contains(&v.id)  { //&& verb_id_not_used()
            let newv = HCVerbOption {
                id: v.id,
                verb: if v.pps[0] == "—" { format!("—, {}", v.pps[1]) } else { v.pps[0].clone() },
            };
            res_verbs.push(newv);
        }
    }

    res_verbs.sort_by(|a,b| hgk_compare_sqlite(&a.verb,&b.verb));
    Ok(res_verbs)
}
