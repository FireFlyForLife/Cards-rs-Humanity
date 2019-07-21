//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;
use json::JsonValue;
use std::sync::RwLock;
use serde_json::json;

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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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
impl Database {
    fn get_player_by_id(&self, id: &Uuid) -> Option<Player> {
        for db_player in &self.players {
            if db_player.player.id == *id {
                return Some(db_player.player.clone());
            }
        }

        None
    }
}

#[derive(PartialEq, Eq)]
pub enum MatchInProgress {
    NotStarted,
    InProgress,
}
impl Default for MatchInProgress {
    fn default() -> Self{
        MatchInProgress::NotStarted
    }
}

#[derive(Default, Clone)]
pub struct PlayerInMatch {
    player: Player,
    cards: Vec<String>,
    submitted_card: Option<String>,
    socket_actor: Option<Addr<crate::MyWebSocket>>,
}

#[derive(Default)]
pub struct Match {
    players: Vec<PlayerInMatch>,
    match_progress: MatchInProgress,
}
impl Match {
    fn remove_player(&mut self, user_id: &Uuid) -> Option<PlayerInMatch>{
        let player_pos_option = self.players.iter().position(move |player| player.player.id == *user_id);
        match player_pos_option {
            Some(player_pos) => {
                let player = self.players.remove(player_pos);

                Some(player)
            },
            None => { None}
        }
    }
}


// /// Message for chat server communications
// pub mod messages {
//     use actix::prelude::*;
//     use uuid::Uuid;
//     use crate::cah_server::{CardId, Card, Player};
//     use crate::CookieToken;

    
// }


/// struct used for sending over network, for syncing new clients
#[derive(Serialize, Deserialize)]
pub struct GameState {
    other_players: Vec<Player>,
    our_player: Player,
    hand_of_cards: Vec<String>,
    czar: Uuid,
    started: bool,
}

/// `CahServer`(Cards against humanity server) manages matches and responsible for coordinating
/// session. 
pub struct CahServer {
    //socket_actors: HashMap<CookieToken, Addr<crate::MyWebSocket>>,
    // The cookie token to player Uuid, the Uuid is the reprisentation internally.
    sessions: RwLock<HashMap<CookieToken, Uuid>>, //TODO: Clear sessins after a couple hours/days, so the ram doesn't quitely go downwards.
    matches: RwLock<HashMap<String, Match>>,
    database: RwLock<Database>
}

impl Default for CahServer {
    fn default() -> CahServer {
        // default room
        let mut matches = HashMap::new();
        matches.insert("Main".to_owned(), Match::default());
        matches.insert("Second Room".to_owned(), Match::default());

        CahServer {
            sessions: Default::default(),
            matches: RwLock::new(matches),
            database: Default::default(),
        }
    }
}

