use super::*;

pub async fn hc_ask(db: &SqlitePool, user_id:Uuid, info:&AskQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> {
    let _ = db::insert_ask_move(&db, user_id, info.session_id, info.person, info.number, info.tense, info.mood, info.voice, info.verb, timestamp).await?;

    let mut res = db::get_session_state(&db, user_id, info.session_id).await?;

    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "ask".to_string();
    res.success = true;
    res.mesg = None;

    Ok(res)
}

pub async fn hc_answer(db: &SqlitePool, user_id:Uuid, info:&AnswerQuery, timestamp:i64, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
    //pull prev move from db to get verb and params
    let m = db::get_last_move(&db, info.session_id).await?;

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

    let mut res = db::get_session_state(&db, user_id, info.session_id).await?;
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }
    res.response_to = "answerresponse".to_string();
    res.success = true;
    res.mesg = None;

    Ok(res)
}

pub async fn hc_get_move(db: &SqlitePool, user_id:Uuid, info:&GetMoveQuery, verbs:&Vec<Arc<HcGreekVerb>>) -> Result<SessionState, sqlx::Error> { 
let mut res = db::get_session_state(&db, user_id, info.session_id).await?;
    if res.starting_form.is_none() && res.verb.is_some() && (res.verb.unwrap() as usize) < verbs.len() {
        res.starting_form = Some(verbs[res.verb.unwrap() as usize].pps[0].to_string());
    }

    res.response_to = "getmoves".to_string();
    res.success = true;
    res.mesg = None;

    Ok(res)
}

pub async fn hc_get_sessions(db: &SqlitePool, user_id:Uuid) -> Result<Vec<SessionsListQuery>, sqlx::Error> { 
    db::get_sessions(&db, user_id).await
}

pub async fn hc_insert_session(db: &SqlitePool, user_id:Uuid, info:&CreateSessionQuery, timestamp:i64) -> Result<Uuid, sqlx::Error> { 
    let mut opponent_user_id:Option<Uuid> = None;
    if info.opponent.len() > 0 {
        let o = db::get_user_id(&db, &info.opponent).await?; //we want to return an error if len of info.opponent > 0 and not found, else it is practice game
        opponent_user_id = Some(o.user_id);
    }
    else {
        opponent_user_id = None;
    }

    //failed to find opponent or opponent is self
    if opponent_user_id.is_some() && opponent_user_id.unwrap() == user_id {
        return Err(sqlx::Error::RowNotFound); //todo oops
    }

    let unit = if let Ok(v) = info.unit.parse::<u32>() { Some(v) } else { None };

    match db::insert_session(&db, user_id, unit, opponent_user_id, timestamp).await {
        Ok(session_uuid) => {
            Ok(session_uuid)
        },
        Err(e) => {
            Err(e)
        }
    }
}
