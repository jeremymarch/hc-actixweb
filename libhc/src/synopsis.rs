use crate::hgk_compare_multiple_forms;
use crate::HcDb;

use hoplite_verbs_rs::check_pps;
use hoplite_verbs_rs::HcCase;
use hoplite_verbs_rs::HcGender;
use hoplite_verbs_rs::HcGreekVerb;
use hoplite_verbs_rs::HcGreekVerbForm;
use hoplite_verbs_rs::HcMood;
use hoplite_verbs_rs::HcNumber;
use hoplite_verbs_rs::HcPerson;
use hoplite_verbs_rs::HcTense;
use hoplite_verbs_rs::HcVerbForms;
use hoplite_verbs_rs::HcVoice;

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;
//use crate::synopsis::polytonic_greek::hgk_compare_multiple_forms;
//use chrono::LocalResult;
//use hoplite_verbs_rs::*;
use itertools::Itertools;
use sqlx::FromRow;
use std::sync::Arc;

use crate::dbpostgres::HcDbPostgres;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynopsisSaverRequest {
    pub advisor: String,
    pub unit: i32,
    pub sname: String,
    pub number: i32,
    pub person: i32,
    pub pp: String,
    pub pp_correct: String,
    pub pp_is_correct: String,
    pub ptccase: Option<i32>,
    pub ptcgender: Option<i32>,
    pub ptcnumber: Option<i32>,
    pub r: Vec<String>,
    pub verb: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynopsisResultRequest {
    pub id: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct LatinSynopsisResult {
    pub id: i64,
    pub updated: i64,
    pub sname: String,
    pub advisor: String,
    pub sgiday: i64,
    pub selectedverb: String,
    pub pp: String,
    pub verbnumber: String,
    pub verbperson: String,
    pub verbptcgender: String,
    pub verbptcnumber: String,
    pub verbptccase: String,
    pub ip: String,
    pub ua: String,
    pub status: i64,
    pub f0: String,
    pub f1: String,
    pub f2: String,
    pub f3: String,
    pub f4: String,
    pub f5: String,
    pub f6: String,
    pub f7: String,
    pub f8: String,
    pub f9: String,
    pub f10: String,
    pub f11: String,
    pub f12: String,
    pub f13: String,
    pub f14: String,
    pub f15: String,
    pub f16: String,
    pub f17: String,
    pub f18: String,
    pub f19: String,
    pub f20: String,
    pub f21: String,
    pub f22: String,
    pub f23: String,
    pub f24: String,
    pub f25: String,
    pub f26: String,
    pub f27: String,
    pub f28: String,
    pub f29: String,
    pub f30: String,
    pub f31: String,
    pub f32: String,
    pub f33: String,
    pub f34: String,
    pub f35: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct GreekSynopsisResult {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub updated: Option<chrono::NaiveDateTime>,
    pub sname: String,
    pub advisor: String,
    pub sgiday: i32,
    pub selectedverb: String,
    pub pp: String,
    pub pp_correct: String,
    pub pp_is_correct: String,
    pub verbnumber: String,
    pub verbperson: String,
    pub verbptcgender: Option<String>,
    pub verbptcnumber: Option<String>,
    pub verbptccase: Option<String>,
    pub ip: String,
    pub ua: String,
    pub status: i32,
    pub score: String,
    pub f0: String,
    pub a0: String,
    pub c0: bool,
    pub f1: String,
    pub a1: String,
    pub c1: bool,
    pub f2: String,
    pub a2: String,
    pub c2: bool,
    pub f3: String,
    pub a3: String,
    pub c3: bool,
    pub f4: String,
    pub a4: String,
    pub c4: bool,
    pub f5: String,
    pub a5: String,
    pub c5: bool,
    pub f6: String,
    pub a6: String,
    pub c6: bool,
    pub f7: String,
    pub a7: String,
    pub c7: bool,
    pub f8: String,
    pub a8: String,
    pub c8: bool,
    pub f9: String,
    pub a9: String,
    pub c9: bool,
    pub f10: String,
    pub a10: String,
    pub c10: bool,
    pub f11: String,
    pub a11: String,
    pub c11: bool,
    pub f12: String,
    pub a12: String,
    pub c12: bool,
    pub f13: String,
    pub a13: String,
    pub c13: bool,
    pub f14: String,
    pub a14: String,
    pub c14: bool,
    pub f15: String,
    pub a15: String,
    pub c15: bool,
    pub f16: String,
    pub a16: String,
    pub c16: bool,
    pub f17: String,
    pub a17: String,
    pub c17: bool,
    pub f18: String,
    pub a18: String,
    pub c18: bool,
    pub f19: String,
    pub a19: String,
    pub c19: bool,
    pub f20: String,
    pub a20: String,
    pub c20: bool,
    pub f21: String,
    pub a21: String,
    pub c21: bool,
    pub f22: String,
    pub a22: String,
    pub c22: bool,
    pub f23: String,
    pub a23: String,
    pub c23: bool,
    pub f24: String,
    pub a24: String,
    pub c24: bool,
    pub f25: String,
    pub a25: String,
    pub c25: bool,
    pub f26: String,
    pub a26: String,
    pub c26: bool,
    pub f27: String,
    pub a27: String,
    pub c27: bool,
    pub f28: String,
    pub a28: String,
    pub c28: bool,
    pub f29: String,
    pub a29: String,
    pub c29: bool,
    pub f30: String,
    pub a30: String,
    pub c30: bool,
    pub f31: String,
    pub a31: String,
    pub c31: bool,
    pub f32: String,
    pub a32: String,
    pub c32: bool,
    pub f33: String,
    pub a33: String,
    pub c33: bool,
    pub f34: String,
    pub a34: String,
    pub c34: bool,
    pub f35: String,
    pub a35: String,
    pub c35: bool,
    pub f36: String,
    pub a36: String,
    pub c36: bool,
    pub f37: String,
    pub a37: String,
    pub c37: bool,
    pub f38: String,
    pub a38: String,
    pub c38: bool,
    pub f39: String,
    pub a39: String,
    pub c39: bool,
    pub f40: String,
    pub a40: String,
    pub c40: bool,
    pub f41: String,
    pub a41: String,
    pub c41: bool,
    pub f42: String,
    pub a42: String,
    pub c42: bool,
    pub f43: String,
    pub a43: String,
    pub c43: bool,
    pub f44: String,
    pub a44: String,
    pub c44: bool,
    pub f45: String,
    pub a45: String,
    pub c45: bool,
    pub f46: String,
    pub a46: String,
    pub c46: bool,
    pub f47: String,
    pub a47: String,
    pub c47: bool,
    pub f48: String,
    pub a48: String,
    pub c48: bool,
    pub f49: String,
    pub a49: String,
    pub c49: bool,
    pub f50: String,
    pub a50: String,
    pub c50: bool,
    pub f51: String,
    pub a51: String,
    pub c51: bool,
    pub f52: String,
    pub a52: String,
    pub c52: bool,
    pub f53: String,
    pub a53: String,
    pub c53: bool,
    pub f54: String,
    pub a54: String,
    pub c54: bool,
    pub f55: String,
    pub a55: String,
    pub c55: bool,
    pub f56: String,
    pub a56: String,
    pub c56: bool,
    pub f57: String,
    pub a57: String,
    pub c57: bool,
    pub f58: String,
    pub a58: String,
    pub c58: bool,
    pub f59: String,
    pub a59: String,
    pub c59: bool,
    pub f60: String,
    pub a60: String,
    pub c60: bool,
    pub f61: String,
    pub a61: String,
    pub c61: bool,
    pub f62: String,
    pub a62: String,
    pub c62: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaverResults {
    pub given: String,
    pub correct: String,
    pub is_correct: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SynopsisJsonResult {
    pub verb_id: i32,
    pub person: i32,
    pub number: i32,
    pub case: Option<i32>,
    pub gender: Option<i32>,
    pub unit: i32,
    pub pp: String,
    pub pp_correct: String,
    pub pp_is_correct: String,
    pub name: String,
    pub advisor: String,
    pub f: Vec<SaverResults>,
}

pub fn get_synopsis(
    payload: SynopsisSaverRequest,
    verbs: &[Arc<HcGreekVerb>],
) -> SynopsisJsonResult {
    let verb_id: usize = payload.verb.try_into().unwrap();

    let forms = get_forms(
        verbs,
        verb_id,
        payload.person,
        payload.number,
        payload.ptccase,
        payload.ptcgender,
    );

    let mut res = Vec::<SaverResults>::new();
    for f in forms {
        res.push(SaverResults {
            given: f.unwrap_or("".to_string()),
            correct: String::from(""),
            is_correct: true,
        });
    }

    SynopsisJsonResult {
        verb_id: payload.verb,
        person: payload.person,
        number: payload.number,
        case: payload.ptccase,
        gender: payload.ptcgender,
        unit: payload.unit,
        pp: verbs[verb_id]
            .pps
            .iter()
            .map(|x| {
                x /*.replace('/', " or ")*/
                    .replace("  ", " ")
            })
            .collect::<Vec<_>>()
            .join(", "),
        pp_correct: "".to_string(),
        pp_is_correct: "".to_string(),
        name: "".to_string(),
        advisor: "".to_string(),
        f: res,
    }
}

pub async fn get_synopsis_result(id: Uuid, hcdb: &HcDbPostgres) -> Option<SynopsisJsonResult> {
    let mut tx = hcdb.begin_tx().await.unwrap();
    if let Ok(result) = tx.greek_get_synopsis_result(id).await {
        //need to store is_correct and correct/incorrect answers
        tx.commit_tx().await.unwrap();

        let res_forms = vec![
            SaverResults {
                given: result.f0,
                correct: result.a0,
                is_correct: result.c0,
            },
            SaverResults {
                given: result.f1,
                correct: result.a1,
                is_correct: result.c1,
            },
            SaverResults {
                given: result.f2,
                correct: result.a2,
                is_correct: result.c2,
            },
            SaverResults {
                given: result.f3,
                correct: result.a3,
                is_correct: result.c3,
            },
            SaverResults {
                given: result.f4,
                correct: result.a4,
                is_correct: result.c4,
            },
            SaverResults {
                given: result.f5,
                correct: result.a5,
                is_correct: result.c5,
            },
            SaverResults {
                given: result.f6,
                correct: result.a6,
                is_correct: result.c6,
            },
            SaverResults {
                given: result.f7,
                correct: result.a7,
                is_correct: result.c7,
            },
            SaverResults {
                given: result.f8,
                correct: result.a8,
                is_correct: result.c8,
            },
            SaverResults {
                given: result.f9,
                correct: result.a9,
                is_correct: result.c9,
            },
            SaverResults {
                given: result.f10,
                correct: result.a10,
                is_correct: result.c10,
            },
            SaverResults {
                given: result.f11,
                correct: result.a11,
                is_correct: result.c11,
            },
            SaverResults {
                given: result.f12,
                correct: result.a12,
                is_correct: result.c12,
            },
            SaverResults {
                given: result.f13,
                correct: result.a13,
                is_correct: result.c13,
            },
            SaverResults {
                given: result.f14,
                correct: result.a14,
                is_correct: result.c14,
            },
            SaverResults {
                given: result.f15,
                correct: result.a15,
                is_correct: result.c15,
            },
            SaverResults {
                given: result.f16,
                correct: result.a16,
                is_correct: result.c16,
            },
            SaverResults {
                given: result.f17,
                correct: result.a17,
                is_correct: result.c17,
            },
            SaverResults {
                given: result.f18,
                correct: result.a18,
                is_correct: result.c18,
            },
            SaverResults {
                given: result.f19,
                correct: result.a19,
                is_correct: result.c19,
            },
            SaverResults {
                given: result.f20,
                correct: result.a20,
                is_correct: result.c20,
            },
            SaverResults {
                given: result.f21,
                correct: result.a21,
                is_correct: result.c21,
            },
            SaverResults {
                given: result.f22,
                correct: result.a22,
                is_correct: result.c22,
            },
            SaverResults {
                given: result.f23,
                correct: result.a23,
                is_correct: result.c23,
            },
            SaverResults {
                given: result.f24,
                correct: result.a24,
                is_correct: result.c24,
            },
            SaverResults {
                given: result.f25,
                correct: result.a25,
                is_correct: result.c25,
            },
            SaverResults {
                given: result.f26,
                correct: result.a26,
                is_correct: result.c26,
            },
            SaverResults {
                given: result.f27,
                correct: result.a27,
                is_correct: result.c27,
            },
            SaverResults {
                given: result.f28,
                correct: result.a28,
                is_correct: result.c28,
            },
            SaverResults {
                given: result.f29,
                correct: result.a29,
                is_correct: result.c29,
            },
            SaverResults {
                given: result.f30,
                correct: result.a30,
                is_correct: result.c30,
            },
            SaverResults {
                given: result.f31,
                correct: result.a31,
                is_correct: result.c31,
            },
            SaverResults {
                given: result.f32,
                correct: result.a32,
                is_correct: result.c32,
            },
            SaverResults {
                given: result.f33,
                correct: result.a33,
                is_correct: result.c33,
            },
            SaverResults {
                given: result.f34,
                correct: result.a34,
                is_correct: result.c34,
            },
            SaverResults {
                given: result.f35,
                correct: result.a35,
                is_correct: result.c35,
            },
            SaverResults {
                given: result.f36,
                correct: result.a36,
                is_correct: result.c36,
            },
            SaverResults {
                given: result.f37,
                correct: result.a37,
                is_correct: result.c37,
            },
            SaverResults {
                given: result.f38,
                correct: result.a38,
                is_correct: result.c38,
            },
            SaverResults {
                given: result.f39,
                correct: result.a39,
                is_correct: result.c39,
            },
            SaverResults {
                given: result.f40,
                correct: result.a40,
                is_correct: result.c40,
            },
            SaverResults {
                given: result.f41,
                correct: result.a41,
                is_correct: result.c41,
            },
            SaverResults {
                given: result.f42,
                correct: result.a42,
                is_correct: result.c42,
            },
            SaverResults {
                given: result.f43,
                correct: result.a43,
                is_correct: result.c43,
            },
            SaverResults {
                given: result.f44,
                correct: result.a44,
                is_correct: result.c44,
            },
            SaverResults {
                given: result.f45,
                correct: result.a45,
                is_correct: result.c45,
            },
            SaverResults {
                given: result.f46,
                correct: result.a46,
                is_correct: result.c46,
            },
            SaverResults {
                given: result.f47,
                correct: result.a47,
                is_correct: result.c47,
            },
            SaverResults {
                given: result.f48,
                correct: result.a48,
                is_correct: result.c48,
            },
            SaverResults {
                given: result.f49,
                correct: result.a49,
                is_correct: result.c49,
            },
            SaverResults {
                given: result.f50,
                correct: result.a50,
                is_correct: result.c50,
            },
            SaverResults {
                given: result.f51,
                correct: result.a51,
                is_correct: result.c51,
            },
            SaverResults {
                given: result.f52,
                correct: result.a52,
                is_correct: result.c52,
            },
            SaverResults {
                given: result.f53,
                correct: result.a53,
                is_correct: result.c53,
            },
            SaverResults {
                given: result.f54,
                correct: result.a54,
                is_correct: result.c54,
            },
            SaverResults {
                given: result.f55,
                correct: result.a55,
                is_correct: result.c55,
            },
            SaverResults {
                given: result.f56,
                correct: result.a56,
                is_correct: result.c56,
            },
            SaverResults {
                given: result.f57,
                correct: result.a57,
                is_correct: result.c57,
            },
            SaverResults {
                given: result.f58,
                correct: result.a58,
                is_correct: result.c58,
            },
            SaverResults {
                given: result.f59,
                correct: result.a59,
                is_correct: result.c59,
            },
            SaverResults {
                given: result.f60,
                correct: result.a60,
                is_correct: result.c60,
            },
            SaverResults {
                given: result.f61,
                correct: result.a61,
                is_correct: result.c61,
            },
            SaverResults {
                given: result.f62,
                correct: result.a62,
                is_correct: result.c62,
            },
        ];

        let res = SynopsisJsonResult {
            verb_id: result.selectedverb.parse::<i32>().unwrap(),
            person: result.verbperson.parse::<i32>().unwrap(),
            number: result.verbnumber.parse::<i32>().unwrap(),
            case: if result.verbptccase.is_some() {
                Some(result.verbptccase.unwrap().parse::<i32>().unwrap())
            } else {
                None
            },
            gender: if result.verbptcgender.is_some() {
                Some(result.verbptcgender.unwrap().parse::<i32>().unwrap())
            } else {
                None
            },
            unit: result.sgiday,
            pp: result.pp,
            pp_correct: result.pp_correct,
            pp_is_correct: result.pp_is_correct,
            // pp: verbs[verb_id]
            //     .pps
            //     .iter()
            //     .map(|x| x.replace('/', " or ").replace("  ", " "))
            //     .collect::<Vec<_>>()
            //     .join(", "),
            name: result.sname.clone(),
            advisor: result.advisor.clone(),
            f: res_forms,
        };
        return Some(res);
    }
    None
}

pub async fn save_synopsis(
    mut payload: SynopsisSaverRequest,
    user_id: Option<Uuid>,
    verbs: &[Arc<HcGreekVerb>],
    hcdb: &HcDbPostgres,
) -> Result<SynopsisJsonResult, Box<dyn std::error::Error>> {
    let verb_id = payload.verb.try_into().unwrap();
    let correct_answers = get_forms(
        verbs,
        verb_id,
        payload.person,
        payload.number,
        payload.ptccase,
        payload.ptcgender,
    );
    let mut is_correct = Vec::new();
    // let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));
    for (i, f) in payload.r.iter().enumerate() {
        if let Some(a) = &correct_answers[i] {
            is_correct.push(hgk_compare_multiple_forms(a, &f.replace("---", "—"), true));
        } else {
            is_correct.push(true);
        }
    }

    let is_correct_pps: Vec<bool> = check_pps(&payload.pp, &verbs[verb_id]);

    let mut db_insert = Vec::<String>::new();

    let mut res_forms = Vec::<SaverResults>::new();
    for (n, i) in correct_answers.into_iter().enumerate() {
        res_forms.push(SaverResults {
            given: payload.r[n].clone(),
            correct: i.clone().unwrap_or(String::from("")),
            is_correct: is_correct[n],
        });
        db_insert.push(payload.r[n].clone());
        db_insert.push(i.unwrap_or(String::from("")));
        db_insert.push(is_correct[n].to_string());
    }

    let res = SynopsisJsonResult {
        verb_id: payload.verb,
        person: payload.person,
        number: payload.number,
        case: payload.ptccase,
        gender: payload.ptcgender,
        unit: payload.unit,
        pp: payload.pp.clone(),
        pp_correct: format!(
            "{}, {}, {}, {}, {}, {}",
            &verbs[verb_id].pps[0],
            &verbs[verb_id].pps[1],
            &verbs[verb_id].pps[2],
            &verbs[verb_id].pps[3],
            &verbs[verb_id].pps[4],
            &verbs[verb_id].pps[5]
        ),
        pp_is_correct: is_correct_pps
            .into_iter()
            .map(|x| (x as i32).to_string())
            .join(","),
        // pp: verbs[verb_id]
        //     .pps
        //     .iter()
        //     .map(|x| x.replace('/', " or ").replace("  ", " "))
        //     .collect::<Vec<_>>()
        //     .join(", "),
        name: payload.sname.clone(),
        advisor: payload.advisor.clone(),
        f: res_forms,
    };

    let mut tx = hcdb.begin_tx().await?;

    payload.r = db_insert; //add correct boolean and correct answers here to save to db
    payload.pp_correct.clone_from(&res.pp_correct);
    payload.pp_is_correct.clone_from(&res.pp_is_correct);

    tx.greek_insert_synopsis(
        user_id, &payload,
        //ip.as_str(),
        //user_agent,
    )
    .await?;

    tx.commit_tx().await?;

    Ok(res)
}

pub fn get_forms(
    verbs: &[Arc<HcGreekVerb>],
    verb_id: usize,
    person: i32,
    number: i32,
    case: Option<i32>,
    gender: Option<i32>,
) -> Vec<Option<String>> {
    let mut forms = Vec::new();

    let tenses = [
        HcTense::Present,
        HcTense::Imperfect,
        HcTense::Future,
        HcTense::Aorist,
        HcTense::Perfect,
        HcTense::Pluperfect,
    ];

    let voices = [HcVoice::Active, HcVoice::Middle, HcVoice::Passive];
    let moods = [
        HcMood::Indicative,
        HcMood::Subjunctive,
        HcMood::Optative,
        HcMood::Imperative,
        HcMood::Infinitive,
        HcMood::Participle,
    ];

    let numbers = match number {
        1 => [HcNumber::Plural],
        _ => [HcNumber::Singular],
    };
    let persons = match person {
        0 => [HcPerson::First],
        1 => [HcPerson::Second],
        _ => [HcPerson::Third],
    };

    let case_value = match case {
        Some(0) => Some(HcCase::Nominative),
        Some(1) => Some(HcCase::Genitive),
        Some(2) => Some(HcCase::Dative),
        Some(3) => Some(HcCase::Accusative),
        Some(4) => Some(HcCase::Vocative),
        _ => None,
    };

    let gender_value = match gender {
        Some(0) => Some(HcGender::Masculine),
        Some(1) => Some(HcGender::Feminine),
        Some(2) => Some(HcGender::Neuter),
        _ => None,
    };

    for m in moods {
        for t in tenses {
            for v in voices {
                if ((m == HcMood::Subjunctive || m == HcMood::Optative || m == HcMood::Imperative)
                    && (t == HcTense::Imperfect
                        || t == HcTense::Perfect
                        || t == HcTense::Pluperfect))
                    || t == HcTense::Future && (m == HcMood::Subjunctive || m == HcMood::Imperative)
                {
                    // allow moods for oida, synoida
                    if !((m == HcMood::Subjunctive
                        || m == HcMood::Optative
                        || m == HcMood::Imperative)
                        && t == HcTense::Perfect
                        && v == HcVoice::Active
                        && (verbs[verb_id].pps[0] == "οἶδα" || verbs[verb_id].pps[0] == "σύνοιδα"))
                    {
                        continue;
                    }
                }

                if (m == HcMood::Infinitive || m == HcMood::Participle)
                    && (t == HcTense::Imperfect || t == HcTense::Pluperfect)
                {
                    continue;
                }

                for n in numbers {
                    for p in persons {
                        let vf = HcGreekVerbForm {
                            verb: verbs[verb_id].clone(),
                            person: if m == HcMood::Infinitive || m == HcMood::Participle {
                                None
                            } else {
                                Some(p)
                            },
                            number: if m == HcMood::Infinitive {
                                None
                            } else {
                                Some(n)
                            },
                            tense: t,
                            voice: v,
                            mood: m,
                            gender: if m == HcMood::Participle {
                                gender_value
                            } else {
                                None
                            },
                            case: if m == HcMood::Participle {
                                case_value
                            } else {
                                None
                            },
                        };

                        if let Ok(f) = vf.get_form(false) {
                            forms.push(Some(f.last().unwrap().form.replace(" /", ",")))
                        } else {
                            forms.push(None)
                        }
                    }
                }
            }
        }
    }
    forms
}

// pub async fn synopsis_json(
//     session: Session,
//     State(state): State<AxumAppState>,
//     extract::Form(payload): extract::Form<SynopsisSaverRequest>,
// ) -> Result<Json<>, StatusCode> {
//     // let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));
//     //let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();
//     // let pp = "λω, λσω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην";
//     // let verb = Arc::new(HcGreekVerb::from_string(1, pp, REGULAR, 0).unwrap());
//     let verb_id: usize = payload.verb;

//     let forms = get_forms(&state.verbs, verb_id, payload.person, payload.number, payload.ptccase, payload.ptcgender);

//     let mut res = Vec::<SaverResults>::new();
//     for f in forms {
//         res.push(SaverResults {
//             given: f.unwrap_or("".to_string()),
//             correct: None,
//             is_correct: true,
//         });
//     }

//     let res = SynopsisJsonResult {
//         verb_id: payload.verb,
//         person: payload.person,
//         number: payload.number,
//         case: payload.ptccase,
//         gender: payload.ptcgender,
//         unit: payload.unit,
//         pp: verbs[verb_id]
//             .pps
//             .iter()
//             .map(|x| x.replace('/', " or ").replace("  ", " "))
//             .collect::<Vec<_>>()
//             .join(", "),
//         name: "".to_string(),
//         advisor: "".to_string(),
//         f: res,
//     };

//     Ok(HttpResponse::Ok().json(res))
// }
/*

pub async fn greek_synopsis_result(
    (info, req): (web::Query<SynopsisResultRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db2 = req.app_data::<SqliteUpdatePool>().unwrap();

    let list = greek_get_synopsis_result(&db2.0, info.id)
        .await
        .map_err(map_sqlx_error)?;

    let mut res = String::from(
        r#"<!DOCTYPE html>
    <html lang='en'>
    <head>
    <meta charset="UTF-8">
    <style nonce="2726c7f26c">
    @font-face {
        font-family: 'WebNewAthenaUnicode';
        src: url('/newathu5_8.ttf') format('truetype');
      }
    BODY {font-family:helvetica,arial}
    .synTable { min-width:800px; font-size:16pt; border-spacing:0px;border-collapse: collapse;margin: 0px auto; }
    .synTable td {padding:4px 5px;}
    .labelcol { width:15%;}
    .label {font-weight:bold; }
    .spacer { width:50px;}
    .majorlabelrow { border-top:1px solid black;font-weight:bold;}
    .greek { font-family: WebNewAthenaUnicode;}
    </style>
    <script nonce="2726c7f26c">
    function fixdate() {
        const ds = document.getElementById('submittedDate');
        const date = new Date(parseInt(ds.innerHTML));
        ds.innerHTML = date.toLocaleString(navigator.language);
    }
    // window.addEventListener('load', fixdate, false);
    </script>
    </head>
    <body><table class='synTable'>"#,
    );
    for l in list {
        let eastern_daylight_tz = FixedOffset::west_opt(4 * 60 * 60).unwrap();
        let d = eastern_daylight_tz.timestamp_millis_opt(l.updated);
        let timestamp_str = match d {
            LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "".to_string(),
        };
        res.push_str(
            format!(
                "<tr><td colspan='2' class='label'>Name</td><td colspan='3'>{}</td></tr>",
                l.sname
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td colspan='2' class='label'>Advisor</td><td colspan='3'>{}</td></tr>",
                l.advisor
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td colspan='2' class='label'>Date</td><td id='submittedDate' colspan='3'>{}</td></tr>",
                timestamp_str
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td colspan='2' class='label'>Pers., Num., Gen., Case</td><td colspan='3'>{}, {}, {}, {}</td></tr>",
                l.verbperson, l.verbnumber, l.verbptcgender, l.verbptccase
            )
            .as_str(),
        );
        //res.push_str("<tr><td colspan='5'>&nbsp;</td></tr>");
        res.push_str(
            format!(
                "<tr><td colspan='2' class='label'>Principal Parts</td><td colspan='3' class='greek'>{}</td></tr>",
                l.pp
            )
            .as_str(),
        );
        //res.push_str("<tr><td colspan='5'>&nbsp;</td></tr>");
        res.push_str("<tr><td colspan='2'>&nbsp;</td><td class='label' align='left'>Active</td><td class='label' align='left'>Middle</td><td class='label' align='left'>Passive</td></tr>");

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Indicative</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f0, l.f1, l.f2).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Imperfect",  l.f3, l.f4, l.f5).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Future",  l.f6, l.f7, l.f8).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f9, l.f10, l.f11).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Perfect",  l.f12, l.f13, l.f14).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Pluperfect",  l.f15, l.f16, l.f17).as_str());

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Subjunctive</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f18, l.f19, l.f20).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f21, l.f22, l.f23).as_str());

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Optative</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f24, l.f25, l.f26).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Future",  l.f27, l.f28, l.f29).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f30, l.f31, l.f32).as_str());

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Imperative</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f33, l.f34, l.f35).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f36, l.f37, l.f38).as_str());

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Infinitive</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f39, l.f40, l.f41).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Future",  l.f42, l.f43, l.f44).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f45, l.f46, l.f47).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Perfect",  l.f48, l.f49, l.f50).as_str());

        res.push_str("<tr><td colspan='5' class='majorlabelrow'>Participle</td></tr>");
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Present",  l.f51, l.f52, l.f53).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Future",  l.f54, l.f55, l.f56).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Aorist",  l.f57, l.f58, l.f59).as_str());
        res.push_str(format!("<tr><td class='spacer'>&nbsp;</td><td class='labelcol label'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td><td class='greek'>{}</td></tr>", "Perfect",  l.f60, l.f61, l.f62).as_str());
    }
    res.push_str("</table></body></html>");

    Ok(HttpResponse::Ok().content_type("text/html").body(res))
}

pub async fn greek_synopsis_saver(
    (info, req): (web::Json<SynopsisSaverRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db2 = req.app_data::<SqliteUpdatePool>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let time_stamp = SystemTime::now().duration_since(UNIX_EPOCH);
    let time_stamp_ms = if let Ok(time_stamp) = time_stamp {
        time_stamp.as_millis()
    } else {
        0
    };
    let user_agent = get_user_agent(&req).unwrap_or("");
    //https://stackoverflow.com/questions/66989780/how-to-retrieve-the-ip-address-of-the-client-from-httprequest-in-actix-web
    let ip = if req.peer_addr().is_some() {
        req.peer_addr().unwrap().ip().to_string()
    } else {
        "".to_string()
    };

    let verb_id = info.verb;
    let correct_answers = get_forms(verbs, verb_id, info.person, info.number, info.ptccase, info.ptcgender);
    let mut is_correct = Vec::new();
    // let is_correct = hgk_compare_multiple_forms(&correct_answer, &info.answer.replace("---", "—"));
    for (i, f) in info.r.iter().enumerate() {
        if let Some(a) = &correct_answers[i] {
            is_correct.push(hgk_compare_multiple_forms(a, &f.replace("---", "—"), true));
        } else {
            is_correct.push(true);
        }
    }

    let mut res_forms = Vec::<SaverResults>::new();
    for (n, i) in correct_answers.into_iter().enumerate() {
        res_forms.push(SaverResults {
            given: info.r[n].clone(),
            correct: i,
            is_correct: is_correct[n],
        });
    }

    let res = SynopsisJsonResult {
        verb_id,
        person: info.person,
        number: info.number,
        case: info.ptccase,
        gender: info.ptcgender,
        unit: info.unit,
        pp: info.pp.clone(),
        // pp: verbs[verb_id]
        //     .pps
        //     .iter()
        //     .map(|x| x.replace('/', " or ").replace("  ", " "))
        //     .collect::<Vec<_>>()
        //     .join(", "),
        name: info.sname.clone(),
        advisor: info.advisor.clone(),
        f: res_forms,
    };

    let _ = greek_insert_synopsis(
        &db2.0,
        &info.into_inner(),
        time_stamp_ms,
        ip.as_str(),
        user_agent,
    )
    .await
    .map_err(map_sqlx_error)?;

    //Ok(HttpResponse::Ok().finish())
    //let res = 1;
    Ok(HttpResponse::Ok().json(res))
}

pub async fn cetest(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    let template = include_str!("cetest.html").to_string();
    Ok(HttpResponse::Ok().content_type("text/html").body(template))
}

pub async fn latin_synopsis_list(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db2 = req.app_data::<SqliteUpdatePool>().unwrap();

    let list = latin_get_synopsis_list(&db2.0)
        .await
        .map_err(map_sqlx_error)?;

    let mut res = String::from(
        r#"<!DOCTYPE html>
    <html>
    <head>
    <meta charset="UTF-8">
    <style nonce="2726c7f26c">
        .synlist { width: 600px;
            margin: 0px auto;
            border-collapse: collapse;
            font-size: 16pt;
            font-family:helvetica,arial;
        }
        .synlist td { padding: 3px; }
        .headerrow {border-bottom:1px solid black;font-weight:bold;}

    </style>
    </head>
    <body><table class='synlist'>
    <tr><td class='headerrow'>Date</td><td class='headerrow'>Name</td><td class='headerrow'>Advisor</td><td class='headerrow'>Verb</td></tr>"#,
    );
    for l in list {
        let eastern_daylight_tz = FixedOffset::west_opt(4 * 60 * 60).unwrap();
        let d = eastern_daylight_tz.timestamp_millis_opt(l.1);
        let timestamp_str = match d {
            LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "".to_string(),
        };

        res.push_str(format!("<tr><td><a href='latin-synopsis-result?id={}'>{}</a></td><td>{}</td><td>{}</td><td>{}</td></tr>", l.0, timestamp_str, l.2, l.3,l.4).as_str());
    }
    res.push_str("</table></body></html>");

    Ok(HttpResponse::Ok().content_type("text/html").body(res))
}

pub async fn latin_synopsis_result(
    (info, req): (web::Query<SynopsisResultRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db2 = req.app_data::<SqliteUpdatePool>().unwrap();

    let list = latin_get_synopsis_result(&db2.0, info.id)
        .await
        .map_err(map_sqlx_error)?;

    let mut res = String::from(
        r#"<!DOCTYPE html>
    <html lang='en'>
    <head>
    <meta charset="UTF-8">
    <style nonce="2726c7f26c">
    BODY {font-family:helvetica,arial}
    .synTable { min-width:800px; font-size:16pt; border-spacing:0px;border-collapse: collapse;margin: 0px auto; }
    .synTable td {padding:4px 5px;}
    .labelcol { width:25%;}
    .label {font-weight:bold; }
    .spacer { width:25%;}
    .majorlabelrow { border-top:1px solid black;}
    </style>
    <script nonce="2726c7f26c">
    function fixdate() {
        const ds = document.getElementById('submittedDate');
        const date = new Date(parseInt(ds.innerHTML));
        ds.innerHTML = date.toLocaleString(navigator.language);
    }
    // window.addEventListener('load', fixdate, false);
    </script>
    </head>
    <body><table class='synTable'>"#,
    );

    for l in list {
        let eastern_daylight_tz = FixedOffset::west_opt(4 * 60 * 60).unwrap();
        let d = eastern_daylight_tz.timestamp_millis_opt(l.updated);
        let timestamp_str = match d {
            LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "".to_string(),
        };

        res.push_str(
            format!(
                "<tr><td class='label'>Name</td><td colspan='3'>{}</td></tr>",
                l.sname
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td class='label'>Advisor</td><td colspan='3'>{}</td></tr>",
                l.advisor
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td class='label'>Date</td><td id='submittedDate' colspan='3'>{}</td></tr>",
                timestamp_str
            )
            .as_str(),
        );
        res.push_str(
            format!(
                "<tr><td class='label'>Pers. Num. Gen.</td><td colspan='3'>{}, {}, {}</td></tr>",
                l.verbperson, l.verbnumber, l.verbptcgender
            )
            .as_str(),
        );

        res.push_str("<tr><td colspan='4'>&nbsp;</td></tr>");
        res.push_str(
            format!(
                "<tr><td class='label'>Principal Parts</td><td colspan='3'>{}</td></tr>",
                l.pp
            )
            .as_str(),
        );
        res.push_str("<tr><td colspan='4'>&nbsp;</td></tr>");
        res.push_str("<tr><td class='spacer'><td></td></td><td class='label' align='left'>Active</td><td class='label' align='left'>Passive</td></tr>");

        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Indicative</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Present",  l.f0, l.f1).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Imperfect",  l.f2, l.f3).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Future",  l.f4, l.f5).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Subjunctive</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Present",  l.f6, l.f7).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Imperfect",  l.f8, l.f9).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Indicative</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Perfect",  l.f10, l.f11).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Pluperfect",  l.f12, l.f13).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Future Perfect",  l.f14, l.f15).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Subjunctive</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Perfect",  l.f16, l.f17).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Pluperfect",  l.f18, l.f19).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Participles</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Present",  l.f20, l.f21).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Perfect",  l.f22, l.f23).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Future",  l.f24, l.f25).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Infintives</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Present",  l.f26, l.f27).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Perfect",  l.f28, l.f29).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Future",  l.f30, l.f31).as_str());
        res.push_str("<tr><td class='label majorlabelrow' colspan='4'>Imperatives</td></tr>");
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Singular",  l.f32, l.f33).as_str());
        res.push_str(format!("<tr><td class='spacer'></td><td class='labelcol label'>{}</td><td>{}</td><td>{}</td></tr>", "Plural",  l.f34, l.f35).as_str());
    }
    res.push_str("</table></body></html>");

    Ok(HttpResponse::Ok().content_type("text/html").body(res))
}

pub async fn latin_synopsis_saver(
    (info, req): (web::Json<SynopsisSaverRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db2 = req.app_data::<SqliteUpdatePool>().unwrap();

    let time_stamp = SystemTime::now().duration_since(UNIX_EPOCH);
    let time_stamp_ms = if let Ok(time_stamp) = time_stamp {
        time_stamp.as_millis()
    } else {
        0
    };
    let user_agent = get_user_agent(&req).unwrap_or("");
    //https://stackoverflow.com/questions/66989780/how-to-retrieve-the-ip-address-of-the-client-from-httprequest-in-actix-web
    let ip = if req.peer_addr().is_some() {
        req.peer_addr().unwrap().ip().to_string()
    } else {
        "".to_string()
    };

    let _ = latin_insert_synopsis(
        &db2.0,
        &info.into_inner(),
        time_stamp_ms,
        ip.as_str(),
        user_agent,
    )
    .await
    .map_err(map_sqlx_error)?;

    //Ok(HttpResponse::Ok().finish())
    //let res = 1;
    Ok(HttpResponse::Ok().json(1))
}

pub async fn latin_synopsis(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    let mut template = include_str!("latin-synopsis.html").to_string();

    let mut rows = String::from("");
    let mut count = 0;
    let rowlabels = vec![
        "Present",
        "Imperfect",
        "Future",
        "Present",
        "Imperfect",
        "Perfect",
        "Pluperfect",
        "Future Perfect ",
        "Perfect",
        "Pluperfect",
        "Present",
        "Perfect",
        "Future",
        "Present",
        "Perfect",
        "Future",
        "Singular",
        "Plural",
    ];
    let voices = vec!["Active", "Passive"];
    for l in rowlabels {
        if count == 0 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Indicative</td></tr>");
        } else if count == 6 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Subjunctive</td></tr>");
        } else if count == 10 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Indicative</td></tr>");
        } else if count == 16 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Subjunctive</td></tr>");
        } else if count == 20 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Participles</td></tr>");
        } else if count == 26 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Infinitives</td></tr>");
        } else if count == 32 {
            rows.push_str("<tr><td colspan='3' class='majorrowheader'>Imperatives</td></tr>");
        }
        rows.push_str(
            format!(
                r#"<tr class="{}"><td class='formcelllabel'>{}</td>"#,
                l.to_lowercase(),
                l
            )
            .as_str(),
        );
        for v in &voices {
            rows.push_str(format!(
            r#"<td class="formcell {}">
                <div class="formcellInner">
                <input type="text" id="gkform{}" class="gkinput formcellinput" spellcheck="false" autocapitalize="off" autocomplete="off" {}/>
                </div>
            </td>"#,
            v.to_lowercase(), count, if count == 21 || count == 22 {"disabled"} else {""}).as_str());
            count += 1;
        }
        rows.push_str("</tr>");
    }

    template = template.replacen("%rows%", &rows, 1);

    Ok(HttpResponse::Ok().content_type("text/html").body(template))
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
*/

/*
CREATE TABLE latinsynopsisresults ( id INTEGER PRIMARY KEY AUTOINCREMENT, updated INTEGER NOT NULL, sname TEXT NOT NULL,
    advisor TEXT NOT NULL, sgiday INTEGER NOT NULL, selectedverb TEXT NOT NULL, pp TEXT NOT NULL, verbnumber TEXT NOT NULL,
    verbperson TEXT NOT NULL, verbptcgender TEXT NOT NULL, verbptcnumber TEXT NOT NULL, verbptccase TEXT NOT NULL, ip TEXT NOT NULL,
    ua TEXT NOT NULL, status INTEGER NOT NULL,
    f0 TEXT NOT NULL, f1 TEXT NOT NULL, f2 TEXT NOT NULL, f3 TEXT NOT NULL, f4 TEXT NOT NULL, f5 TEXT NOT NULL, f6 TEXT NOT NULL, f7 TEXT NOT NULL,
    f8 TEXT NOT NULL, f9 TEXT NOT NULL, f10 TEXT NOT NULL, f11 TEXT NOT NULL, f12 TEXT NOT NULL, f13 TEXT NOT NULL, f14 TEXT NOT NULL,
    f15 TEXT NOT NULL, f16 TEXT NOT NULL, f17 TEXT NOT NULL, f18 TEXT NOT NULL, f19 TEXT NOT NULL, f20 TEXT NOT NULL, f21 TEXT NOT NULL,
    f22 TEXT NOT NULL, f23 TEXT NOT NULL, f24 TEXT NOT NULL, f25 TEXT NOT NULL, f26 TEXT NOT NULL, f27 TEXT NOT NULL, f28 TEXT NOT NULL,
    f29 TEXT NOT NULL, f30 TEXT NOT NULL, f31 TEXT NOT NULL, f32 TEXT NOT NULL, f33 TEXT NOT NULL, f34 TEXT NOT NULL, f35 TEXT NOT NULL);


CREATE TABLE greeksynopsisresults ( id INTEGER PRIMARY KEY AUTOINCREMENT, updated INTEGER NOT NULL, sname TEXT NOT NULL, advisor TEXT NOT NULL, sgiday INTEGER NOT NULL, selectedverb TEXT NOT NULL, pp TEXT NOT NULL, verbnumber TEXT NOT NULL, verbperson TEXT NOT NULL, verbptcgender TEXT NOT NULL, verbptcnumber TEXT NOT NULL, verbptccase TEXT NOT NULL, ip TEXT NOT NULL, ua TEXT NOT NULL, status INTEGER NOT NULL, f0 TEXT NOT NULL, f1 TEXT NOT NULL, f2 TEXT NOT NULL, f3 TEXT NOT NULL, f4 TEXT NOT NULL, f5 TEXT NOT NULL, f6 TEXT NOT NULL, f7 TEXT NOT NULL, f8 TEXT NOT NULL, f9 TEXT NOT NULL, f10 TEXT NOT NULL, f11 TEXT NOT NULL, f12 TEXT NOT NULL, f13 TEXT NOT NULL, f14 TEXT NOT NULL, f15 TEXT NOT NULL, f16 TEXT NOT NULL, f17 TEXT NOT NULL, f18 TEXT NOT NULL, f19 TEXT NOT NULL, f20 TEXT NOT NULL, f21 TEXT NOT NULL, f22 TEXT NOT NULL, f23 TEXT NOT NULL, f24 TEXT NOT NULL, f25 TEXT NOT NULL, f26 TEXT NOT NULL, f27 TEXT NOT NULL, f28 TEXT NOT NULL, f29 TEXT NOT NULL, f30 TEXT NOT NULL, f31 TEXT NOT NULL, f32 TEXT NOT NULL, f33 TEXT NOT NULL, f34 TEXT NOT NULL, f35 TEXT NOT NULL, f36 TEXT NOT NULL, f37 TEXT NOT NULL, f38 TEXT NOT NULL, f39 TEXT NOT NULL, f40 TEXT NOT NULL, f41 TEXT NOT NULL, f42 TEXT NOT NULL, f43 TEXT NOT NULL, f44 TEXT NOT NULL, f45 TEXT NOT NULL, f46 TEXT NOT NULL, f47 TEXT NOT NULL, f48 TEXT NOT NULL, f49 TEXT NOT NULL, f50 TEXT NOT NULL, f51 TEXT NOT NULL, f52 TEXT NOT NULL, f53 TEXT NOT NULL, f54 TEXT NOT NULL, f55 TEXT NOT NULL, f56 TEXT NOT NULL, f57 TEXT NOT NULL, f58 TEXT NOT NULL, f59 TEXT NOT NULL, f60 TEXT NOT NULL, f61 TEXT NOT NULL, f62 TEXT NOT NULL);
*/
