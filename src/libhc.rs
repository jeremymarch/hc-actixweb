use super::*;
use polytonic_greek::hgk_compare_sqlite;

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

        let mut res = db::get_session_state(db, user_id, info.session_id).await?;
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
    
        let mut res = db::get_session_state(db, user_id, info.session_id).await?;
        if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
            res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
        }
        res.response_to = "mfpressedresponse".to_string();
        res.success = true;
        res.mesg = Some("verb does not have multiple forms".to_string());
        res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(&db, user_id, info.session_id, s.highest_unit, &verbs).await.unwrap()) } else { None };
    
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

    //get move seq and add one
    
    let _ = db::insert_ask_move(db, user_id, info.session_id, info.person, info.number, info.tense, info.mood, info.voice, info.verb, timestamp).await?;

    let mut res = db::get_session_state(db, user_id, info.session_id).await?;

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
    let prev_form = HcGreekVerbForm {verb:verbs[idx].clone(), person:HcPerson::from_u8(m.person.unwrap()), number:HcNumber::from_u8(m.number.unwrap()), tense:HcTense::from_u8(m.tense.unwrap()), voice:HcVoice::from_u8(m.voice.unwrap()), mood:HcMood::from_u8(m.mood.unwrap()), gender:None, case:None};

    let correct_answer = prev_form.get_form(false).unwrap().last().unwrap().form.replace(" /", ",");
    let is_correct = hgk_compare_multiple_forms(&correct_answer.replace('/', ","), &info.answer);

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

    let mut res = db::get_session_state(db, user_id, info.session_id).await?;
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "answerresponse".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn && !is_correct { Some(hc_get_available_verbs(&db, user_id, info.session_id, s.highest_unit, &verbs).await.unwrap()) } else { None };

    Ok(res)
}

pub async fn hc_get_move(db: &SqlitePool, user_id:Uuid, info:&GetMoveQuery, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    let s = db::get_session(db, info.session_id).await?;
    let mut res = db::get_session_state(db, user_id, info.session_id).await?;

    //set starting_form to 1st pp of verb if verb is set, but starting form is None (i.e. we just changed verbs)
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = "getmoves".to_string();
    res.success = true;
    res.mesg = None;
    res.verbs = if res.move_type == MoveType::FirstMoveMyTurn { Some(hc_get_available_verbs(&db, user_id, info.session_id, s.highest_unit, &verbs).await.unwrap()) } else {None};

    Ok(res)
}

pub async fn hc_get_sessions(db: &SqlitePool, user_id:Uuid) -> Result<Vec<SessionsListQuery>, sqlx::Error> { 
    db::get_sessions(db, user_id).await
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
