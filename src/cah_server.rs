//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;
use json::JsonValue;
use std::sync::RwLock;
use std::sync::Arc;
use serde_json::json;
use std::collections::hash_map::Entry;
use num::PrimInt;
use std::time::Duration;
use rusqlite::NO_PARAMS;
use rusqlite::params;

use std::u64;
use crate::CookieToken;
use crate::messages;
use crate::db;

use rand::distributions::WeightedIndex;
use rand::distributions::Distribution;
use rand::thread_rng;
use rand::Rng;

use sha2::Sha512;
use sha2::Digest;
use generic_array::GenericArray;

use maplit::hashmap;
use str_macro::str;

use rand::seq::SliceRandom;

use r2d2_sqlite;
use r2d2_sqlite::SqliteConnectionManager;
use crate::db::{Pool, Database};


pub type CardId = i64;
pub type PlayerId = i64;
const PlayerNilId: PlayerId = 0;

type ShaImpl = Sha512;
//TODO: Infer this from ShaImpl::OutputSize instead of hardcoding
pub const PASSWORD_HASH_BYTE_SIZE: usize = 64;
pub type PasswordHash = GenericArray<u8, <ShaImpl as Digest>::OutputSize>;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
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
    pub deck_name: String,
    pub black_cards: Vec<Card>,
    pub white_cards: Vec<Card>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub id: PlayerId,
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
    cards: Vec<Card>,
    points: u32,
    submitted_card: Option<Card>,
    socket_actor: Option<Addr<crate::MyWebSocket>>,
}

// Increment by one (unchecked) and then wrap it to `wrap_to` if the new value is equal to `wrap_from`
//
// @arg value is the value being wrapped if it's to big
// @arg wrap_from if the `value`  reaches this number (inclusive) then the returned number will be equal to `wrap_to`
// @arg wrap_to the value being wrapped to once `value` reaches `wrap_from`
fn increment_and_wrap<T: PrimInt>(value: T, wrap_from: T, wrap_to: T) -> T {
    assert!(wrap_from > wrap_to);

    let new_val = value + T::one();
    if new_val >= wrap_from {
        wrap_to
    } else {
        new_val
    }
}

pub struct Match {
    players: Vec<PlayerInMatch>,
    match_progress: MatchInProgress,
    active_decks: Vec<String>,
    czar: PlayerId,
    points_to_win: u32,
}
impl Default for Match{
    fn default() -> Self {
        Match {
            players: Default::default(),
            match_progress: Default::default(),
            active_decks: Default::default(),
            czar: PlayerNilId,
            points_to_win: 7,
        }
    }
}
impl Match {
    fn remove_player(&mut self, user_id: &PlayerId) -> Option<PlayerInMatch>{
        let player_pos_option = self.players.iter().position(move |player| player.player.id == *user_id);
        match player_pos_option {
            Some(player_pos) => {
                if self.players.len() > 1 {
                    let next_czar_pos = increment_and_wrap(player_pos+1, self.players.len(), 0);
                    self.czar = self.players.get(next_czar_pos).expect("My increment_and_wrap function is giving me our of range player indices!").player.id;
                } else {
                    self.czar = PlayerNilId;
                }
                let player = self.players.remove(player_pos);

                Some(player)
            },
            None => { None }
        }
    }

    fn has_everyone_submitted_card(&self) -> bool {
        for player in &self.players {
            if player.player.id != self.czar && player.submitted_card.is_none() {
                return false;
            }
        }

        true
    }

