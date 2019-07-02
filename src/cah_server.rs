//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use rand::{self, rngs::ThreadRng, Rng};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use std::u64;

pub type CardId = u64;

#[derive(Default, Serialize, Deserialize)]
pub struct Card {
    pub content: String,
    pub id: CardId,
}

impl Card {
    //TODO: I'm not sure if this is a good way if doing it

    pub fn is_white_card(&self) -> bool {
        return self.id < CardId::max_value()/2;
    }

    pub fn is_black_card(&self) -> bool {
        return !self.is_white_card();
    }
}



#[derive(Default, Serialize, Deserialize)]
pub struct CardDeck {
    deck_name: String,
    black_cards: Vec<Card>,
    white_cards: Vec<Card>,
}

#[derive(Default)]
pub struct Player {
    name: String,
    id: Uuid,
}

#[derive(Default)]
pub struct DatabasePlayer {
    player: Player,
    email: String,
    password_hash: u128, 
}

#[derive(Default)]
pub struct Database {
    card_decks: HashMap<String, CardDeck>,
}

pub struct Match {
    players: Vec<Player>,
    player_hands: HashMap<String, Vec<String>>,
    player_submitted_card: HashMap<String, String>,
}


/// Message for chat server communications
pub mod messages{
    use actix::prelude::*;
    use uuid::Uuid;
    use crate::cah_server::CardId;

    /// Chat server sends this messages to session
    #[derive(Message)]
    pub struct Message(pub String);

    /// New chat session is created
    #[derive(Message)]
    pub struct Connect {
        pub addr: Recipient<Message>,
        pub user_id: Uuid,
    }

    /// Session is disconnected
    #[derive(Message)]
    pub struct Disconnect {
        pub id: Uuid,
    }

    /// Send message to specific room
    #[derive(Message)]
    pub struct ClientMessage {
        /// Id of the client session
        pub id: Uuid,
        /// Peer message
        pub msg: String,
        /// Room name
        pub room: String,
    }

    /// List of available rooms
    pub struct ListRooms;

    impl actix::Message for ListRooms {
        type Result = Vec<String>;
    }

    /// Join room, if room does not exists create new one.
    #[derive(Message)]
    pub struct Join {
        /// Client id
        pub id: Uuid,
        /// Room name
        pub name: String,
    }

    #[derive(Message)]
    pub struct SubmitCard {
        pub user_id: Uuid,
        pub card_id: CardId,
    }
}


/// `ChatServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct CahServer {
    sessions: HashMap<Uuid, Recipient<messages::Message>>,
    rooms: HashMap<String, HashSet<Uuid>>,
    database: Database,
    rng: ThreadRng,
}

impl Default for CahServer {
    fn default() -> CahServer {
        // default room
        let mut rooms = HashMap::new();
        rooms.insert("Main".to_owned(), HashSet::new());

        CahServer {
            sessions: HashMap::new(),
            rooms: rooms,
            database: Default::default(),
            rng: rand::thread_rng(),
        }
    }
}

impl CahServer {
    /// Send message to all users in the room
    fn send_message(&self, room: &str, message: &str, skip_id: Uuid) {
        if let Some(sessions) = self.rooms.get(room) {
            for id in sessions {
                if *id != skip_id {
                    if let Some(addr) = self.sessions.get(id) {
                        let _ = addr.do_send(messages::Message(message.to_owned()));
                    }
                }
            }
        }
    }
}

/// Make actor from `ChatServer`
impl Actor for CahServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<messages::Connect> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::Connect, _: &mut Context<Self>) -> Self::Result {
        println!("{} is connecting", msg.user_id);

        // notify all users in same room
        self.send_message(&"Main".to_owned(), "Someone joined", Uuid::nil());

        // register session with uuid
        self.sessions.insert(msg.user_id, msg.addr);

        // auto join session to Main room
        self.rooms.get_mut(&"Main".to_owned()).unwrap().insert(msg.user_id);

        // send id back
        // msg.user_id
    }
}

/// Handler for SubmitCard message
///
/// Register new session and assign unique id to this session
impl Handler<messages::SubmitCard> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::SubmitCard, _: &mut Context<Self>) -> Self::Result {
        println!("{} submitted the card: {}", msg.user_id, msg.card_id);
    }
}

/// Handler for Disconnect message.
impl Handler<messages::Disconnect> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::Disconnect, _: &mut Context<Self>) {
        println!("Someone disconnected");

        let mut rooms: Vec<String> = Vec::new();

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
        for room in rooms {
            self.send_message(&room, "Someone disconnected", Uuid::nil());
        }
    }
}

/// Handler for Message message.
impl Handler<messages::ClientMessage> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::ClientMessage, _: &mut Context<Self>) {
        self.send_message(&msg.room, msg.msg.as_str(), msg.id);
    }
}

/// Handler for `ListRooms` message.
impl Handler<messages::ListRooms> for CahServer {
    type Result = MessageResult<messages::ListRooms>;

    fn handle(&mut self, _: messages::ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::new();

        for key in self.rooms.keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<messages::Join> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::Join, _: &mut Context<Self>) {
        let messages::Join { id, name } = msg;
        let mut rooms = Vec::new();

        // remove session from all rooms
        for (n, sessions) in &mut self.rooms {
            if sessions.remove(&id) {
                rooms.push(n.to_owned());
            }
        }
        // send message to other users
        for room in rooms {
            self.send_message(&room, "Someone disconnected", Uuid::nil());
        }

        if self.rooms.get_mut(&name).is_none() {
            self.rooms.insert(name.clone(), HashSet::new());
        }
        self.send_message(&name, "Someone connected", id);
        self.rooms.get_mut(&name).unwrap().insert(id);
    }
}