impl CahServer {
    //TODO: Optimize
    fn get_room_from_uuid(&self, user_id: &Uuid) -> Option<String> {
        for room in self.matches.read().unwrap().iter() {
            for player in room.1.players.iter() {
                if &player.player.id == user_id {
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

/// Make actor from `CahServer`
impl Actor for CahServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

fn hash_password(salt: &Uuid, password: &str) -> PasswordHash {
    let mut sha =  ShaImpl::new();

    sha.input(salt.clone().to_simple_ref().to_string());
    sha.input(password);

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
impl Handler<messages::incomming::SocketConnectMatch> for CahServer {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: messages::incomming::SocketConnectMatch, ctx: &mut Context<Self>) -> Self::Result {
        // register session with token
        //self.socket_actors.insert(msg.token.clone(), msg.addr);

        let user_id;
        if let Some(user_id_) = self.get_user_id(&msg.token) {
            user_id = user_id_;
        } else {
            println!("No user could be found with cookie token: {}.", &msg.token);
            return Err(format!("No user could be found with cookie token: {}", &msg.token));

            // user_id = Uuid::new_v4();
            // self.sessions.get_mut().unwrap().insert(msg.token.clone(), user_id.clone());
            // self.database.get_mut().unwrap().players.push(DatabasePlayer{player: Player{id: user_id.clone(), name: format!("Temp account name for {}", &user_id)}, email: "".to_owned(), password_hash: Default::default(), salt: Uuid::new_v4()});
        }

        println!("{} is connecting", &user_id);

        let db_player = self.database.get_mut().unwrap().players.iter().find(|&db_player| &db_player.player.id == &user_id);
        debug_assert!(db_player.is_some(), 
            format!("ERROR: Cannot find player with id:{} in the database, pleaes make an account first! Thanks you!", user_id));

        let player = db_player.unwrap().player.clone();

        if let Some(room_name) = self.get_room_from_uuid(&user_id) {
            //TODO: MyWebSocket stores the room it should be found to, and should be checked here too.

            if let Some(room) = self.matches.get_mut().unwrap().get_mut(&room_name) {
                let pim_opt = room.players.iter_mut().find(|elem| elem.player.id == user_id);
                match pim_opt {
                    Some(pim) => {
                        pim.socket_actor = Some(msg.addr);
                    },
                    None => {},
                }
            }

            //self.matches.get_mut().unwrap().get_mut(&"Main".to_owned()).unwrap().players.push(player.clone());

            // for i in 0..4 {
            //     //TODO: Not hardcode cards...
            //     let card = Card{content: format!("A card from the server! {}", i), id: i};
            //     ctx.address().do_send(messages::outgoing::AddCardToHand{room: room_name.clone(), player: player.clone(), card: card});
            // }

            Ok(())
        } else {
            Err("Could not find a match where this user is in, is JoinMatch not send beforehand?".to_owned())
        }

    }
}

impl Handler<messages::incomming::JoinMatch> for CahServer {
    type Result = Result<GameState, String>;

    fn handle(&mut self, msg: messages::incomming::JoinMatch, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            let mut already_in_match = false;

            //Firstly disconnect from an existing match if we are switching match.
            if let Some(room_name) = self.get_room_from_uuid(&user_id) {
                //ctx.address().do_send(messages::incomming::Leavematch{match_name: room, token: msg.token});
                already_in_match = room_name == msg.match_name;
                if !already_in_match {
                    let _ = self.handle(messages::incomming::Leavematch{match_name: room_name, token: msg.token}, ctx);
                }
            }
            
            let matches = self.matches.get_mut().unwrap();
            if let Some(room) = matches.get_mut(&msg.match_name) {
                let db = self.database.read().unwrap();
                let player_option = db.get_player_by_id(&user_id);
                debug_assert!(player_option.is_some(), 
                    "We managed to find ourselves with the call `CahServer::get_user_id()` but we cannot find ourselves in `self.get_player_by_id()`");
                let player = player_option.unwrap();
                let player_in_match = PlayerInMatch{player: player.clone(), cards: Vec::new(), submitted_card: None, socket_actor: None };
                if !already_in_match {
                    for other_player_in_match in  &room.players{
                        let join_json = json!({
                            "type": "player_joined",
                            "player": player.clone(),
                        });

                        match &other_player_in_match.socket_actor {
                            Some(socket_actor) => socket_actor.do_send(messages::outgoing::Message(join_json.to_string())),
                            None => {}
                        }
                    }
                    
                    room.players.push(player_in_match.clone());
                }
                
                let game_state = GameState{
                    other_players: room.players.iter().map(|elem| elem.player.clone()).collect(), 
                    our_player: player.clone(), 
                    hand_of_cards: player_in_match.cards.clone(),
                    czar: room.players[0].player.id.clone(),
                    started: room.match_progress == MatchInProgress::InProgress};

                Ok(game_state)
            } else {
                Err(format!("Cannot find the room named '{}'. Has it been removed in the meantime?", msg.match_name))
            }
        } else {
            Err("No user with that cookie token could be found, maybe the session expired?".to_owned())
        }
    }
}

/// Handler for Disconnect message.
impl Handler<messages::incomming::Leavematch> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::Leavematch, _: &mut Context<Self>) {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            match self.matches.get_mut().unwrap().get_mut(&msg.match_name) {
                Some(room) => {
                    let removed_player_opt = room.remove_player(&user_id);
                    match removed_player_opt {
                        Some(removed_player) => {
                            for player in room.players.iter() {
                                match &player.socket_actor {
                                    Some(socket) => {
                                        let leave_json = json!({
                                            "type": "player_left",
                                            "player_id": removed_player.player.id,
                                        });
                                        
                                        socket.do_send(messages::outgoing::Message(leave_json.to_string()));
                                    },
                                    None => {
                                        println!("Coudln't send the thing leave message!");
                                    },
                                }
                            }
                        },
                        None => {},
                    }
                },
                None => {},
            }
        }
    }
}

impl Handler<messages::incomming::StartMatch> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::StartMatch, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            if let Some(room) = self.matches.get_mut().unwrap().get_mut(&msg.match_name) {
                if let Some(player_in_match) = room.players.iter().find(|elem| elem.player.id == user_id) {
                    // We found ourselves so there should be at least 1 entry
                    // Check if we are the czar:
                    if room.players[0].player.id == player_in_match.player.id && room.match_progress == MatchInProgress::NotStarted {
                        room.match_progress = MatchInProgress::InProgress;
                        
                        let msg_json = json!({
                            "type": "matchStarted",
                        });
                        for every_player in &room.players {
                            match &every_player.socket_actor{
                                Some(socket_actor) => socket_actor.do_send(messages::outgoing::Message( msg_json.to_string() )),
                                None => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Handler<messages::outgoing::AddCardToHand> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::outgoing::AddCardToHand, _ctx: &mut Context<Self>) -> Self::Result {
        let mut message = json::parse(r#"{"type": "addCardToHand"}"#).unwrap();
        message["card_content"] = JsonValue::String(msg.card.content);
        message["card_id"] = JsonValue::Number(json::number::Number::from(msg.card.id));
        let message_json = json::stringify(message);

        if let Some(room) = self.matches.read().unwrap().get(&msg.room) {
            let user_id = msg.player.id;
            if let Some(pim) = room.players.iter().find(|elem| elem.player.id == user_id){
                if let Some(socket_actor) = &pim.socket_actor {
                    socket_actor.do_send(messages::outgoing::Message(message_json));
                }
            }
        }
    }
}

/// Handler for SubmitCard message
impl Handler<messages::incomming::SubmitCard> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::SubmitCard, _: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            let room_option = self.get_room_from_uuid(&user_id);
            match room_option {
                Some(room_name) => {
                    println!("room: {}. player: {} submitted the card: {}", room_name, &user_id, msg.card_id);
                    let room = self.matches.get_mut().unwrap().get_mut(&room_name).unwrap();
                    if let Some(pid_player) = room.players.iter_mut().find(|elem| elem.player.id == user_id) {
                        let _ = pid_player.submitted_card.get_or_insert("wasd".to_owned());
                    } else {
                        debug_assert!(false, "We managed to find ourselves with `self.get_user_id()`, however not while finding ourselves");
                    }
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

        // let socket_actor_entrty = self.socket_actors.entry(msg.token.clone());
        // match socket_actor_entrty {
        //     Entry::Occupied(entry) => {
        //         let (_saved_token, _socket_actor) = entry.remove_entry();
        //         //actually we don't need to stop, if all Addr's go out of scope, the actor is stopped automatically
        //         //socket_actor.stop()
        //     },
        //     _ => {},
        // }
        // let _ = self.socket_actors.remove(&msg.token);
        
        if let Some(user_id) = self.get_user_id(&msg.token) {
            // remove address
            if self.sessions.get_mut().unwrap().remove(&msg.token).is_some() {
                // remove session from all rooms
                for (_name, room) in self.matches.get_mut().unwrap() {
                    room.players.retain(|elem| elem.player.id != user_id);
                }
            }
        }
        // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", Uuid::nil());
        // }
    }
}

/// Handler for `ListRooms` message.
impl Handler<messages::incomming::ListRooms> for CahServer {
    type Result = MessageResult<messages::incomming::ListRooms>;

    fn handle(&mut self, _: messages::incomming::ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::<String>::new();

        for key in self.matches.read().unwrap().keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}
