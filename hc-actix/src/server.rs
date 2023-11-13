//! `HcGameServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `HcGameServer`.

use actix::prelude::*;
use sqlx::types::Uuid;
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicUsize, Arc},
};

pub static MAIN_ROOM: Uuid = Uuid::from_u128(0x00000000000000000000000000000001);

/// game server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

/// Message for game server communications

/// New game session is created
#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub id: Uuid,
    pub addr: Recipient<Message>,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: Uuid,
}

/// Send message to specific room
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    /// Id of the client session
    pub id: Uuid,
    /// Peer message
    pub msg: String,
    /// Room name
    pub room: Uuid,
}

/// List of available rooms
pub struct ListRooms;

impl actix::Message for ListRooms {
    type Result = Vec<Uuid>;
}

/// Join room, if room does not exists create new one.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    /// Client ID
    pub user_uuid: Uuid,

    /// Room name
    pub game_uuid: Uuid,
}

/// `HcGameServer` manages game rooms and responsible for coordinating game session.
///
/// Implementation is very naïve.
#[derive(Debug)]
pub struct HcGameServer {
    sessions: HashMap<Uuid, Recipient<Message>>,
    rooms: HashMap<Uuid, HashSet<Uuid>>,
    #[allow(dead_code)]
    visitor_count: Arc<AtomicUsize>,
}

impl HcGameServer {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> HcGameServer {
        // default room
        let rooms = HashMap::new();
        //rooms.insert("main".to_owned(), HashSet::new());

        HcGameServer {
            sessions: HashMap::new(),
            rooms,
            visitor_count,
        }
    }
}

impl HcGameServer {
    /// Send message to all users in the room
    fn send_message(&self, room: Uuid, message: &str, skip_id: Uuid) {
        //println!("send message1 room: {:?}", room);
        if let Some(sessions) = self.rooms.get(&room) {
            //println!("send message2 room: {:?}", room);
            for id in sessions {
                //println!("send message3 room: {:?}, id: {:?}", room, id);
                if *id != skip_id {
                    //println!("send message4 room: {:?}", room);
                    if let Some(addr) = self.sessions.get(id) {
                        //println!("send message5 room: {:?}, id: {:?}", room, id);
                        addr.do_send(Message(message.to_owned()));
                    }
                }
            }
        }
    }
}

/// Make actor from `HcGameServer`
impl Actor for HcGameServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for HcGameServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        //println!("Someone joined");

        // notify all users in same room
        //self.send_message(MAIN_ROOM, "Someone joined", 0);

        // register session with random id
        //let id = self.rng.gen::<usize>();
        //println!("connect id: {:?}, addr: {:?}", msg.id, msg.addr);
        self.sessions.insert(msg.id, msg.addr);

        // auto join session to main room
        self.rooms
            .entry(MAIN_ROOM.to_owned())
            //.or_insert_with(HashSet::new)
            .or_default()
            .insert(msg.id);

        //let count = self.visitor_count.fetch_add(1, Ordering::SeqCst);
        //self.send_message(MAIN_ROOM, &format!("Total visitors {count}"), 0);

        // send id back
        //msg.id
        1
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for HcGameServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        //println!("Someone disconnected");

        let mut rooms: Vec<Uuid> = Vec::new();

        // remove address
        if self.sessions.remove(&msg.id).is_some() {
            // remove session from all rooms
            for (name, sessions) in &mut self.rooms {
                if sessions.remove(&msg.id) {
                    rooms.push(name.to_owned());
                }
            }
        }
        // send message to other users
        // for room in rooms {
        //     self.send_message(room, "Someone disconnected", 0);
        // }
    }
}

/// Handler for Message message.
impl Handler<ClientMessage> for HcGameServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.send_message(msg.room, msg.msg.as_str(), msg.id);
    }
}

/// Handler for `ListRooms` message.
impl Handler<ListRooms> for HcGameServer {
    type Result = MessageResult<ListRooms>;

    fn handle(&mut self, _: ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::new();

        for key in self.rooms.keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<Join> for HcGameServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join {
            user_uuid,
            game_uuid,
        } = msg;
        let mut rooms = Vec::new();

        // remove session from all rooms
        for (n, sessions) in &mut self.rooms {
            if sessions.remove(&user_uuid) {
                rooms.push(n.to_owned());
            }
        }
        // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", 0);
        // }

        //println!("joined room: {:?}, id: {:?}", name, id);

        self.rooms
            .entry(game_uuid)
            //.or_insert_with(HashSet::new)
            .or_default()
            .insert(user_uuid);

        //self.send_message(&name, "Someone connected", id);
    }
}