    fn send_to_all_players(&mut self, msg: messages::outgoing::Message) {
        for player in &self.players {
            match &player.socket_actor{
                Some(socket_actor) => {
                    socket_actor.do_send(msg.clone());
                },
                None => {},
            }
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
    hand_of_cards: Vec<Card>,
    czar: PlayerId,
    started: bool,
}

pub struct WithCounter<T: Clone> {
    counter: u32,
    pub value: T,
}
impl<T: Clone> WithCounter<T> {
    fn new(value: T) -> Self {
        Self{counter: 1, value: value}
    }

    fn increment_counter(&mut self) {
        self.counter += 1;
    }

    // Returns if the value is still valid
    fn decrement_counter(&mut self) -> bool {
        match self.counter.checked_sub(1) {
            Some(new_val) => self.counter = new_val,
            None => {
                debug_assert!(false, "counter value was already 0 before it was decremented again");
                return false; 
            },
        }

        self.counter > 0
    }

    fn ref_count(&self) -> u32 {
        self.counter
    }
}

#[derive(Default, Clone, Debug)]
pub struct DeckCardIds {
    pub black_cards: Vec<CardId>,
    pub white_cards: Vec<CardId>
}

// An in memory cache of all decks in use across all matches
#[derive(Default)]
pub struct CardDeckCache{
    // All the cards in use at the moment
    cards: HashMap<CardId, String>,
    // Decks name to card id vector
    decks: HashMap<String, WithCounter<DeckCardIds> >,
}
impl CardDeckCache {
    pub fn get_card(&self, card_id: CardId) -> Option<Card> {
        match self.cards.get(&card_id) {
            Some(card_content) => Some(Card{content: card_content.to_string(), id: card_id}),
            None => None,
        }
    }

    pub fn add_deck(&mut self, deck: &CardDeck) {
        let deck_entry = self.decks.entry(deck.deck_name.clone());
        match deck_entry {
            Entry::Occupied(mut occupied_entry) => { occupied_entry.get_mut().increment_counter(); },
            Entry::Vacant(vacant_entry) => {
                let mut card_ids = DeckCardIds{black_cards: Vec::with_capacity(deck.black_cards.len()), white_cards: Vec::with_capacity(deck.white_cards.len())};
                for card in &deck.black_cards {
                    card_ids.black_cards.push(card.id.clone());
                    let old_val_opt = self.cards.insert(card.id.clone(), card.content.clone());
                    debug_assert!(old_val_opt.is_none(), 
                        "We should never override a pair here because the card_id should be unique. And we ref count our loaded decks.");
                }
                for card in &deck.white_cards {
                    card_ids.white_cards.push(card.id.clone());
                    let old_val_opt = self.cards.insert(card.id.clone(), card.content.clone());
                    debug_assert!(old_val_opt.is_none(), 
                        "We should never override a pair here because the card_id should be unique. And we ref count our loaded decks.");
                }
                vacant_entry.insert(WithCounter::new(card_ids)); 
            },
        }

    }

    pub fn remove_deck(&mut self, deck: &CardDeck) {
        let deck_entry = self.decks.entry(deck.deck_name.clone());
        match deck_entry {
            Entry::Occupied(mut occupied_entry) => {
                let should_be_removed = occupied_entry.get_mut().decrement_counter();
                if should_be_removed {
                    let (_deck_name, card_ids) = occupied_entry.remove_entry();
                    for card_id in card_ids.value.black_cards.iter().chain(card_ids.value.white_cards.iter()) {
                        let card_content = self.cards.remove(&card_id);
                        debug_assert!(card_content.is_some(), 
                            "Somehow our decks vector is refering to cards that don't exist in the cache");
                    }
                }
            },
            Entry::Vacant(_vacant_entry) => {
                debug_assert!(false, 
                    "We are trying to remove a deck that never got loaded?");
                println!("We are trying to remove a deck that never got loaded?");
            },
        }
    }

    pub fn get_random_black_card(&self, active_decks: &Vec<String>) -> Option<Card> {
        let mut weights = Vec::new();
        for deck_name in active_decks {
            if let Some(card_deck_rc) = self.decks.get(deck_name) {
                let card_deck = &card_deck_rc.value;
                weights.push(card_deck.black_cards.len());
            }
        }

        let mut rng = thread_rng();
        let distribution_result = WeightedIndex::new(weights.as_slice());
        let deck_index = match distribution_result {
            Ok(distribution) => {
                distribution.sample(&mut rng)
            },
            Err(weighted_err) => { 
                debug_assert!(false); 
                println!("ERROR: Could not add a card to the hand!! {}", weighted_err);
                return None;
            }
        };

        let ref picked_deck_name = active_decks[deck_index];
        let deck_ids_opt = &self.decks.get(picked_deck_name);
        if let Some(ref deck_ids) = deck_ids_opt {
            let local_card_index = rng.gen_range(0, deck_ids.value.black_cards.len());
            let local_card_id = deck_ids.value.black_cards[local_card_index];
            let card_content_opt = self.cards.get(&local_card_id);
            if let Some(card_content) = card_content_opt {
                return Some(Card{content: card_content.clone(), id: local_card_id.clone()});
            }
        }
        
        None
    }

    pub fn get_random_white_card(&self, active_decks: &Vec<String>) -> Option<Card> {
        let mut weights = Vec::new();
        for deck_name in active_decks {
            if let Some(card_deck_rc) = self.decks.get(deck_name) {
                let card_deck = &card_deck_rc.value;
                weights.push(card_deck.white_cards.len());
            }
        }

        let mut rng = thread_rng();
        let distribution_result = WeightedIndex::new(weights.as_slice());
        let deck_index = match distribution_result {
            Ok(distribution) => {
                distribution.sample(&mut rng)
            },
            Err(weighted_err) => { 
                debug_assert!(false); 
                println!("ERROR: Could not add a card to the hand!! {}", weighted_err);
                return None;
            }
        };

        let ref picked_deck_name = active_decks[deck_index];
        let deck_ids_opt = &self.decks.get(picked_deck_name);
        if let Some(ref deck_ids) = deck_ids_opt {
            let local_card_index = rng.gen_range(0, deck_ids.value.white_cards.len());
            let local_card_id = deck_ids.value.white_cards[local_card_index];
            let card_content_opt = self.cards.get(&local_card_id);
            if let Some(card_content) = card_content_opt {
                return Some(Card{content: card_content.clone(), id: local_card_id.clone()});
            }
        }
        
        None
    }
}

/// `CahServer`(Cards against humanity server) manages matches and responsible for coordinating
/// session. 
pub struct CahServer {
    //socket_actors: HashMap<CookieToken, Addr<crate::MyWebSocket>>,
    // The cookie token to player PlayerId, the PlayerId is the reprisentation internally.
    sessions: RwLock<HashMap<CookieToken, PlayerId>>, //TODO: Clear sessins after a couple hours/days, so the ram doesn't quitely go downwards.
    matches: RwLock<HashMap<String, Match>>,
    database: RwLock<Database>,
    card_cache: RwLock<CardDeckCache>,
}

impl CahServer {
    pub fn new(connection_pool: Pool) -> Self {
        // default room
        let mut matches = HashMap::new();
        let mut main_match = Match::default();
        main_match.active_decks.push(str!("Default"));
        let mut second_room_match = Match::default();
        second_room_match.active_decks.push(str!("Default"));
        matches.insert("Main".to_owned(), main_match);
        matches.insert("Second Room".to_owned(), second_room_match);

        let mut db: Database = Database::new(connection_pool);
        let mut card_cache: CardDeckCache = Default::default();

        let default_card_deck = db.execute(db::GetCardDeck{deck_name: str!("Default")}).wait().unwrap();
        card_cache.add_deck(&default_card_deck);

        CahServer {
            sessions: Default::default(),
            matches: RwLock::new(matches),
            database: RwLock::new(db),
            card_cache: RwLock::new(card_cache),
        }
    } 

    //TODO: Optimize
    fn get_room_from_uuid(&self, user_id: &PlayerId) -> Option<String> {
        for room in self.matches.read().unwrap().iter() {
            for player in room.1.players.iter() {
                if &player.player.id == user_id {
                    return Some(room.0.clone());
                }
            }
        }

        None
    }

    fn get_user_id(&self, cookie_token: &CookieToken) -> Option<PlayerId> {
        match self.sessions.read().unwrap().get(cookie_token) {
            Some(uuid) => Some(uuid.clone()),
            None => None,
        }
    }

    fn add_random_card(card_cache: &CardDeckCache, active_decks: &Vec<String>, player: &mut PlayerInMatch) {
        if let Some(card) = card_cache.get_random_white_card(active_decks) {

            player.cards.push(card.clone());

            if let Some(socket_actor) = player.socket_actor.clone() {
                let json_msg = json!({
                    "type": "addCardToHand",
                    "card_id": card.id,
                    "card_content": card.content.clone(),
                });

                socket_actor.do_send(messages::outgoing::Message(json_msg.to_string()));
            }
        }
    }
}

/// Make actor from `CahServer`
impl Actor for CahServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        
    }
}

