//https://tokio.rs/tokio/topics/bridging
use crate::get_timestamp;
use crate::hc_answer;
use crate::hc_ask;
use crate::hc_create_db;
use crate::hc_create_user;
use crate::hc_get_game_moves;
use crate::hc_get_move;
use crate::hc_get_sessions;
use crate::hc_insert_session;
use crate::hc_mf_pressed;
use crate::hc_validate_credentials;
use crate::AnswerQuery;
use crate::AskQuery;
use crate::CreateSessionQuery;
use crate::Credentials;
use crate::GetMovesQuery;
use crate::GetSessions;
use crate::HcError;
use crate::HcGreekVerb;
use crate::MoveResult;
use crate::SessionState;
use crate::SessionsListResponse;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "sqlite")]
pub struct HcBlockingClient {
    inner_db: HcDbSqlite,
    /// A `current_thread` runtime for executing operations on the
    /// asynchronous client in a blocking manner.
    rt: tokio::runtime::Runtime,
}

#[cfg(feature = "sqlite")]
use crate::dbsqlite::HcDbSqlite;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqliteConnectOptions;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePool;

#[cfg(feature = "sqlite")]
impl HcBlockingClient {
    pub fn connect(options: SqliteConnectOptions) -> Result<HcBlockingClient, HcError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // Call the asynchronous connect method using the runtime.
        let inner_db = HcDbSqlite {
            db: rt.block_on(SqlitePool::connect_with(options)).unwrap(),
        };

        Ok(HcBlockingClient { inner_db, rt })
    }

    pub fn create_db(&self) -> Result<(), HcError> {
        self.rt.block_on(hc_create_db(&self.inner_db))
    }

    pub fn create_user(&self, name: &str, pwd: &str, email: &str) -> Result<Uuid, HcError> {
        self.rt.block_on(hc_create_user(
            &self.inner_db,
            name,
            pwd,
            email,
            get_timestamp(),
        ))
    }

    pub fn validate_credentials(&self, credentials: Credentials) -> Result<uuid::Uuid, HcError> {
        self.rt
            .block_on(hc_validate_credentials(&self.inner_db, credentials))
    }

    pub fn get_sessions(
        &self,
        user_id: Uuid,
        verbs: &[Arc<HcGreekVerb>],
        get_sessions: &GetSessions,
        username: Option<String>,
    ) -> Result<SessionsListResponse, HcError> {
        self.rt.block_on(hc_get_sessions(
            &self.inner_db,
            user_id,
            verbs,
            username,
            get_sessions,
        ))
    }

    pub fn insert_session(
        &self,
        user_id: Uuid,
        verbs: &[Arc<HcGreekVerb>],
        create_session: &CreateSessionQuery,
    ) -> Result<Uuid, HcError> {
        self.rt.block_on(hc_insert_session(
            &self.inner_db,
            user_id,
            create_session,
            verbs,
            get_timestamp(),
        ))
    }

    pub fn get_move(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        verbs: &[Arc<HcGreekVerb>],
    ) -> Result<SessionState, HcError> {
        self.rt.block_on(hc_get_move(
            &self.inner_db,
            user_id,
            false,
            session_id,
            verbs,
        ))
    }

    pub fn get_game_moves(
        &self,
        get_moves_query: &GetMovesQuery,
    ) -> Result<Vec<MoveResult>, HcError> {
        self.rt
            .block_on(hc_get_game_moves(&self.inner_db, get_moves_query))
    }

    pub fn answer(
        &self,
        user_id: Uuid,
        answer_query: &AnswerQuery,
        verbs: &[Arc<HcGreekVerb>],
    ) -> Result<SessionState, HcError> {
        self.rt.block_on(hc_answer(
            &self.inner_db,
            user_id,
            answer_query,
            get_timestamp(),
            verbs,
        ))
    }

    pub fn ask(
        &self,
        user_id: Uuid,
        ask_query: &AskQuery,
        verbs: &[Arc<HcGreekVerb>],
    ) -> Result<SessionState, HcError> {
        self.rt.block_on(hc_ask(
            &self.inner_db,
            user_id,
            ask_query,
            get_timestamp(),
            verbs,
        ))
    }

    pub fn mf(
        &self,
        user_id: Uuid,
        answer_query: &AnswerQuery,
        verbs: &[Arc<HcGreekVerb>],
    ) -> Result<SessionState, HcError> {
        self.rt.block_on(hc_mf_pressed(
            &self.inner_db,
            user_id,
            answer_query,
            get_timestamp(),
            verbs,
        ))
    }
}
