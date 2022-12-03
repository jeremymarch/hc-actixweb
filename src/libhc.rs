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
use sqlx::Postgres;

pub async fn get_session_state(
    db: &HcSqliteDb,
    user_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
) -> Result<SessionState, sqlx::Error> {
    let mut tx = db.db.begin().await?;

    let r = get_session_state_tx(
       &mut tx,
       db,
        user_id,
        session_id,
    ).await?;
        
    tx.commit().await?;
    Ok(r)
}

pub async fn get_session_state_tx<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, Postgres>,
    db: &HcSqliteDb,
    user_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
) -> Result<SessionState, sqlx::Error> {

    let res = db.get_session_tx(&mut *tx, session_id).await?;
    let m = db.get_last_n_moves(&mut *tx, session_id, 2).await?;

    let first = if !m.is_empty() { Some(&m[0]) } else { None };
    let (myturn, move_type) = move_get_type(first, user_id, res.challenged_user_id);

    let asking_new_verb:bool = move_type == MoveType::FirstMoveMyTurn; //don't old show desc when *asking* a new verb
    let answering_new_verb = m.len() > 1 && m[0].verb_id != m[1].verb_id; //don't show old desc when *answering* a new verb

    let r = SessionState {
        session_id,
        move_type,
        myturn,
        starting_form: if m.len() == 2 && m[0].verb_id == m[1].verb_id { m[1].correct_answer.clone() } else { None },
        answer: if !m.is_empty() { m[0].answer.clone() } else { None },
        is_correct: if !m.is_empty() && m[0].is_correct.is_some() { Some(m[0].is_correct.unwrap()) } else { None },
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
        
    Ok(r)
}

pub async fn hc_ask(db: &HcSqliteDb, user_id:Uuid, info:&AskQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> {
    //todo check that user_id is either challenger_user_id or challenged_user_id
    //todo check that user_id == challenger_user_id if this is first move

    let s = db.get_session(info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }
    
    //prevent out-of-sequence asks
    let m = match db.get_last_move(info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.answer_user_id != Some(user_id) || m.is_correct.is_none(){
                return Err(sqlx::Error::RowNotFound);//same user cannot ask twice in a row and ask user must be same as previous answer user and previous answer must be marked correct or incorrect
            }
            else {
                Ok(m)
            }
         },
        Err(m) => Err(m) //this is first move, nothing to check
    };

    //be sure this asktimestamp is at least one greater than previous, if there was a previous one
    let new_time_stamp = if m.is_ok() && timestamp <= m.as_ref().unwrap().asktimestamp { m.unwrap().asktimestamp + 1 } else { timestamp };

    //get move seq and add one?
    
    let _ = db.insert_ask_move(Some(user_id), info, new_time_stamp).await?;

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

pub async fn hc_answer(db: &HcSqliteDb, user_id:Uuid, info:&AnswerQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    //todo check that user_id is either challenger_user_id or challenged_user_id
    let mut tx = db.db.begin().await?;

    let s = db.get_session_tx(&mut tx, info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match db.get_last_move_tx(&mut tx, info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some() {
                return Err(sqlx::Error::RowNotFound); //same user cannot answer question they asked and previous question must not already be answered
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
    let prev_form = HcGreekVerbForm {verb:verbs[idx].clone(), person:HcPerson::from_i16(m.person.unwrap()), number:HcNumber::from_i16(m.number.unwrap()), tense:HcTense::from_i16(m.tense.unwrap()), voice:HcVoice::from_i16(m.voice.unwrap()), mood:HcMood::from_i16(m.mood.unwrap()), gender:None, case:None};

    let correct_answer_result = prev_form.get_form(false);
    let correct_answer = match correct_answer_result {
        Ok(a) => a.last().unwrap().form.replace(" /", ","),
        Err(_) => "—".to_string(),
    };
    let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));

    let _res = db.update_answer_move_tx(&mut tx, 
        info,
        user_id,
        &correct_answer,
        is_correct,
        info.mf_pressed,
        timestamp).await?;

    //if practice session, ask the next here
    if s.challenged_user_id.is_none() {
        ask_practice(&mut tx, db, info.session_id, prev_form, timestamp, m.asktimestamp).await?;
    }
    else {
        //add to other player's score if not practice and not correct
        if !is_correct {
            let user_to_score = if s.challenger_user_id == user_id { "challenged_score" } else { "challenger_score" };
            let points = 1;
            let _ = db.add_to_score(&mut tx, info.session_id, user_to_score, points).await?;
        }
    }
    
    let mut res = get_session_state_tx(&mut tx, db, user_id, info.session_id).await?;
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    tx.commit().await?;

    //if practice session, add in is_correct and correct_answer back into session state here
    if s.challenged_user_id.is_none() {
        res.is_correct = Some(is_correct);
        res.correct_answer = Some(correct_answer);
        res.response_to = "answerresponsepractice".to_string();
    }
    else {
        res.response_to = "answerresponse".to_string();
    }

    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(db, user_id, info.session_id, s.highest_unit, verbs).await.unwrap()) } else { None };

    Ok(res)
}

pub async fn hc_mf_pressed(db: &HcSqliteDb, user_id:Uuid, info:&AnswerQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> {
    let mut tx = db.db.begin().await?;
    
    let s = db.get_session_tx(&mut tx, info.session_id).await?;
    if user_id != s.challenger_user_id && Some(user_id) != s.challenged_user_id {
        return Err(sqlx::Error::RowNotFound);
    }

    //pull prev move from db to get verb and params and to prevent out-of-sequence answers
    let m = match db.get_last_move_tx(&mut tx, info.session_id).await {
        Ok(m) => {
            if m.ask_user_id == Some(user_id) || m.is_correct.is_some(){
                return Err(sqlx::Error::RowNotFound); //same user cannot answer question they asked and previous question must not already be answered
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
    let prev_form = HcGreekVerbForm {verb:verbs[idx].clone(), person:HcPerson::from_i16(m.person.unwrap()), number:HcNumber::from_i16(m.number.unwrap()), tense:HcTense::from_i16(m.tense.unwrap()), voice:HcVoice::from_i16(m.voice.unwrap()), mood:HcMood::from_i16(m.mood.unwrap()), gender:None, case:None};

    let correct_answer = prev_form.get_form(false).unwrap().last().unwrap().form.replace(" /", ",");

    if correct_answer.contains(',') {

        let mut res = get_session_state(db, user_id, info.session_id).await?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "mfpressedresponse".to_string();
        res.success = true;
        res.mesg = Some("verb *does* have multiple forms".to_string());
        res.verbs = None;

        tx.rollback().await?;

        Ok(res)
    }
    else {
        let is_correct = false;
        let _res = db.update_answer_move(
            info,
            user_id,
            &correct_answer,
            is_correct,
            true,
            timestamp).await?;

        //if practice session, ask the next here
        if s.challenged_user_id.is_none() {
            ask_practice(&mut tx, db, info.session_id, prev_form, timestamp, m.asktimestamp).await?;
        }
        else {
            //add to other player's score if not practice and not correct
            if !is_correct {
                let user_to_score = if s.challenger_user_id == user_id { "challenged_score" } else { "challenger_score" };
                let points = 1;
                let _ = db.add_to_score(&mut tx, info.session_id, user_to_score, points).await?;
            }
        }

        let mut res = get_session_state_tx(&mut tx, db, user_id, info.session_id).await?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }

        tx.commit().await?;

        //if practice session, add in is_correct and correct_answer back into session state here
        if s.challenged_user_id.is_none() {
            res.is_correct = Some(is_correct);
            res.correct_answer = Some(correct_answer);
            res.response_to = "mfpressedresponsepractice".to_string();
        }
        else {
            res.response_to = "mfpressedresponse".to_string();
        }

        res.success = true;
        res.mesg = Some("verb does not have multiple forms".to_string());
        res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(db, user_id, info.session_id, s.highest_unit, verbs).await.unwrap()) } else { None };
    
        Ok(res)
    }
}

async fn ask_practice<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, Postgres>, db: &HcSqliteDb, session_id:Uuid, prev_form:HcGreekVerbForm, timestamp:i64, asktimestamp:i64) -> Result<(), sqlx::Error> {
    let persons = vec![HcPerson::First, HcPerson::Second, HcPerson::Third];
    let numbers = vec![HcNumber::Singular, HcNumber::Plural];
    let tenses = vec![HcTense::Present, HcTense::Imperfect, HcTense::Future, HcTense::Aorist, HcTense::Perfect, HcTense::Pluperfect];
    let moods = vec![HcMood::Indicative, HcMood::Subjunctive, HcMood::Optative, HcMood::Imperative];
    let voices = vec![HcVoice::Active, HcVoice::Middle, HcVoice::Passive];

    //a = HcGreekVerbForm { verb: verbs[idx].clone(), person, number, tense, voice, mood, gender: None, case: None};

    let mut pf:HcGreekVerbForm;
    loop {
        pf = prev_form.clone();

        pf.change_params(2, &persons, &numbers, &tenses, &voices, &moods);
        if let Ok(_ff) = pf.get_form(false) {
            break;
        }
    }

    //be sure this asktimestamp is at least one greater than previous one
    let new_time_stamp = if timestamp > asktimestamp { timestamp } else { asktimestamp + 1 };
    //ask
    let aq = AskQuery {
        qtype: "ask".to_string(),
        session_id,
        person: pf.person.to_i16(),
        number: pf.number.to_i16(),
        tense: pf.tense.to_i16(),
        voice: pf.voice.to_i16(),
        mood: pf.mood.to_i16(),
        verb: pf.verb.id as i32,
    };
    let _ = db.insert_ask_move_tx(tx, None, &aq, new_time_stamp).await?;
    Ok(())
}

//opponent_id gets move status for opponent rather than user_id when true:
//we handle the case of s.challenged_user_id.is_none() here, but opponent_id should always be false for practice games
pub async fn hc_get_move(db: &HcSqliteDb, user_id:Uuid, opponent_id:bool, session_id:Uuid, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    let s = db.get_session(session_id).await?;

    let real_user_id = if !opponent_id || s.challenged_user_id.is_none() { user_id } else if user_id == s.challenger_user_id { s.challenged_user_id.unwrap() } else { s.challenger_user_id };

    let mut res = get_session_state(db, real_user_id, session_id).await?;

    //set starting_form to 1st pp of verb if verb is set, but starting form is None (i.e. we just changed verbs)
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = "getmoves".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn { Some(hc_get_available_verbs(db, real_user_id, session_id, s.highest_unit, verbs).await.unwrap()) } else {None};

    Ok(res)
}

fn move_get_type(s:Option<&MoveResult>, user_id:Uuid, challenged_id:Option<Uuid>) -> (bool, MoveType) {
    let myturn:bool;
    let move_type:MoveType;

    let change_verb_on_incorrect = true;

    match s {
        Some(s) => { 
            #[allow(clippy::collapsible_else_if)]
            if challenged_id.is_none() {
                myturn = true;
                move_type = MoveType::Practice; //practice, my turn always
            }
            else if s.ask_user_id == Some(user_id) { 
                if s.answer_user_id.is_some() { //xxxanswered, my turn to ask | I asked, they answered, their turn to ask (waiting for them to ask)
                    myturn = false;
                    move_type = MoveType::AskTheirTurn;
                }
                else {
                    myturn = false; //unanswered, their turn to answer
                    move_type = MoveType::AnswerTheirTurn;
                }
            }
            else { 
                if s.answer_user_id.is_some() { //xxxanswered, their turn to ask | they asked, I answered, my turn to ask
                    myturn = true;
                    
                    if change_verb_on_incorrect && s.is_correct.is_some() && !s.is_correct.unwrap() {
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

pub async fn hc_get_sessions(db: &HcSqliteDb, user_id:Uuid) -> Result<Vec<SessionsListQuery>, sqlx::Error> { 
    let mut res = db.get_sessions(user_id).await?;

    for r in &mut res {
        if let Ok(m) = db.get_last_move(r.session_id).await {
            (r.myturn, r.move_type) = move_get_type(Some(&m), user_id, r.challenged);
        }
        else {
            (r.myturn, r.move_type) = move_get_type(None, user_id, r.challenged);
        }
        //these were needed to tell whose turn, but no need to send these out to client
        r.challenged = None;
        //r.opponent = None;
    }
    Ok(res) 
}

pub async fn hc_insert_session(db: &HcSqliteDb, user_id:Uuid, info:&CreateSessionQuery, verbs:&[Arc<HcGreekVerb>], timestamp:i64) -> Result<Uuid, sqlx::Error> { 
    let opponent_user_id:Option<Uuid>;
    if !info.opponent.is_empty() {
        let o = db.get_user_id(&info.opponent).await?; //we want to return an error if len of info.opponent > 0 and not found, else it is practice game
        opponent_user_id = Some(o.user_id);
    }
    else {
        opponent_user_id = None;
    }

    //failed to find opponent or opponent is self
    if opponent_user_id.is_some() && opponent_user_id.unwrap() == user_id {
        return Err(sqlx::Error::RowNotFound); //todo oops
    }

    let highest_unit = if let Ok(v) = info.unit.parse::<i16>() { Some(v) } else { None };
    let max_changes = 2;

    match db.insert_session(user_id, highest_unit, opponent_user_id, max_changes, info.practice_reps_per_verb, timestamp).await {
        Ok(session_uuid) => {
            //for practice sessions we should do the ask here
            if opponent_user_id.is_none() {
                let persons = vec![HcPerson::First, HcPerson::Second, HcPerson::Third];
                let numbers = vec![HcNumber::Singular, HcNumber::Plural];
                let tenses = vec![HcTense::Present, HcTense::Imperfect, HcTense::Future, HcTense::Aorist, HcTense::Perfect, HcTense::Pluperfect];
                let moods = vec![HcMood::Indicative, HcMood::Subjunctive, HcMood::Optative, HcMood::Imperative];
                let voices = vec![HcVoice::Active, HcVoice::Middle, HcVoice::Passive];

                let idx = 0;

                let mut prev_form = HcGreekVerbForm { verb: verbs[idx].clone(), person:HcPerson::First, number:HcNumber::Singular, tense:HcTense::Present, voice:HcVoice::Active, mood:HcMood::Indicative, gender: None, case: None};
                loop {
                    prev_form.change_params(2, &persons, &numbers, &tenses, &voices, &moods);
                    if let Ok(_ff) = prev_form.get_form(false) {
                        break;
                    }
                }

                //ask
                let aq = AskQuery {
                    qtype: "ask".to_string(),
                    session_id: session_uuid,
                    person: prev_form.person.to_i16(),
                    number: prev_form.number.to_i16(),
                    tense: prev_form.tense.to_i16(),
                    voice: prev_form.voice.to_i16(),
                    mood: prev_form.mood.to_i16(),
                    verb: prev_form.verb.id as i32,
                };
                let _ = db.insert_ask_move(None, &aq, timestamp + 1).await?;
            }
            Ok(session_uuid)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn hc_get_available_verbs(db: &HcSqliteDb, _user_id:Uuid, session_id:Uuid, top_unit:Option<i16>, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<Vec<HCVerbOption>, sqlx::Error> { 
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

/*
text_id, gloss_id, count

pub async fn hc_get_verbs(db: &HcSqliteDb, _user_id:Uuid, session_id:Uuid, top_unit:Option<i16>, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<Vec<HCVerbOption>, sqlx::Error> { 
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