fn hash_password(salt: &Uuid, password: &str) -> PasswordHash {
    let mut sha =  ShaImpl::new();

    sha.input(salt.to_simple_ref().to_string());
    sha.input(password);

    sha.result()
}

impl Handler<messages::incomming::RegisterAccount> for CahServer {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: messages::incomming::RegisterAccount, _ctx: &mut Context<Self>) -> Self::Result {

        // if let Some(db_player) = self.database.read().unwrap().players.iter().find(|&db_player| db_player.email == msg.email || db_player.player.name == msg.username) {
        //     return if db_player.email == msg.email {
        //         Err("User already exists with email".to_owned())
        //     } else { // we only check for email and username, so username should be equal here.
        //         Err("User already exists with username".to_owned())
        //     };
        // }

        let salt = Uuid::new_v4();
        let password_hash = hash_password(&salt, &msg.password);

        let db_cmd = db::RegisterPlayer{username: msg.username, email: msg.email, password_hash: password_hash, salt: salt};
        let database = self.database.get_mut().unwrap();
        let db_future = database.execute(db_cmd);

        //TODO: IDGAF fuck this give me the value
        db_future.wait().map_err(|db_err| format!("Db error registering new account: {:?}", db_err))
        
        //let new_db_player = DatabasePlayer{player: Player{name: msg.username, id: Uuid::new_v4()}, email: msg.email, password_hash: password_hash, salt: salt};

