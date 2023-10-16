use crate::server;
use actix::prelude::*;
use actix_web_actors::ws;
use std::time::{Duration, Instant};

use crate::GetMoveQuery;
use sqlx::types::Uuid;

use crate::GetSessions;
use crate::HcDbPostgres;
use crate::MoveType;
use crate::SessionsListResponse;
use crate::StatusResponse;
use hoplite_verbs_rs::HcGreekVerb;
use libhc::HcDb;
use libhc::HcError;
use std::sync::Arc;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct WsHcGameSession {
    /// unique session id
    pub id: Uuid,

    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    pub hb: Instant,

    /// joined room
    pub room: Uuid,

    /// peer name
    pub name: Option<String>,

    pub addr: Addr<server::HcGameServer>,
    pub verbs: Vec<Arc<HcGreekVerb>>,
    pub db: HcDbPostgres,
    pub username: Option<String>,
}

impl WsHcGameSession {
    /// helper method that sends ping to client every 5 seconds (HEARTBEAT_INTERVAL).
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // notify game server
                act.addr.do_send(server::Disconnect { id: act.id });

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl Actor for WsHcGameSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with HcGameServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in game server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsHcGameSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                id: self.id,
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|_res, _act, _ctx| {
                // match res {
                //     Ok(res) => act.id = res,
                //     // something is wrong with game server
                //     _ => ctx.stop(),
                // }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify game server
        self.addr.do_send(server::Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle messages from game server, we simply send it to peer websocket
impl Handler<server::Message> for WsHcGameSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsHcGameSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        log::debug!("WEBSOCKET MESSAGE: {msg:?}");
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                let m = text.trim();
                let msg = if let Some(ref name) = self.name {
                    format!("{name}: {m}")
                } else {
                    m.to_owned()
                };

                //https://stackoverflow.com/questions/64434912/how-to-correctly-call-async-functions-in-a-websocket-handler-in-actix-web
                //https://stackoverflow.com/questions/72068485/how-use-postgres-deadpool-postgres-with-websocket-actix-actix-web-actors
                //https://github.com/agmcleod/sc-predictions-server/blob/dev/server/src/websocket/client_messages.rs
                let db = self.db.clone();
                let verbs = self.verbs.clone();
                let user_id = self.id;
                //let oid = self.id.clone();
                //let room = self.room.clone();
                let addr = ctx.address(); //self.addr.clone();
                let timestamp = libhc::get_timestamp();
                let username = self.username.clone();
                if msg.contains("getmove") {
                    if let Ok(info) = serde_json::from_str::<GetMoveQuery>(&msg) {
                        //join game room
                        self.addr.do_send(server::Join {
                            user_uuid: user_id,
                            game_uuid: info.session_id,
                        });
                        let fut = async move {
                            let mut tx = db.begin_tx().await.unwrap();
                            if let Ok(res) = libhc::hc_get_move_tr(
                                &mut tx,
                                user_id,
                                false,
                                info.session_id,
                                &verbs,
                            )
                            .await
                            {
                                tx.commit_tx().await.unwrap();
                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            }
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                } else if msg.contains("newsession") {
                    if let Ok(mut info) = serde_json::from_str(&msg) {
                        let fut = async move {
                            let (mesg, success) = match libhc::hc_insert_session(
                                &db, user_id, &mut info, &verbs, timestamp,
                            )
                            .await
                            {
                                Ok(_session_uuid) => ("inserted!".to_string(), true),
                                Err(HcError::UnknownError) => {
                                    ("opponent not found!".to_string(), false)
                                }
                                Err(e) => (format!("error inserting: {e:?}"), false),
                            };
                            let res = StatusResponse {
                                response_to: "newsession".to_string(),
                                mesg,
                                success,
                            };
                            if let Ok(resjson) = serde_json::to_string(&res) {
                                let _ = addr.send(server::Message(resjson)).await;
                            }
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                } else if msg.contains("ask") {
                    if let Ok(info) = serde_json::from_str(&msg) {
                        let addr2 = self.addr.clone();
                        let fut = async move {
                            if let Ok(res) =
                                libhc::hc_ask(&db, user_id, &info, timestamp, &verbs).await
                            {
                                if res.move_type != MoveType::Practice {
                                    let gm = GetMoveQuery {
                                        qtype: "getmove".to_string(),
                                        session_id: info.session_id,
                                    };
                                    let mut tx = db.begin_tx().await.unwrap();
                                    if let Ok(gm_res) = libhc::hc_get_move_tr(
                                        &mut tx,
                                        user_id,
                                        true,
                                        gm.session_id,
                                        &verbs,
                                    )
                                    .await
                                    {
                                        tx.commit_tx().await.unwrap();
                                        if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                            //println!("send to room {:?}", info.session_id);
                                            addr2.do_send(server::ClientMessage {
                                                id: user_id,
                                                msg: gm_resjson,
                                                room: info.session_id,
                                            });
                                        }
                                    }
                                }
                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            }
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                } else if msg.contains("submit") {
                    if let Ok(info) = serde_json::from_str(&msg) {
                        let addr2 = self.addr.clone();
                        let fut = async move {
                            if let Ok(res) =
                                libhc::hc_answer(&db, user_id, &info, timestamp, &verbs).await
                            {
                                if res.move_type != MoveType::Practice {
                                    let gm = GetMoveQuery {
                                        qtype: "getmove".to_string(),
                                        session_id: info.session_id,
                                    };
                                    let mut tx = db.begin_tx().await.unwrap();
                                    if let Ok(gm_res) = libhc::hc_get_move_tr(
                                        &mut tx,
                                        user_id,
                                        true,
                                        gm.session_id,
                                        &verbs,
                                    )
                                    .await
                                    {
                                        tx.commit_tx().await.unwrap();
                                        if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                            //println!("send to room {:?}", info.session_id);
                                            addr2.do_send(server::ClientMessage {
                                                id: user_id,
                                                msg: gm_resjson,
                                                room: info.session_id,
                                            });
                                        }
                                    }
                                }

                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            }
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                } else if msg.contains("mfpressed") {
                    if let Ok(info) = serde_json::from_str(&msg) {
                        let addr2 = self.addr.clone();
                        let fut = async move {
                            if let Ok(res) =
                                libhc::hc_mf_pressed(&db, user_id, &info, timestamp, &verbs).await
                            {
                                if res.move_type != MoveType::Practice
                                    && res.is_correct == Some(false)
                                {
                                    let gm = GetMoveQuery {
                                        qtype: "getmove".to_string(),
                                        session_id: info.session_id,
                                    };
                                    let mut tx = db.begin_tx().await.unwrap();
                                    if let Ok(gm_res) = libhc::hc_get_move_tr(
                                        &mut tx,
                                        user_id,
                                        true,
                                        gm.session_id,
                                        &verbs,
                                    )
                                    .await
                                    {
                                        tx.commit_tx().await.unwrap();
                                        if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                            //println!("send to room {:?}", info.session_id);
                                            addr2.do_send(server::ClientMessage {
                                                id: user_id,
                                                msg: gm_resjson,
                                                room: info.session_id,
                                            });
                                        }
                                    }
                                }

                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            }
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                } else if msg.contains("getsessions") {
                    if let Ok(info) = serde_json::from_str::<GetSessions>(&msg) {
                        if let Some(session_id) = info.current_session {
                            //join game room
                            self.addr.do_send(server::Join {
                                user_uuid: user_id,
                                game_uuid: session_id,
                            });
                        }
                        let fut = async move {
                            let current_session = match info.current_session {
                                Some(r) => {
                                    let mut tx = db.begin_tx().await.unwrap();
                                    match libhc::hc_get_move_tr(&mut tx, user_id, false, r, &verbs)
                                        .await
                                    {
                                        Ok(res) => {
                                            tx.commit_tx().await.unwrap();
                                            Some(res)
                                        }
                                        Err(_) => {
                                            tx.commit_tx().await.unwrap();
                                            None
                                        }
                                    }
                                }
                                _ => None,
                            };
                            let mut tx = db.begin_tx().await.unwrap();
                            if let Ok(sessions) = libhc::hc_get_sessions_tr(&mut tx, user_id).await
                            {
                                let res = SessionsListResponse {
                                    response_to: "getsessions".to_string(),
                                    sessions,
                                    success: true,
                                    username,
                                    current_session,
                                };
                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            }
                            tx.commit_tx().await.unwrap();
                        };
                        let fut = actix::fut::wrap_future::<_, Self>(fut);
                        ctx.spawn(fut);
                    }
                }
            }
            ws::Message::Binary(_) => println!("Unexpected binary"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}
