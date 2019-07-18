//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use rand::{self, rngs::ThreadRng, Rng};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use json::JsonValue;
use std::sync::RwLock;

use std::u64;
use crate::CookieToken;
use crate::messages;

use sha2::Sha512;
use sha2::Digest;
use generic_array::GenericArray;

pub type CardId = u64;
type ShaImpl = Sha512;
type PasswordHash = GenericArray<u8, <ShaImpl as Digest>::OutputSize>;

#[derive(Default, Clone, Serialize, Deserialize)]
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



#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CardDeck {
    deck_name: String,
    black_cards: Vec<Card>,
    white_cards: Vec<Card>,
}

#[derive(Default, Debug, Clone)]
pub struct Player {
    name: String,
    id: Uuid,
}

#[derive(Default, Debug, Clone)]
pub struct DatabasePlayer {
    player: Player,
    email: String,
    password_hash: PasswordHash, 
    salt: Uuid,
}

#[derive(Default)]
pub struct Database {
    card_decks: HashMap<String, CardDeck>,
    players: Vec<DatabasePlayer>,
}

#[derive(Default)]
pub struct Match {
    players: Vec<Player>,
    player_hands: HashMap<String, Vec<String>>,
    player_submitted_card: HashMap<String, String>,
}


// /// Message for chat server communications
// pub mod messages {
//     use actix::prelude::*;
//     use uuid::Uuid;
//     use crate::cah_server::{CardId, Card, Player};
//     use crate::CookieToken;

    
// }

/// `CahServer`(Cards against humanity server) manages matches and responsible for coordinating
/// session. 
pub struct CahServer {
    socket_actors: HashMap<CookieToken, Recipient<messages::outgoing::Message>>,
    // The cookie token to player Uuid, the Uuid is the reprisentation internally.
    sessions: RwLock<HashMap<CookieToken, Uuid>>, //TODO: Clear sessins after a couple hours/days, so the ram doesn't quitely go downwards.
    matches: HashMap<String, Match>,
    database: RwLock<Database>,
    rng: ThreadRng,
}

impl Default for CahServer {
    fn default() -> CahServer {
        // default room
        let mut matches = HashMap::new();
        matches.insert("Main".to_owned(), Match::default());
        matches.insert("Second Room".to_owned(), Match::default());

        CahServer {
            socket_actors: HashMap::new(),
            sessions: Default::default(),
            matches: matches,
            database: Default::default(),
            rng: rand::thread_rng(),
        }
    }
}

impl CahServer {
    //TODO: Optimize
    fn get_room_from_uuid(&self, user_id: &Uuid) -> Option<String> {
        for room in self.matches.iter() {
            for player in room.1.players.iter() {
                if &player.id == user_id {
                    return Some(room.0.clone());
                }
            }
        }

        None
    }

    //TODO: Optimize
    fn get_cookie_token_from_user_id(&self, user_id: &Uuid) -> Option<CookieToken> {
        for token_and_uuid in self.sessions.read().unwrap().iter() {
            let (token, uuid) = token_and_uuid;
            if uuid == user_id {
                return Some(token.clone());
            }
        }

        None
    }

    fn get_user_id(&self, cookie_token: &CookieToken) -> Option<Uuid> {
        match self.sessions.read().unwrap().get(cookie_token) {
            Some(uuid) => Some(uuid.clone()),
            None => None,
        }
    }
}

/// Make actor from `ChatServer`
impl Actor for CahServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

fn hash_password(salt: &Uuid, password: &str) -> PasswordHash {
    let mut total_string = salt.to_string();
    total_string.push_str(password);

    let mut sha =  ShaImpl::new();
    sha.input(total_string);
    sha.result()
}

impl Handler<messages::incomming::RegisterAccount> for CahServer {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: messages::incomming::RegisterAccount, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(db_player) = self.database.read().unwrap().players.iter().find(|&db_player| db_player.email == msg.email || db_player.player.name == msg.username) {
            return if db_player.email == msg.email {
                Err("User already exists with email".to_owned())
            } else { // we only check for email and username, so username should be equal here.
                Err("User already exists with username".to_owned())
            };
        }

        let salt = Uuid::new_v4();
        let password_hash = hash_password(&salt, &msg.password);
        let new_db_player = DatabasePlayer{player: Player{name: msg.username, id: Uuid::new_v4()}, email: msg.email, password_hash: password_hash, salt: salt};
        println!("Registering new user: {:?}", &new_db_player);
        self.database.get_mut().unwrap().players.push(new_db_player);

        Ok(())
        
    }
}

impl Handler<messages::incomming::Login> for CahServer {
    type Result = Result<CookieToken, String>;