        //self.database.get_mut().unwrap().players.push(new_db_player);

        // Ok(())
    }
}

impl Handler<messages::incomming::Login> for CahServer {
    type Result = Result<CookieToken, String>;

    //TODO: How to handle two people fighting over a account?
    fn handle(&mut self, msg: messages::incomming::Login, _ctx: &mut Context<Self>) -> Self::Result {
        let database = self.database.get_mut().unwrap();
        let db_cmd = db::LoginPlayer{username_or_email: msg.username_or_email};
        let db_future = database.execute(db_cmd);
        //TODO: Not wait or something idc
        match db_future.wait() {
            Ok((player_id, db_password_hash, salt)) => {
                let user_password_hash = hash_password(&salt, &msg.password);
                if user_password_hash == db_password_hash {
                    let sessions = self.sessions.get_mut().unwrap();
                    //TODO: Optimize this, there can only be one.
                    sessions.retain(|&_key, &mut value| value != player_id);
                    let new_cookie_token = CookieToken::new_v4();
                    sessions.insert(new_cookie_token, player_id);

                    Ok(new_cookie_token)
                } else {
                    Err(str!("Password incorrect!"))
                }
            },
            Err(db_err) => Err(format!("Error retrieving players from db query, db_err: {:?}", db_err)),
        }

        // } else {
        //     Err(str!("Cannot find player by name of email"))
        // }

        // let db = self.database.get_mut().unwrap();
        // if let Some(db_player) = db.players.iter().find(|&db_player| db_player.player.name == msg.username_or_email || db_player.email == msg.username_or_email) {
        //     let password_hash = hash_password(&db_player.salt, &msg.password);
        //     if password_hash == db_player.password_hash {
        //         let sessions = self.sessions.get_mut().unwrap();
        //         let player_id = db_player.player.id.clone();
        //         //TODO: Optimize this, there can only be one.
        //         sessions.retain(|&_key, &mut value| value != player_id);
        //         let new_cookie_token = CookieToken::new_v4();
        //         sessions.insert(new_cookie_token, player_id);

        //         Ok(new_cookie_token)
        //     } else {
        //         Err("Wrong password!".to_owned())
        //     }
        // } else {
        //     Err("Username/Email not found".to_owned())
        // }
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

        let databse = self.database.get_mut().unwrap();
        let query_player = db::GetPlayerById{player_id: user_id.clone()};
        let query_player_fut = databse.execute(query_player);

        let player_result = query_player_fut.wait();
        debug_assert!(player_result.is_ok(), 
            format!("ERROR: Cannot find player with id:{} in the database, pleaes make an account first! Thanks you!", user_id));

        let player = player_result.unwrap();

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
                let db = self.database.get_mut().unwrap();
                let player_option = db.execute(db::GetPlayerById{player_id: user_id}).wait();
                debug_assert!(player_option.is_ok(), 
                    "We managed to find ourselves with the call `CahServer::get_user_id()` but we cannot find ourselves in `self.get_player_by_id()`");
                let player = player_option.unwrap();
                let player_in_match = PlayerInMatch{player: player.clone(), cards: Vec::new(), points: 0, submitted_card: None, socket_actor: None };
                if !already_in_match {
                    room.players.push(player_in_match.clone());

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
                }

                //handle czar memes
                if room.czar == PlayerNilId {
                    if !room.players.is_empty() {
                        room.czar = room.players[0].player.id;
                    }
                }
                
                let game_state = GameState{
                    other_players: room.players.iter().map(|elem| elem.player.clone()).collect(), 
                    our_player: player.clone(), 
                    hand_of_cards: player_in_match.cards.clone(),
                    czar: room.czar,
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

    fn handle(&mut self, msg: messages::incomming::StartMatch, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            if let Some(room) = self.matches.get_mut().unwrap().get_mut(&msg.match_name) {
                if room.players.len() >= 3 {
                    if let Some(player_in_match) = room.players.iter().find(|elem| elem.player.id == user_id) {
                        // We found ourselves so there should be at least 1 entry
                        // Check if we are the czar:
                        if room.players[0].player.id == player_in_match.player.id && room.match_progress == MatchInProgress::NotStarted {
                            room.match_progress = MatchInProgress::InProgress;
                            
                            let db = self.database.get_mut().unwrap();
                            let default_card_deck = db.execute(db::GetCardDeck{deck_name: str!("Default")}).wait().unwrap();//db.card_decks.get("Default").unwrap();
                            let msg_json = json!({
                                "type": "matchStarted",
                            });
                            for every_player in &mut room.players {
                                match &every_player.socket_actor{
                                    Some(socket_actor) => {
                                        socket_actor.do_send(messages::outgoing::Message( msg_json.to_string() ));

                                        let random_cards: Vec<_> = default_card_deck.white_cards.choose_multiple(&mut rand::thread_rng(), 3).collect();
                                        for card in random_cards{
                                            every_player.cards.push(card.clone());
                                            ctx.address().do_send(messages::outgoing::AddCardToHand{room: msg.match_name.clone(), player: every_player.player.clone(), card: card.clone()});
                                        }
                                    },
                                    None => {}
                                }
                            }

                            if let Some(card) = self.card_cache.read().unwrap().get_random_black_card(&room.active_decks) {
                                let json_msg = json!({
                                    "type": "newBlack",
                                    "card_id": card.id,
                                    "card_content": card.content
                                });
                                room.send_to_all_players(messages::outgoing::Message(json_msg.to_string()))
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
                    let card_opt = self.card_cache.read().unwrap().get_card(msg.card_id);
                    if card_opt.is_none() {
                        println!("Submit request is invalid! The card doesn't exist in the card cache!");
                        debug_assert!(false);
                        return;
                    }
                    let card = card_opt.unwrap();
                    println!("room: {}. player: {} submitted the card(id: {:?}, txt: {})", room_name, &user_id, msg.card_id, &card.content);
                    let room = self.matches.get_mut().unwrap().get_mut(&room_name).unwrap();
                    if let Some(pid_player) = room.players.iter_mut().find(|elem| elem.player.id == user_id) {
                        let _ = pid_player.submitted_card.get_or_insert(card);
                    } else {
                        debug_assert!(false, "We managed to find ourselves with `self.get_user_id()`, however not while finding ourselves");
                    }
                    let everyone_submitted = room.has_everyone_submitted_card();
                    if everyone_submitted {
                        let mut card_ids: Vec<CardId> = Vec::with_capacity(room.players.len()-1);
                        for player in &room.players {
                            if room.czar != player.player.id {
                                //TODO: WTF Why do I need to clone the Option here? I just want to read a value from it???
                                let submitted_card_opt = player.submitted_card.clone();
                                card_ids.push(submitted_card_opt.expect("We already checked with `Match::has_everyone_submitted_card()`").id.clone());
                            }
                        }
                        let everyone_submitted_json = json!({
                            "type": "everyone_submitted",
                            "card_ids": card_ids
                        });
                        let everyone_submitted_msg = messages::outgoing::Message(everyone_submitted_json.to_string());
                        room.send_to_all_players(everyone_submitted_msg);
                    }
                }
                None => {
                    println!("NO ROOM FOUND FOR PLAYER: {} ! While trying to submit the card_id: {}", &user_id, msg.card_id);
                }
            }
        }
    }
}

impl Handler<messages::incomming::GetCards> for CahServer {
    type Result = Result<CardDeck, String>;

    fn handle(&mut self, msg: messages::incomming::GetCards, _: &mut Context<Self>) -> Self::Result {
        if let Some(_user_id) = self.sessions.read().unwrap().get(&msg.token) {
            let database = self.database.get_mut().unwrap();
            database.execute(db::GetCardDeck{deck_name: msg.deck_name}).wait().map_err(|db_err| format!("{}", db_err))
        } else {
            Err(str!("Cannot find logged in player with that session token, is it invalid?"))
        }
    }
}

impl Handler<messages::incomming::AddCard> for CahServer {
    type Result = Result<CardId, String>;

    fn handle(&mut self, msg: messages::incomming::AddCard, _: &mut Context<Self>) -> Self::Result {
        if let Some(_user_id) = self.sessions.read().unwrap().get(&msg.token) {
            let database = self.database.get_mut().unwrap();

            database.execute(db::AddCard{deck_name: msg.deck_name, card_content: msg.card_content, is_black: msg.is_black}).wait()
                .map_err(|db_err| format!("Db error: {}", db_err))
        } else {
            Err(str!("Cannot find logged in player with that session token, is it invalid?"))
        }
    }
}


impl Handler<messages::incomming::DelCard> for CahServer {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: messages::incomming::DelCard, _: &mut Context<Self>) -> Self::Result {
        if let Some(_user_id) = self.sessions.read().unwrap().get(&msg.token) {
            let database = self.database.get_mut().unwrap();

            let _void = database.execute(db::DelCard{deck_name: msg.deck_name, card_id: msg.card_id}).wait().map_err(|db_err| format!("Db Err: {}", db_err))?;

            Ok(())
        } else {
            Err(str!("Cannot find logged in player with that session token, is it invalid?"))
        }
    }
}

impl Handler<messages::incomming::RevealCard> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::RevealCard, _: &mut Context<Self>) -> Self::Result {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            let matches = self.matches.get_mut().unwrap();
            match matches.get_mut(&msg.match_name) {
                Some(room) => {                    
                    if user_id == room.czar && room.has_everyone_submitted_card() {
                        let card_opt = self.card_cache.read().unwrap().get_card(msg.card_id);
                        if card_opt.is_none() {
                            println!("Submit request is invalid! The card doesn't exist in the card cache!");
                            debug_assert!(false);
                            return;
                        }
                        let card = card_opt.unwrap();
                        println!("room: {}. czar player: {} revealed the card: {:?}", &msg.match_name, &user_id, &card.id);

                        let reveal_card_json = json!({
                            "type": "revealCard",
                            "card_id": card.id,
                            "card_content": card.content,
                        });
                        let reveal_card_msg = messages::outgoing::Message(reveal_card_json.to_string());
                        room.send_to_all_players(reveal_card_msg);
                    } else {
                        if user_id != room.czar {
                            println!("The user trying to submit a reveal card is not the czar!");
                        } else {
                            println!("In this room not everyone has submitted their card yet! (Meaning they are not ready to reveal yet).");
                        }
                    }
                }
                None => {
                    println!("NO ROOM FOUND FOR PLAYER: {} ! While trying to reveal the card_id: {}", &user_id, msg.card_id);
                }
            }
        }
    }
}

