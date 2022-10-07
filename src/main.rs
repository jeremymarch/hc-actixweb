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

use actix_web::{http::StatusCode, ResponseError};
use actix_web::cookie::Key;
use actix_session::Session;
use thiserror::Error;
use actix_files as fs;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web_flash_messages::FlashMessagesFramework;
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web::cookie::time::Duration;
use actix_session::config::PersistentSession;
const SECS_IN_10_YEARS: i64 = 60 * 60 * 24 * 7 * 4 * 12 * 10;

use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;

use polytonic_greek::hgk_compare_multiple_forms;

use std::sync::Arc;

use std::io;

//use uuid::Uuid;

use chrono::prelude::*;

//use mime;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use sqlx::postgres::PgPool;
use sqlx::postgres::PgPoolOptions;
use sqlx::FromRow;
use sqlx::types::Uuid;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

use hoplite_verbs_rs::*;
mod login;
mod db;
mod libhc;

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[derive(Clone)]
pub struct HcSqliteDb {
    //db:SqlitePool,
    db: sqlx::postgres::PgPool,
}

const PPS:&str = r##"παιδεύω, παιδεύσω, ἐπαίδευσα, πεπαίδευκα, πεπαίδευμαι, ἐπαιδεύθην % 2
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
//         practice_reps_per_verb: Option<u32>,
//         timestamp: i64) -> Result<Uuid, sqlx::Error>;
// }

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

#[derive(Deserialize,Serialize,PartialEq,Eq,Debug)]
pub struct HCVerbOption {
    id:i32,
    verb:String,
}

#[derive(Deserialize)]
pub struct AnswerQuery {
    qtype: String,
    answer: String,
    time: String,
    mf_pressed: bool,
    timed_out: bool,
    session_id:Uuid,
}

#[derive(Serialize)]
pub struct StatusResponse {
    response_to: String,
    mesg: String,
    success: bool,
}

