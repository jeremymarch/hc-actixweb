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

mod login;
mod server;
mod session;

use actix_files as fs;
use actix_session::Session;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::Key;
use actix_web::cookie::SameSite;
use actix_web::http::header::ContentType;
use actix_web::http::header::HeaderValue;
use actix_web::http::header::LOCATION;
use actix_web::http::header::{CONTENT_SECURITY_POLICY, STRICT_TRANSPORT_SECURITY};
use actix_web::{http::StatusCode, ResponseError};
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use libhc;
use libhc::db::HcDb;
use libhc::AnswerQuery;
use libhc::AskQuery;
use libhc::CreateSessionQuery;
use libhc::GetMovesQuery;
use libhc::MoveResult;
use libhc::MoveType;
use libhc::SessionState;
use libhc::SessionsListQuery;
use thiserror::Error;

use actix::Actor;
use actix::Addr;
use actix_web::Error;
use actix_web_actors::ws;
use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};

use actix_session::config::PersistentSession;
use actix_web::cookie::time::Duration;
const SECS_IN_10_YEARS: i64 = 60 * 60 * 24 * 7 * 4 * 12 * 10;

//use std::fs::File;
//use std::io::BufReader;
//use std::io::BufRead;
use rand::Rng;
use std::io;

//use uuid::Uuid;

use chrono::prelude::*;

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Uuid;

use hoplite_verbs_rs::*;

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