impl Handler<crate::StopServer> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: crate::StopServer, ctx: &mut Context<Self>) {
        System::current().stop_with_code(0);
    }
}

impl Handler<messages::incomming::CzarChoice> for CahServer {
    type Result = ();

    fn handle(&mut self, msg: messages::incomming::CzarChoice, ctx: &mut Context<Self>) {
        if let Some(user_id) = self.get_user_id(&msg.token) {
            let matches = self.matches.get_mut().unwrap();
            match matches.get_mut(&msg.match_name) {
                Some(room) => {                    
                    if user_id == room.czar && room.has_everyone_submitted_card() {
                        let card_opt = self.card_cache.read().unwrap().get_card(msg.card_id);
                        if card_opt.is_none() {
                            println!("Submit request is invalid! The card doesn't exist in the card cache!");
                            debug_assert!(false);
                            return;
                        }
                        let card = card_opt.unwrap();
                        println!("room: {}. czar player: {} choose the card: {:?}", &msg.match_name, &user_id, &card.id);

                        let czar_submitted_json = json!({
                            "type": "czar_choice",
                            "card_id": card.id,
                        });
                        let czar_submitted_msg = messages::outgoing::Message(czar_submitted_json.to_string());
                        room.send_to_all_players(czar_submitted_msg);

                        let victorious_player_opt = room.players.iter_mut().find(|player_in_match| match &player_in_match.submitted_card {
                            Some(submitted_card) => submitted_card.id == card.id,
                            None => false,
                        });
                        
                        debug_assert!(victorious_player_opt.is_some());
                        if let Some(victorious_player) = victorious_player_opt {
                            victorious_player.points += 1;
                            let victorious_player_id = victorious_player.player.id;
                            let did_player_win = victorious_player.points >= room.points_to_win;
                            {
                                let player_won_round_json = json!({
                                    "type": "roundWon",
                                    "player_id": victorious_player_id,
                                });
                                room.send_to_all_players(messages::outgoing::Message(player_won_round_json.to_string()));
                            }


                            if did_player_win {
                                let player_won_msg = json!({
                                    "type": "playerWon",
                                    "player_id": victorious_player_id,
                                });
                                room.send_to_all_players(messages::outgoing::Message(player_won_msg.to_string()));
                            }

                            ctx.run_later(Duration::from_millis(3000), move |cah, _ctx| {
                                let matches = cah.matches.get_mut().unwrap();
                                let card_cache = cah.card_cache.read().unwrap();
                                match matches.get_mut(&msg.match_name) {
                                    Some(room) => { 
                                        for player_in_match in &mut room.players {
                                            if player_in_match.player.id != room.czar {
                                                if let Some(submitted_card) = player_in_match.submitted_card.clone() {
                                                    if let Some(card_pos) = player_in_match.cards.iter().position(|card| card.id == submitted_card.id) {
                                                        if let Some(socket_connection) = player_in_match.socket_actor.clone() {
                                                            let msg_json = json!({
                                                                "type": "removeCard",
                                                                "card_id": player_in_match.cards[card_pos].id
                                                            });
                                                            let msg_json_string = msg_json.to_string();
                                                            socket_connection.do_send( messages::outgoing::Message(msg_json_string) );
                                                        }

                                                        player_in_match.cards.remove(card_pos);
                                                        CahServer::add_random_card(&card_cache, &room.active_decks, player_in_match);
                                                    }
                                                }
                                            }

                                            player_in_match.submitted_card = None;
                                        }

                                        let new_round_json = json!({
                                            "type": "newRound"
                                        });
                                        room.send_to_all_players(messages::outgoing::Message(new_round_json.to_string()));

                                        {
                                            if let Some(card) = card_cache.get_random_black_card(&room.active_decks) {
                                                let new_black_card_json = json!({
                                                    "type": "newBlack",
                                                    "card_id": card.id,
                                                    "card_content": card.content
                                                });
                                                room.send_to_all_players( messages::outgoing::Message(new_black_card_json.to_string()) );
                                            }
                                        }

                                        let czar_index_opt = room.players.iter().position(|pim| &pim.player.id == &room.czar);
                                        match czar_index_opt {
                                            Some(czar_index) => {
                                                if !room.players.is_empty() {
                                                    let new_czar_index = increment_and_wrap(czar_index, room.players.len(), 0);
                                                    room.czar = room.players[new_czar_index].player.id;
                                                }
                                            },
                                            None => { 
                                                // Right now, if it can't find the last czar, it will default to p1, however thats unfair for the last player.
                                                // However, this should "never" happen as when the player disconnects it already gets handled and czar gets handed over.
                                                // Which makes this soludion here solid.
                                                if !room.players.is_empty() { room.czar = room.players[0].player.id; } 
                                            }
                                        }

                                        let new_czar_json = json!({
                                            "type": "newCzar",
                                            "czar": room.czar
                                        });
                                        room.send_to_all_players(messages::outgoing::Message(new_czar_json.to_string()));
                                    },
                                    None => {},
                                }              
                            });
                        }
                    }
                }
                None => {
                    println!("NO ROOM FOUND FOR PLAYER: {} ! While trying to submit the card_id: {}", &user_id, msg.card_id);
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