#[derive(Deserialize,Serialize)]
pub struct CreateSessionQuery {
    qtype:String,
    unit: String,
    opponent:String,
    practice_reps_per_verb:Option<i32>,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct SessionsListQuery {
    session_id: sqlx::types::Uuid,
    challenged: Option<sqlx::types::Uuid>, //the one who didn't start the game, or null for practice
    opponent: Option<sqlx::types::Uuid>,
    opponent_name: Option<String>,
    timestamp: i64,
    myturn: bool,
    move_type: MoveType,
}

#[derive(Deserialize,Serialize)]
pub struct SessionsListResponse {
    response_to: String,
    sessions: Vec<SessionsListQuery>,
    success: bool,
    username: Option<String>,
}

#[derive(Deserialize,Serialize)]
pub struct GetMoveQuery {
    session_id:sqlx::types::Uuid,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct UserResult {
    user_id: sqlx::types::Uuid,
    user_name: String,
    password: String,
    email: String,
    user_type: i32,
    timestamp: i64,
}

#[derive(Deserialize,Serialize, FromRow)]
pub struct SessionResult {
    session_id: Uuid, 
    challenger_user_id: Uuid,
    challenged_user_id: Option<Uuid>,
    highest_unit: Option<i32>,
    custom_verbs: Option<String>, 
    max_changes: i32,
    challenger_score: Option<i32>,
    challenged_score: Option<i32>,
    practice_reps_per_verb: Option<i32>,
    timestamp: i64,
}

#[derive(Deserialize, Serialize, FromRow)]
pub struct MoveResult {
    move_id: sqlx::types::Uuid,
    session_id: sqlx::types::Uuid,
    ask_user_id: Option<sqlx::types::Uuid>,
    answer_user_id: Option<sqlx::types::Uuid>,
    verb_id: Option<i32>,
    person: Option<i32>,
    number: Option<i32>,
    tense: Option<i32>,
    mood: Option<i32>,
    voice: Option<i32>,
    answer: Option<String>,
    correct_answer: Option<String>,
    is_correct: Option<bool>,
    time: Option<String>,
    timed_out: Option<bool>,
    mf_pressed: Option<bool>,
    asktimestamp: i64,
    answeredtimestamp: Option<i64>,
}

#[derive(Deserialize,Serialize)]
pub struct AskQuery {
    session_id: Uuid,
    person: i32,
    number: i32,
    tense: i32,
    voice: i32,
    mood: i32,
    verb: i32,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct SessionState {
    session_id: Uuid,
    move_type: MoveType,
    myturn: bool,
    starting_form:Option<String>,
    answer:Option<String>,
    is_correct: Option<bool>,
    correct_answer:Option<String>,
    verb: Option<i32>,
    person: Option<i32>,
    number: Option<i32>,
    tense: Option<i32>,
    voice: Option<i32>,
    mood: Option<i32>,
    person_prev: Option<i32>,
    number_prev: Option<i32>,
    tense_prev: Option<i32>,
    voice_prev: Option<i32>,
    mood_prev: Option<i32>,
    time: Option<String>, //time for prev answer
    response_to:String,
    success:bool,
    mesg:Option<String>,
    verbs:Option<Vec<HCVerbOption>>,
}

fn get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

async fn get_sessions(
    (session, req): (Session, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();

    if let Some(user_id) = login::get_user_id(session.clone()) {

        let username = login::get_username(session);

        //let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");
        
        let res = SessionsListResponse {
            response_to: "getsessions".to_string(),
            sessions: libhc::hc_get_sessions(db, user_id).await.map_err(map_sqlx_error)?,
            success: true,
            username,
        };

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = StatusResponse {
            response_to: "getsessions".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn create_session(
    (session, info, req): (Session, web::Form<CreateSessionQuery>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {

        let timestamp = get_timestamp();
        //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        //let user_agent = get_user_agent(&req).unwrap_or("");

        let (mesg, success) = match libhc::hc_insert_session(db, user_id, &info, verbs, timestamp).await {
            Ok(_session_uuid) => {
                ("inserted!".to_string(), true) 
            },
            Err(sqlx::Error::RowNotFound) => {
                ("opponent not found!".to_string(), false)
            },
            Err(e) => {
                (format!("error inserting: {:?}", e), false)
            }
        };
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg,
            success,
        };
        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = StatusResponse {
            response_to: "newsession".to_string(),
            mesg: "error inserting: not logged in".to_string(),
            success: false,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn get_move(
    (info, req, session): (web::Form<GetMoveQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    //"ask", prev form to start from or null, prev answer and is_correct, correct answer

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_get_move(db, user_id, &info, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
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
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
            verbs: None,
        };
        //let res = ("abc","def",);
        //Ok(HttpResponse::InternalServerError().finish())
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn enter(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {

        let res = libhc::hc_answer(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        return Ok(HttpResponse::Ok().json(res));
    }
    let res = SessionState {
        session_id: info.session_id,
        move_type: MoveType::Practice,
        myturn: false,
        starting_form:None,
        answer:None,
        is_correct: None,
        correct_answer:None,
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
        time: None,//time for prev answer
        response_to:"ask".to_string(),
        success:false,
        mesg:Some("not logged in".to_string()),
        verbs: None,
    };
    Ok(HttpResponse::Ok().json(res))
}

async fn ask(
    (info, req, session): (web::Form<AskQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_ask(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
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
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
            verbs: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn mf(
    (info, req, session): (web::Form<AnswerQuery>, HttpRequest, Session)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<HcSqliteDb>().unwrap();
    let verbs = req.app_data::<Vec<Arc<HcGreekVerb>>>().unwrap();

    let timestamp = get_timestamp();
    //let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
    //let user_agent = get_user_agent(&req).unwrap_or("");

    if let Some(user_id) = login::get_user_id(session) {
        
        let res = libhc::hc_mf_pressed(db, user_id, &info, timestamp, verbs).await.map_err(map_sqlx_error)?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = SessionState {
            session_id: info.session_id,
            move_type: MoveType::Practice,
            myturn: false,
            starting_form:None,
            answer:None,
            is_correct: None,
            correct_answer:None,
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
            time: None,//time for prev answer
            response_to:"ask".to_string(),
            success:false,
            mesg:Some("not logged in".to_string()),
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
            error: format!("sqlx Configuration: {}", e),
        },
        sqlx::Error::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Database: {}", e),
        },
        sqlx::Error::Io(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Io: {}", e),
        },
        sqlx::Error::Tls(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Tls: {}", e),
        },
        sqlx::Error::Protocol(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Protocol: {}", e),
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
            error: format!("sqlx ColumnNotFound: {}", e),
        },
        sqlx::Error::ColumnDecode { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnDecode".to_string(),
        },
        sqlx::Error::Decode(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Decode: {}", e),
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
            error: format!("sqlx Migrate: {}", e),
        },
        _ => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx Unknown error".to_string(),
        },
    }
}

fn load_verbs(path:&str) -> Vec<Arc<HcGreekVerb>> {
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
    let pp_lines = PPS.split("\n");
    for (idx, line) in pp_lines.enumerate() {        
        if !line.starts_with('#') && line.len() > 0 { //skip commented lines
            if let Some(l) = HcGreekVerb::from_string_with_properties(idx as u32, &line) {
                //println!("line: {} {}", idx, line);
                verbs.push(Arc::new(l));
            }
        }
    }

    verbs
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    
    HttpServer::new(move || {

        App::new()
        .service(fs::Files::new("/", "./static").prefer_utf8(true).index_file("index.html"))
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}