static PPS: &str = r##"παιδεύω, παιδεύσω, ἐπαίδευσα, πεπαίδευκα, πεπαίδευμαι, ἐπαιδεύθην % 2
πέμπω, πέμψω, ἔπεμψα, πέπομφα, πέπεμμαι, ἐπέμφθην % 2
κελεύω, κελεύσω, ἐκέλευσα, κεκέλευκα, κεκέλευσμαι, ἐκελεύσθην % 2
λῡ́ω, λῡ́σω, ἔλῡσα, λέλυκα, λέλυμαι, ἐλύθην % 2
γράφω, γράψω, ἔγραψα, γέγραφα, γέγραμμαι, ἐγράφην % 3
θῡ́ω, θῡ́σω, ἔθῡσα, τέθυκα, τέθυμαι, ἐτύθην % 3
παύω, παύσω, ἔπαυσα, πέπαυκα, πέπαυμαι, ἐπαύθην % 3
φυλάττω, φυλάξω, ἐφύλαξα, πεφύλαχα, πεφύλαγμαι, ἐφυλάχθην % 3
διδάσκω, διδάξω, ἐδίδαξα, δεδίδαχα, δεδίδαγμαι, ἐδιδάχθην % 4
ἐθέλω, ἐθελήσω, ἠθέλησα, ἠθέληκα, —, — % 4
θάπτω, θάψω, ἔθαψα, —, τέθαμμαι, ἐτάφην % 4 % CONSONANT_STEM_PERFECT_PI
τάττω, τάξω, ἔταξα, τέταχα, τέταγμαι, ἐτάχθην % 4 % CONSONANT_STEM_PERFECT_GAMMA
ἄρχω, ἄρξω, ἦρξα, ἦρχα, ἦργμαι, ἤρχθην % 5 % CONSONANT_STEM_PERFECT_CHI
βλάπτω, βλάψω, ἔβλαψα, βέβλαφα, βέβλαμμαι, ἐβλάβην / ἐβλάφθην % 5 % CONSONANT_STEM_PERFECT_BETA
πείθω, πείσω, ἔπεισα, πέπεικα, πέπεισμαι, ἐπείσθην % 5
πρᾱ́ττω, πρᾱ́ξω, ἔπρᾱξα, πέπρᾱχα / πέπρᾱγα, πέπρᾱγμαι, ἐπρᾱ́χθην % 5 % CONSONANT_STEM_PERFECT_GAMMA
δουλεύω, δουλεύσω, ἐδούλευσα, δεδούλευκα, —, — % 6
κωλῡ́ω, κωλῡ́σω, ἐκώλῡσα, κεκώλῡκα, κεκώλῡμαι, ἐκωλῡ́θην % 6
πολῑτεύω, πολῑτεύσω, ἐπολῑ́τευσα, πεπολῑ́τευκα, πεπολῑ́τευμαι, ἐπολῑτεύθην % 6
χορεύω, χορεύσω, ἐχόρευσα, κεχόρευκα, κεχόρευμαι, ἐχορεύθην % 6
κλέπτω, κλέψω, ἔκλεψα, κέκλοφα, κέκλεμμαι, ἐκλάπην % 7 % CONSONANT_STEM_PERFECT_PI
λείπω, λείψω, ἔλιπον, λέλοιπα, λέλειμμαι, ἐλείφθην % 7 % CONSONANT_STEM_PERFECT_PI
σῴζω, σώσω, ἔσωσα, σέσωκα, σέσωσμαι / σέσωμαι, ἐσώθην % 7
ἄγω, ἄξω, ἤγαγον, ἦχα, ἦγμαι, ἤχθην % 8 % CONSONANT_STEM_PERFECT_GAMMA
ἥκω, ἥξω, —, —, —, — % 8
ἀδικέω, ἀδικήσω, ἠδίκησα, ἠδίκηκα, ἠδίκημαι, ἠδικήθην % 9
νῑκάω, νῑκήσω, ἐνῑ́κησα, νενῑ́κηκα, νενῑ́κημαι, ἐνῑκήθην % 9
ποιέω, ποιήσω, ἐποίησα, πεποίηκα, πεποίημαι, ἐποιήθην % 9
τῑμάω, τῑμήσω, ἐτῑ́μησα, τετῑ́μηκα, τετῑ́μημαι, ἐτῑμήθην % 9
ἀγγέλλω, ἀγγελῶ, ἤγγειλα, ἤγγελκα, ἤγγελμαι, ἠγγέλθην % 10 % CONSONANT_STEM_PERFECT_LAMBDA
ἀξιόω, ἀξιώσω, ἠξίωσα, ἠξίωκα, ἠξίωμαι, ἠξιώθην % 10
δηλόω, δηλώσω, ἐδήλωσα, δεδήλωκα, δεδήλωμαι, ἐδηλώθην % 10
καλέω, καλῶ, ἐκάλεσα, κέκληκα, κέκλημαι, ἐκλήθην % 10
μένω, μενῶ, ἔμεινα, μεμένηκα, —, — % 10
τελευτάω, τελευτήσω, ἐτελεύτησα, τετελεύτηκα, τετελεύτημαι, ἐτελευτήθην % 10
ἀκούω, ἀκούσομαι, ἤκουσα, ἀκήκοα, —, ἠκούσθην % 11
ἀποδέχομαι, ἀποδέξομαι, ἀπεδεξάμην, —, ἀποδέδεγμαι, — % 11 % CONSONANT_STEM_PERFECT_CHI PREFIXED
βάλλω, βαλῶ, ἔβαλον, βέβληκα, βέβλημαι, ἐβλήθην % 11
βούλομαι, βουλήσομαι, —, —, βεβούλημαι, ἐβουλήθην % 11
δέχομαι, δέξομαι, ἐδεξάμην, —, δέδεγμαι, — % 11 % CONSONANT_STEM_PERFECT_CHI
λαμβάνω, λήψομαι, ἔλαβον, εἴληφα, εἴλημμαι, ἐλήφθην % 11 % CONSONANT_STEM_PERFECT_BETA
πάσχω, πείσομαι, ἔπαθον, πέπονθα, —, — % 11
ἀνατίθημι, ἀναθήσω, ἀνέθηκα, ἀνατέθηκα, ἀνατέθειμαι, ἀνετέθην % 12 % PREFIXED
ἀποδίδωμι, ἀποδώσω, ἀπέδωκα, ἀποδέδωκα, ἀποδέδομαι, ἀπεδόθην % 12 % PREFIXED
ἀφίστημι, ἀποστήσω, ἀπέστησα / ἀπέστην, ἀφέστηκα, ἀφέσταμαι, ἀπεστάθην % 12 % PREFIXED
δίδωμι, δώσω, ἔδωκα, δέδωκα, δέδομαι, ἐδόθην % 12
ἵστημι, στήσω, ἔστησα / ἔστην, ἕστηκα, ἕσταμαι, ἐστάθην % 12
καθίστημι, καταστήσω, κατέστησα / κατέστην, καθέστηκα, καθέσταμαι, κατεστάθην % 12 % PREFIXED
καταλῡ́ω, καταλῡ́σω, κατέλῡσα, καταλέλυκα, καταλέλυμαι, κατελύθην % 12 % PREFIXED
τίθημι, θήσω, ἔθηκα, τέθηκα, τέθειμαι, ἐτέθην % 12
φιλέω, φιλήσω, ἐφίλησα, πεφίληκα, πεφίλημαι, ἐφιλήθην % 12
φοβέομαι, φοβήσομαι, —, —, πεφόβημαι, ἐφοβήθην % 12
γίγνομαι, γενήσομαι, ἐγενόμην, γέγονα, γεγένημαι, — % 13
ἔρχομαι, ἐλεύσομαι, ἦλθον, ἐλήλυθα, —, — % 13
μανθάνω, μαθήσομαι, ἔμαθον, μεμάθηκα, —, — % 13
μάχομαι, μαχοῦμαι, ἐμαχεσάμην, —, μεμάχημαι, — % 13
μεταδίδωμι, μεταδώσω, μετέδωκα, μεταδέδωκα, μεταδέδομαι, μετεδόθην % 13 % PREFIXED
μετανίσταμαι, μεταναστήσομαι, μετανέστην, μετανέστηκα, —, — % 13 % PREFIXED
μηχανάομαι, μηχανήσομαι, ἐμηχανησάμην, —, μεμηχάνημαι, — % 13
φεύγω, φεύξομαι, ἔφυγον, πέφευγα, —, — % 13
δείκνῡμι, δείξω, ἔδειξα, δέδειχα, δέδειγμαι, ἐδείχθην % 14
ἐπανίσταμαι, ἐπαναστήσομαι, ἐπανέστην, ἐπανέστηκα, —, —  % 14 % PREFIXED
ἐπιδείκνυμαι, ἐπιδείξομαι, ἐπεδειξάμην, —, ἐπιδέδειγμαι, — % 14 % PREFIXED
ἐρωτάω, ἐρωτήσω, ἠρώτησα, ἠρώτηκα, ἠρώτημαι, ἠρωτήθην % 14
λανθάνω, λήσω, ἔλαθον, λέληθα, —, — % 14
παραγίγνομαι, παραγενήσομαι, παρεγενόμην, παραγέγονα, παραγεγένημαι, — % 14 % PREFIXED
παραδίδωμι, παραδώσω, παρέδωκα, παραδέδωκα, παραδέδομαι, παρεδόθην % 14 % PREFIXED
παραμένω, παραμενῶ, παρέμεινα, παραμεμένηκα, —, — % 14 % PREFIXED
τυγχάνω, τεύξομαι, ἔτυχον, τετύχηκα, —, — % 14
ὑπακούω, ὑπακούσομαι, ὑπηκουσα, ὑπακήκοα, —, ὑπηκούσθην % 14 % PREFIXED
ὑπομένω, ὑπομενῶ, ὑπέμεινα, ὑπομεμένηκα, —, — % 14 % PREFIXED
φθάνω, φθήσομαι, ἔφθασα / ἔφθην, —, —, — % 14
χαίρω, χαιρήσω, —, κεχάρηκα, —, ἐχάρην % 14
αἱρέω, αἱρήσω, εἷλον, ᾕρηκα, ᾕρημαι, ᾑρέθην % 15
αἰσθάνομαι, αἰσθήσομαι, ᾐσθόμην, —, ᾔσθημαι, — % 15
διαφέρω, διοίσω, διήνεγκα / διήνεγκον, διενήνοχα, διενήνεγμαι, διηνέχθην % 15 % PREFIXED
εἰμί, ἔσομαι, —, —, —, — % 15
ἔστι(ν), ἔσται, —, —, —, — % 15
ἔξεστι(ν), ἐξέσται, —, —, —, — % 15
ἕπομαι, ἕψομαι, ἑσπόμην, —, —, — % 15
ὁράω, ὄψομαι, εἶδον, ἑόρᾱκα / ἑώρᾱκα, ἑώρᾱμαι / ὦμμαι, ὤφθην % 15 % CONSONANT_STEM_PERFECT_PI
συμφέρω, συνοίσω, συνήνεγκα / συνήνεγκον, συνενήνοχα, συνενήνεγμαι, συνηνέχθην % 15 % PREFIXED
φέρω, οἴσω, ἤνεγκα / ἤνεγκον, ἐνήνοχα, ἐνήνεγμαι, ἠνέχθην % 15
ἀναβαίνω, ἀναβήσομαι, ἀνέβην, ἀναβέβηκα, —, — % 16 % PREFIXED
βαίνω, -βήσομαι, -ἔβην, βέβηκα, —, — % 16
γιγνώσκω, γνώσομαι, ἔγνων, ἔγνωκα, ἔγνωσμαι, ἐγνώσθην % 16
ἐκπῑ́πτω, ἐκπεσοῦμαι, ἐξέπεσον, ἐκπέπτωκα, —, — % 16 % PREFIXED
λέγω, ἐρῶ / λέξω, εἶπον / ἔλεξα, εἴρηκα, εἴρημαι / λέλεγμαι, ἐλέχθην / ἐρρήθην % 16 % CONSONANT_STEM_PERFECT_GAMMA
νομίζω, νομιῶ, ἐνόμισα, νενόμικα, νενόμισμαι, ἐνομίσθην % 16
πῑ́πτω, πεσοῦμαι, ἔπεσον, πέπτωκα, —, — % 16
προδίδωμι, προδώσω, προέδωκα / προύδωκα, προδέδωκα, προδέδομαι, προεδόθην / προυδόθην % 16 % PREFIXED
φημί, φήσω, ἔφησα, —, —, — % 16
ἁμαρτάνω, ἁμαρτήσομαι, ἥμαρτον, ἡμάρτηκα, ἡμάρτημαι, ἡμαρτήθην % 17
δοκέω, δόξω, ἔδοξα, —, δέδογμαι, -ἐδόχθην % 17
δύναμαι, δυνήσομαι, —, —, δεδύνημαι, ἐδυνήθην % 17
εἶμι, —, —, —, —, — % 17
ἐλαύνω, ἐλῶ, ἤλασα, -ἐλήλακα, ἐλήλαμαι, ἠλάθην % 17
ἐπίσταμαι, ἐπιστήσομαι, —, —, —, ἠπιστήθην % 17
ἔχω, ἕξω / σχήσω, ἔσχον, ἔσχηκα, -ἔσχημαι, — % 17
ἀποθνῄσκω, ἀποθανοῦμαι, ἀπέθανον, τέθνηκα, —, — % 18 % PREFIXED
ἀποκτείνω, ἀποκτενῶ, ἀπέκτεινα, ἀπέκτονα, —, — % 18 % PREFIXED
ἀφῑ́ημι, ἀφήσω, ἀφῆκα, ἀφεῖκα, ἀφεῖμαι, ἀφείθην % 18 % PREFIXED
βουλεύω, βουλεύσω, ἐβούλευσα, βεβούλευκα, βεβούλευμαι, ἐβουλεύθην % 18
ἐπιβουλεύω, ἐπιβουλεύσω, ἐπεβούλευσα, ἐπιβεβούλευκα, ἐπιβεβούλευμαι, ἐπεβουλεύθην % 18 % PREFIXED
ζητέω, ζητήσω, ἐζήτησα, ἐζήτηκα, —, ἐζητήθην % 18
ῑ̔́ημι, -ἥσω, -ἧκα, -εἷκα, -εἷμαι, -εἵθην % 18
μέλλω, μελλήσω, ἐμέλλησα, —, —, — % 18
πιστεύω, πιστεύσω, ἐπίστευσα, πεπίστευκα, πεπίστευμαι, ἐπιστεύθην % 18
συμβουλεύω, συμβουλεύσω, συνεβούλευσα, συμβεβούλευκα, συμβεβούλευμαι, συνεβουλεύθην % 18 % PREFIXED
συνῑ́ημι, συνήσω, συνῆκα, συνεῖκα, συνεῖμαι, συνείθην % 18 % PREFIXED
αἰσχῡ́νομαι, αἰσχυνοῦμαι, —, —, ᾔσχυμμαι, ᾐσχύνθην % 19 % CONSONANT_STEM_PERFECT_NU
ἀποκρῑ́νομαι, ἀποκρινοῦμαι, ἀπεκρῑνάμην, —, ἀποκέκριμαι, — % 19
ἀπόλλῡμι, ἀπολῶ, ἀπώλεσα / ἀπωλόμην, ἀπολώλεκα / ἀπόλωλα, —, — % 19
—, ἀνερήσομαι, ἀνηρόμην, —, —, — % 19
—, ἐρήσομαι, ἠρόμην, —, —, — % 19
εὑρίσκω, εὑρήσω, ηὗρον, ηὕρηκα, ηὕρημαι, ηὑρέθην % 19
ἡγέομαι, ἡγήσομαι, ἡγησάμην, —, ἥγημαι, ἡγήθην % 19
κρῑ́νω, κρινῶ, ἔκρῑνα, κέκρικα, κέκριμαι, ἐκρίθην % 19
οἶδα, εἴσομαι, —, —, —, — % 19
σύνοιδα, συνείσομαι, —, —, —, — % 19
ἀφικνέομαι, ἀφίξομαι, ἀφῑκόμην, —, ἀφῖγμαι, — % 20 % PREFIXED
δεῖ, δεήσει, ἐδέησε(ν), —, —, — % 20
κεῖμαι, κείσομαι, —, —, —, — % 20
πυνθάνομαι, πεύσομαι, ἐπυθόμην, —, πέπυσμαι, — % 20
τρέπω, τρέψω, ἔτρεψα / ἐτραπόμην, τέτροφα, τέτραμμαι, ἐτράπην / ἐτρέφθην % 20 % CONSONANT_STEM_PERFECT_PI
φαίνω, φανῶ, ἔφηνα, πέφηνα, πέφασμαι, ἐφάνην % 20 % CONSONANT_STEM_PERFECT_NU
χρή, χρῆσται, —, —, —, — % 20
"##;

