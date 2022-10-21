use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web_actors::ws;
use crate::server;

use crate::GetMoveQuery;
use sqlx::types::Uuid;

use std::sync::Arc;
use hoplite_verbs_rs::HcGreekVerb;
use crate::HcSqliteDb;
use crate::map_sqlx_error;
use crate::libhc;
use crate::AnswerQuery;
use crate::get_timestamp;
use crate::SessionsListResponse;
use crate::GetSessions;
use crate::StatusResponse;
use crate::MoveType;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct WsChatSession {
    /// unique session id
    pub id: Uuid,

    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    pub hb: Instant,

    /// joined room
    pub room: Uuid,

    /// peer name
    pub name: Option<String>,

    /// Chat server
    pub addr: Addr<server::ChatServer>,
    pub verbs: Vec<Arc<HcGreekVerb>>,
    pub db: HcSqliteDb,
    pub username: Option<String>,
}

impl WsChatSession {
    /// helper method that sends ping to client every 5 seconds (HEARTBEAT_INTERVAL).
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // notify chat server
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

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with ChatServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsChatSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                id: self.id,
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                // match res {
                //     Ok(res) => act.id = res,
                //     // something is wrong with chat server
                //     _ => ctx.stop(),
                // }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(server::Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<server::Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
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
                // we check for /sss type of messages
                if m.starts_with('/') {
                    // let v: Vec<&str> = m.splitn(2, ' ').collect();
                    // match v[0] {
                        // "/list" => {
                        //     // Send ListRooms message to chat server and wait for
                        //     // response
                        //     println!("List rooms");
                        //     self.addr
                        //         .send(server::ListRooms)
                        //         .into_actor(self)
                        //         .then(|res, _, ctx| {
                        //             match res {
                        //                 Ok(rooms) => {
                        //                     for room in rooms {
                        //                         ctx.text(room);
                        //                     }
                        //                 }
                        //                 _ => println!("Something is wrong"),
                        //             }
                        //             fut::ready(())
                        //         })
                        //         .wait(ctx)
                        //     // .wait(ctx) pauses all events in context,
                        //     // so actor wont receive any new messages until it get list
                        //     // of rooms back
                        // }
                    //     "/join" => {
                    //         if v.len() == 2 {
                    //             self.room = v[1].to_owned();
                    //             self.addr.do_send(server::Join {
                    //                 id: self.id,
                    //                 name: self.room.clone(),
                    //             });

                    //             ctx.text("joined");
                    //         } else {
                    //             ctx.text("!!! room name is required");
                    //         }
                    //     }
                    //     "/name" => {
                    //         if v.len() == 2 {
                    //             self.name = Some(v[1].to_owned());
                    //         } else {
                    //             ctx.text("!!! name is required");
                    //         }
                    //     }
                    //     _ => ctx.text(format!("!!! unknown command: {m:?}")),
                    // }
                } else {
                    let msg = if let Some(ref name) = self.name {
                        format!("{name}: {m}")
                    } else {
                        m.to_owned()
                    };
                    
//https://stackoverflow.com/questions/64434912/how-to-correctly-call-async-functions-in-a-websocket-handler-in-actix-web
//https://stackoverflow.com/questions/72068485/how-use-postgres-deadpool-postgres-with-websocket-actix-actix-web-actors

                    let db = self.db.clone();
                    let verbs = self.verbs.clone();
                    let user_id = self.id;
                    //let oid = self.id.clone();
                    //let room = self.room.clone();
                    let addr = ctx.address();//self.addr.clone();
                    let timestamp = get_timestamp();
                    let username = self.username.clone();
                    if msg.contains("getmove") {
                        if let Ok(info) = serde_json::from_str::<GetMoveQuery>(&msg) {
                            //join game room
                            self.addr.do_send(server::Join {
                                id: user_id,
                                name: info.session_id,
                            });
                            let fut = async move {
                                if let Ok(res) = libhc::hc_get_move(&db, user_id, false, &info, &verbs).await {
                                    if let Ok(resjson) = serde_json::to_string(&res) {
                                        let _ = addr.send(server::Message(resjson)).await;
                                    }
                                }
                            };
                            let fut = actix::fut::wrap_future::<_, Self>(fut);
                            ctx.spawn(fut);   
                        } 
                    }
                    else if msg.contains("newsession") {
                        if let Ok(info) = serde_json::from_str(&msg) {
                            let fut = async move {
                                let (mesg, success) = match libhc::hc_insert_session(&db, user_id, &info, &verbs, timestamp).await {
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
                                if let Ok(resjson) = serde_json::to_string(&res) {
                                    let _ = addr.send(server::Message(resjson)).await;
                                }
                            };
                            let fut = actix::fut::wrap_future::<_, Self>(fut);
                            ctx.spawn(fut);
                        }
                    }
                    else if msg.contains("ask") {
                        if let Ok(info) = serde_json::from_str(&msg) {
                            let addr2 = self.addr.clone();
                            let fut = async move {
                                if let Ok(res) = libhc::hc_ask(&db, user_id, &info, timestamp, &verbs).await {

                                    if res.move_type != MoveType::Practice {
                                        let gm = GetMoveQuery {
                                            qtype: "getmove".to_string(),
                                            session_id: info.session_id,
                                        };
                                        if let Ok(gm_res) = libhc::hc_get_move(&db, user_id, true, &gm, &verbs).await {
                                            if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                                //println!("send to room {:?}", info.session_id);
                                                addr2.do_send(server::ClientMessage {
                                                    id: user_id,
                                                    msg: gm_resjson.clone(),
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
                    }
                    else if msg.contains("submit") {
                        if let Ok(info) = serde_json::from_str(&msg) {
                            let addr2 = self.addr.clone();
                            let fut = async move {
                                if let Ok(res) = libhc::hc_answer(&db, user_id, &info, timestamp, &verbs).await {

                                    if res.move_type != MoveType::Practice {
                                        let gm = GetMoveQuery {
                                            qtype: "getmove".to_string(),
                                            session_id: info.session_id,
                                        };
                                        if let Ok(gm_res) = libhc::hc_get_move(&db, user_id, true, &gm, &verbs).await {
                                            if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                                //println!("send to room {:?}", info.session_id);
                                                addr2.do_send(server::ClientMessage {
                                                    id: user_id,
                                                    msg: gm_resjson.clone(),
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
                    }
                    else if msg.contains("mfpressed") {
                        if let Ok(info) = serde_json::from_str(&msg) {
                            let addr2 = self.addr.clone();
                            let fut = async move {
                                if let Ok(res) = libhc::hc_mf_pressed(&db, user_id, &info, timestamp, &verbs).await {

                                    if res.move_type != MoveType::Practice && res.is_correct == Some(false) {
                                        let gm = GetMoveQuery {
                                            qtype: "getmove".to_string(),
                                            session_id: info.session_id,
                                        };
                                        if let Ok(gm_res) = libhc::hc_get_move(&db, user_id, true, &gm, &verbs).await {
                                            if let Ok(gm_resjson) = serde_json::to_string(&gm_res) {
                                                //println!("send to room {:?}", info.session_id);
                                                addr2.do_send(server::ClientMessage {
                                                    id: user_id,
                                                    msg: gm_resjson.clone(),
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
                    }
                    else if msg.contains("getsessions") {
                        if let Ok(info) = serde_json::from_str::<GetSessions>(&msg) {
                            let fut = async move {
                                if let Ok(sessions) = libhc::hc_get_sessions(&db, user_id).await {
                                    let res = SessionsListResponse {
                                        response_to: "getsessions".to_string(),
                                        sessions,
                                        success: true,
                                        username,
                                    };
                                    if let Ok(resjson) = serde_json::to_string(&res) {
                                        let _ = addr.send(server::Message(resjson)).await;
                                    }
                                }
                            };
                            let fut = actix::fut::wrap_future::<_, Self>(fut);
                            ctx.spawn(fut);
                        }
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