    //TODO: How to handle two people fighting over a account?
    fn handle(&mut self, msg: messages::incomming::Login, _ctx: &mut Context<Self>) -> Self::Result {
        let db = self.database.get_mut().unwrap();
        if let Some(db_player) = db.players.iter().find(|&db_player| db_player.player.name == msg.username_or_email || db_player.email == msg.username_or_email) {
            let password_hash = hash_password(&db_player.salt, &msg.password);
            if password_hash == db_player.password_hash {
                let sessions = self.sessions.get_mut().unwrap();
                let player_id = db_player.player.id.clone();
                //TODO: Optimize this, there can only be one.
                sessions.retain(|&_key, &mut value| value != player_id);
                let new_cookie_token = CookieToken::new_v4();
                sessions.insert(new_cookie_token, player_id);

                Ok(new_cookie_token)
            } else {
                Err("Wrong password!".to_owned())
            }
        } else {
            Err("Username/Email not found".to_owned())
        }
    }
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<messages::incomming::Connect> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::Connect, ctx: &mut Context<Self>) -> Self::Result {
        // register session with token
        self.socket_actors.insert(msg.token.clone(), msg.addr);

        let user_id;
        if let Some(user_id_) = self.get_user_id(&msg.token) {
            user_id = user_id_;
        } else {
            println!("No user could be found with cookie token: {}. Creating temp account", &msg.token);
            //TODO: Not do this
            user_id = Uuid::new_v4();
            self.sessions.get_mut().unwrap().insert(msg.token.clone(), user_id.clone());
            self.database.get_mut().unwrap().players.push(DatabasePlayer{player: Player{id: user_id.clone(), name: format!("Temp account name for {}", &user_id)}, email: "".to_owned(), password_hash: Default::default(), salt: Uuid::new_v4()});
        }

        println!("{} is connecting", &user_id);

        let user_id = user_id;
        let db_player = self.database.get_mut().unwrap().players.iter().find(|&db_player| &db_player.player.id == &user_id);
        if db_player.is_none() {
            println!("ERROR: Cannot find player with id:{} in the database, pleaes make an account first! Thanks you!", user_id);
            return;
        }
        let player = db_player.unwrap().player.clone();

        // auto join session to Main room
        self.matches.get_mut(&"Main".to_owned()).unwrap().players.push(player.clone());

        //TODO: Let this be choosen by the client
        let room = "Main".to_owned();

        for i in 0..4 {
            //TODO: Not hardcode cards...
            let card = Card{content: format!("A card from the server! {}", i), id: i};
            ctx.address().do_send(messages::outgoing::AddCardToHand{room: room.clone(), player: player.clone(), card: card});
        }
    }
}

impl Handler<messages::outgoing::AddCardToHand> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::outgoing::AddCardToHand, ctx: &mut Context<Self>) -> Self::Result {
        let mut message = json::parse(r#"{"type": "addCardToHand"}"#).unwrap();
        message["card_content"] = JsonValue::String(msg.card.content);
        message["card_id"] = JsonValue::Number(json::number::Number::from(msg.card.id));
        let message_json = json::stringify(message);

        if let Some(cookie_token) = self.get_cookie_token_from_user_id(&msg.player.id) {
            let recipient = &self.socket_actors[&cookie_token];
            let msg_result = recipient.do_send(messages::outgoing::Message(message_json));
            if msg_result.is_err() {
                //TODO: Handle this or something
                debug_assert!(false);
            }
        }
    }
}

/// Handler for SubmitCard message
///
/// Register new session and assign unique id to this session
impl Handler<messages::incomming::SubmitCard> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::SubmitCard, _: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            let room = self.get_room_from_uuid(&user_id);
            match room {
                Some(r) => {
                println!("room: {}. player: {} submitted the card: {}", r, &user_id, msg.card_id);
                }
                None => {
                    println!("NO ROOM FOUND FOR PLAYER: {} tries to submit the card: {}", &user_id, msg.card_id);
                }
            }
        }
    }
}

/// Handler for Disconnect message.
impl Handler<messages::incomming::Disconnect> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::Disconnect, _: &mut Context<Self>) {
        println!("Someone disconnected");

        let _ = self.socket_actors.remove(&msg.token);
        
        if let Some(user_id) = self.get_user_id(&msg.token) {
            // remove address
            if self.sessions.get_mut().unwrap().remove(&msg.token).is_some() {
                // remove session from all rooms
                for (_name, room) in &mut self.matches {
                    room.players.retain(|elem| elem.id != user_id);
                }
            }
        }
        // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", Uuid::nil());
        // }
    }
}

/// Handler for Message message.
impl Handler<messages::incomming::ClientMessage> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::ClientMessage, _: &mut Context<Self>) {
        // self.send_message(&msg.room, msg.msg.as_str(), msg.id);
    }
}

/// Handler for `ListRooms` message.
impl Handler<messages::incomming::ListRooms> for CahServer {
    type Result = MessageResult<messages::incomming::ListRooms>;

    fn handle(&mut self, _: messages::incomming::ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::<String>::new();

        for key in self.matches.keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<messages::incomming::Join> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::Join, _: &mut Context<Self>) {
        let messages::incomming::Join { id, name } = msg;
        // let mut rooms = Vec::new();

        // remove session from all rooms
        // for (n, sessions) in &mut self.rooms {
        //     if sessions.remove(&id) {
        //         rooms.push(n.to_owned());
        //     }
        // }
        // // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", Uuid::nil());
        // }

        // if self.rooms.get_mut(&name).is_none() {
        //     self.rooms.insert(name.clone(), HashSet::new());
        // }
        // self.send_message(&name, "Someone connected", id);
        // self.rooms.get_mut(&name).unwrap().insert(id);
    }
}