// pub trait HcDb {
//     fn insert_session(&self,
//         pool: &SqlitePool,
//         user_id: Uuid,
//         highest_unit: Option<u32>,
//         opponent_id: Option<Uuid>,
//         max_changes: u8,
//         practice_reps_per_verb: Option<i16>,
//         timestamp: i64) -> Result<Uuid, sqlx::Error>;
// }

#[derive(Serialize)]
struct GetMovesResponse {
    response_to: String,
    session_id: Uuid,
    moves: Vec<MoveResult>,
    success: bool,
}

#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

#[derive(Deserialize, Serialize)]
pub struct SessionsListResponse {
    response_to: String,
    sessions: Vec<SessionsListQuery>,
    success: bool,
    username: Option<String>,
    current_session: Option<SessionState>,
}

#[derive(Deserialize, Serialize)]
pub struct GetMoveQuery {
    qtype: String,
    session_id: sqlx::types::Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct GetSessions {
    qtype: String,
    current_session: Option<sqlx::types::Uuid>,
}

/// Entry point for our websocket route
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::HcGameServer>>,
    session: Session,
) -> Result<HttpResponse, Error> {
    if let Some(uuid) = login::get_user_id(session.clone()) {
        //println!("uuid {:?}", uuid);
        let db = req.app_data::<HcDb>().unwrap();
        let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();
        let username = login::get_username(session);
        ws::start(
            session::WsHcGameSession {
                id: uuid,
                hb: Instant::now(),
                room: server::MAIN_ROOM,
                name: None,
                addr: srv.get_ref().clone(),
                verbs: verbs.clone(),
                db: db.clone(),
                username,
            },
            &req,
            stream,
        )
    } else {
        Ok(HttpResponse::InternalServerError().finish())
    }
}

fn _get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn _get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

static INDEX_PAGE: &str = include_str!("index.html");
static CSP: &str = "style-src 'nonce-%NONCE%';script-src 'nonce-%NONCE%' 'wasm-unsafe-eval' \
                    'unsafe-inline'; object-src 'none'; base-uri 'none'";

async fn index_page() -> Result<HttpResponse, AWError> {
    let mut rng = rand::thread_rng();
    let csp_nonce: String = rng.gen::<u32>().to_string();

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header((CONTENT_SECURITY_POLICY, CSP.replace("%NONCE%", &csp_nonce)))
        .body(INDEX_PAGE.replace("%NONCE%", &csp_nonce)))
}

async fn get_sessions(
    (info, session, req): (web::Form<GetSessions>, Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session.clone()) {
        let username = login::get_username(session);

        //let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");

        let current_session = match info.current_session {
            Some(r) => Some(
                libhc::hc_get_move(db, user_id, false, r, verbs)
                    .await
                    .map_err(map_sqlx_error)?,
            ),
            _ => None,
        };
        let res = SessionsListResponse {
            response_to: "getsessions".to_string(),
            sessions: libhc::hc_get_sessions(db, user_id)
                .await
                .map_err(map_sqlx_error)?,
            success: true,
            username,
            current_session,
        };

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = StatusResponse {
            response_to: "getsessions".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn get_game_moves(
    (info, session, req): (web::Form<GetMovesQuery>, Session, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();

    if let Some(_user_id) = login::get_user_id(session.clone()) {
        let res = GetMovesResponse {
            response_to: "getgamemoves".to_string(),
            session_id: info.session_id,
            moves: libhc::hc_get_game_moves(db, &info)
                .await
                .map_err(map_sqlx_error)?,
            success: true,
        };

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = StatusResponse {
            response_to: "getmoves".to_string(),
            mesg: "error getting moves: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn create_session(
    (session, mut info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");

        let (mesg, success) =
            match libhc::hc_insert_session(db, user_id, &mut info, verbs, timestamp).await {
                Ok(_session_uuid) => ("inserted!".to_string(), true),
                Err(sqlx::Error::RowNotFound) => ("opponent not found!".to_string(), false),
                Err(e) => (format!("error inserting: {e:?}"), false),
            };
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg,
            success,
        };
        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn get_move(
    (info, req, session): (web::Form<GetMoveQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_get_move(db, user_id, false, info.session_id, verbs)
            .await
            .map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form: None,
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None, //time for prev answer
            response_to: "ask".to_string(),
            success: false,
            mesg: Some("not logged in".to_string()),
            verbs: None,
        };
        //let res = ("abc","def",);
        //Ok(HttpResponse::InternalServerError().finish())
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn enter(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_answer(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_sqlx_error)?;

        return Ok(HttpResponse::Ok().json(res));
    }
    let res = SessionState {
        session_id: info.session_id,
        move_type: MoveType::Practice,
        myturn: false,
        starting_form: None,
        answer: None,
        is_correct: None,
        correct_answer: None,
        verb: None,
        person: None,
        number: None,
        tense: None,
        voice: None,
        mood: None,
        person_prev: None,
        number_prev: None,
        tense_prev: None,
        voice_prev: None,
        mood_prev: None,
        time: None, //time for prev answer
        response_to: "ask".to_string(),
        success: false,
        mesg: Some("not logged in".to_string()),
        verbs: None,
    };
    Ok(HttpResponse::Ok().json(res))
}

async fn ask(
    (info, req, session): (web::Form<AskQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_ask(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form: None,
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None, //time for prev answer
            response_to: "ask".to_string(),
            success: false,
            mesg: Some("not logged in".to_string()),
            verbs: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn mf(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        let res = libhc::hc_mf_pressed(db, user_id, &info, timestamp, verbs)
            .await
            .map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form: None,
            answer: None,
            is_correct: None,
            correct_answer: None,
            verb: None,
            person: None,
            number: None,
            tense: None,
            voice: None,
            mood: None,
            person_prev: None,
            number_prev: None,
            tense_prev: None,
            voice_prev: None,
            mood_prev: None,
            time: None, //time for prev answer
            response_to: "ask".to_string(),
            success: false,
            mesg: Some("not logged in".to_string()),
            verbs: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    error: String,
    message: String,
}

#[derive(Error, Debug)]
pub struct PhilologusError {
    code: StatusCode,
    name: String,
    error: String,
}

impl std::fmt::Display for PhilologusError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "PhilologusError: {} {}: {}.",
            self.code.as_u16(),
            self.name,
            self.error
        )
    }
}

impl ResponseError for PhilologusError {
    fn error_response(&self) -> HttpResponse {
        let error_response = ErrorResponse {
            code: self.code.as_u16(),
            message: self.error.to_string(),
            error: self.name.to_string(),
        };
        HttpResponse::build(self.code).json(error_response)
    }
}

fn map_sqlx_error(e: sqlx::Error) -> PhilologusError {
    match e {
        sqlx::Error::Configuration(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Configuration: {e}"),
        },
        sqlx::Error::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Database: {e}"),
        },
        sqlx::Error::Io(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Io: {e}"),
        },
        sqlx::Error::Tls(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Tls: {e}"),
        },
        sqlx::Error::Protocol(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Protocol: {e}"),
        },
        sqlx::Error::RowNotFound => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx RowNotFound".to_string(),
        },
        sqlx::Error::TypeNotFound { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx TypeNotFound".to_string(),
        },
        sqlx::Error::ColumnIndexOutOfBounds { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnIndexOutOfBounds".to_string(),
        },
        sqlx::Error::ColumnNotFound(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx ColumnNotFound: {e}"),
        },
        sqlx::Error::ColumnDecode { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnDecode".to_string(),
        },
        sqlx::Error::Decode(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Decode: {e}"),
        },
        sqlx::Error::PoolTimedOut => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolTimeOut".to_string(),
        },
        sqlx::Error::PoolClosed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolClosed".to_string(),
        },
        sqlx::Error::WorkerCrashed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx WorkerCrashed".to_string(),
        },
        sqlx::Error::Migrate(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Migrate: {e}"),
        },
        _ => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx Unknown error".to_string(),
        },
    }
}

fn load_verbs(_path: &str) -> Vec<Arc<HcGreekVerb>> {
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

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // start ws server actor
    let app_state = Arc::new(AtomicUsize::new(0));
    let server = server::HcGameServer::new(app_state.clone()).start();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://db.sqlite?mode=rwc
    // let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
    //     panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
    // });

    // let db_path = "testing.sqlite?mode=rwc";
    // let options = SqliteConnectOptions::from_str(db_path)
    //     .expect("Could not connect to db.")
    //     .foreign_keys(true)
    //     .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    //     .read_only(false)
    //     .collation("PolytonicGreek", |l, r| {
    //         l.to_lowercase().cmp(&r.to_lowercase())
    //     });

    // let pool = PgPoolOptions::new()
    //     .max_connections(5)
    //     .connect("postgres://jwm:1234@localhost/hc").await?;

    //e.g. export HOPLITE_DB=postgres://jwm:1234@localhost/hc
    let db_string = std::env::var("HOPLITE_DB")
        .unwrap_or_else(|_| panic!("Environment variable for db string not set: HOPLITE_DB."));

    let hcdb = HcDb {
        db: PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_string)
            .await
            .expect("Could not connect to db."),
    };

    // let hcdb = HcDb { db: SqlitePool::connect_with(options)
    //     .await
    //     .expect("Could not connect to db.")
    // };

    let res = hcdb.create_db().await;
    if res.is_err() {
        println!("error: {res:?}");
    }

    //1. to make a new key:
    // let secret_key = Key::generate(); // only for testing: should use same key from .env file/variable, else have to login again on each restart
    // println!("key: {}{}", hex::encode( secret_key.signing() ), hex::encode( secret_key.encryption() ));

    //2. a simple example testing key
    //https://docs.rs/cookie/0.16.0/src/cookie/secure/key.rs.html#35
    // let key: &Vec<u8> = &(0..64).collect();
    // let secret_key = Key::from(key);

    //3. to load from string
    // let string_key_64_bytes = "c67ba35ad969a3f4255085c359f120bae733c5a5756187aaffab31c7c84628b6a9a02ce6a1e923a945609a884f913f83ea50675b184514b5d15c3e1a606a3fd2";
    // let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    // let secret_key = Key::from(&key);

    //4. or load from env
    //e.g. export HCKEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285
    let string_key_64_bytes =
        std::env::var("HCKEY").unwrap_or_else(|_| panic!("Key env not set: HCKEY."));
    let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    let secret_key = Key::from(&key);

    let cookie_secure = !cfg!(debug_assertions); //cookie is secure for release, not secure for debug builds

    //for flash messages on login page
    let message_store = CookieMessageStore::builder(
        secret_key.clone(), /*Key::from(hmac_secret.expose_secret().as_bytes())*/
    )
    .secure(cookie_secure)
    .same_site(SameSite::Strict)
    .build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();

    HttpServer::new(move || {
        App::new()
            .app_data(load_verbs("pp.txt"))
            .app_data(hcdb.clone())
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .wrap(
                middleware::DefaultHeaders::new()
                    // .add((CONTENT_SECURITY_POLICY,
                    //     HeaderValue::from_static("style-src 'nonce-2726c7f26c';\
                    //         script-src 'nonce-2726c7f26c' 'wasm-unsafe-eval' 'unsafe-inline'; object-src 'none'; base-uri 'none'")))
                    .add((
                        STRICT_TRANSPORT_SECURITY,
                        HeaderValue::from_static("max-age=31536000" /* 1 year */),
                    )),
            )
            .wrap(middleware::Compress::default()) // enable automatic response compression - usually register this first
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(cookie_secure) //cookie_secure must be false if testing without https
                    .cookie_same_site(SameSite::Strict)
                    .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                    .session_lifecycle(
                        PersistentSession::default()
                            .session_ttl(Duration::seconds(SECS_IN_10_YEARS)),
                    )
                    .cookie_name(String::from("hcid"))
                    .build(),
            )
            .wrap(message_framework.clone())
            .wrap(middleware::Logger::default()) // enable logger - always register Actix Web Logger middleware last
            .configure(config)
    })
    .workers(2)
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(index_page))
        .route("/login", web::get().to(login::login_get))
        .route("/login", web::post().to(login::login_post))
        .route("/newuser", web::get().to(login::new_user_get))
        .route("/newuser", web::post().to(login::new_user_post))
        .route("/logout", web::get().to(login::logout))
        //.route("/ws", web::get().to(ws_route))
        .service(web::resource("/ws").route(web::get().to(ws_route)))
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(web::resource("/enter").route(web::post().to(enter)))
        .service(web::resource("/new").route(web::post().to(create_session)))
        .service(web::resource("/list").route(web::post().to(get_sessions)))
        .service(web::resource("/getmove").route(web::post().to(get_move)))
        .service(web::resource("/getgamemoves").route(web::post().to(get_game_moves)))
        .service(web::resource("/ask").route(web::post().to(ask)))
        .service(web::resource("/mf").route(web::post().to(mf)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libhc::*;
    use actix_web::test;
    use sqlx::Executor;
    use tokio::sync::OnceCell;
    static ONCE: OnceCell<()> = OnceCell::const_new();

    async fn setup_test_db() {
        let db = HcDb {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect("postgres://jwm:1234@localhost/hctest")
                .await
                .expect("Could not connect to db."),
        };

        let _ = db.db.execute("DROP TABLE IF EXISTS moves;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS sessions;").await;
        let _ = db.db.execute("DROP TABLE IF EXISTS users;").await;

        let res = db.create_db().await;
        if res.is_err() {
            println!("error: {res:?}");
        }
    }

    pub async fn initialize_db_once() {
        ONCE.get_or_init(setup_test_db).await;
    }

    #[test]
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

    #[test]
    async fn test_change_verb() {
        assert!(hc_change_verbs(&vec![], 2));
        assert!(hc_change_verbs(&vec![1, 1, 2, 2], 2));
        assert!(!hc_change_verbs(&vec![1, 1, 2, 2, 2], 3));
        assert!(hc_change_verbs(&vec![1, 1, 1, 1, 1, 2, 2, 2, 2, 2], 5));
        assert!(!hc_change_verbs(&vec![1, 1, 1, 1, 2, 2, 2, 2, 2], 5));
    }

    #[test]
    async fn test_two_player() {
        initialize_db_once().await;

        let db = HcDb {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect("postgres://jwm:1234@localhost/hctest")
                .await
                .expect("Could not connect to db."),
        };

        let verbs = load_verbs("pp.txt");

        let mut timestamp = get_timestamp();

        let uuid1 = db
            .create_user("testuser1", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let uuid2 = db
            .create_user("testuser2", "abcdabcd", "user2@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid = db
            .create_user("testuser3", "abcdabcd", "user3@blah.com", timestamp)
            .await
            .unwrap();

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

        let s = hc_get_sessions(&db, uuid1).await;
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

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;

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

        let ss2 = hc_get_move(&db, uuid2, false, m.session_id, &verbs).await;

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

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;

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

        let ss2 = hc_get_move(&db, uuid2, false, m.session_id, &verbs).await;

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

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;
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

        let ss2 = hc_get_move(&db, uuid2, false, m.session_id, &verbs).await;

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

        let s = hc_get_sessions(&db, uuid1).await;
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

        let s = hc_get_sessions(&db, uuid2).await;
        //println!("s: {:?}", s);
        assert_eq!(s.as_ref().unwrap()[0].move_type, MoveType::AskTheirTurn);
        assert_eq!(s.as_ref().unwrap()[0].my_score, Some(1));
        assert_eq!(s.as_ref().unwrap()[0].their_score, Some(0));

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;

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

        let ss2 = hc_get_move(&db, uuid2, false, m.session_id, &verbs).await;

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

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;
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

        let ss2 = hc_get_move(&db, uuid2, false, m.session_id, &verbs).await;

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

    #[test]
    async fn test_practice() {
        initialize_db_once().await;
        let verbs = load_verbs("pp.txt");

        let db = HcDb {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect("postgres://jwm:1234@localhost/hctest")
                .await
                .expect("Could not connect to db."),
        };

        let timestamp = get_timestamp();

        let uuid1 = db
            .create_user("testuser4", "abcdabcd", "user1@blah.com", timestamp)
            .await
            .unwrap();
        let invalid_uuid = db
            .create_user("testuser6", "abcdabcd", "user3@blah.com", timestamp)
            .await
            .unwrap();

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

        let s = hc_get_sessions(&db, uuid1).await;
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

        let ss = hc_get_move(&db, uuid1, false, m.session_id, &verbs).await;

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