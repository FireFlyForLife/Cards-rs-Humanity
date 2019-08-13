use crate::cah_server::{Card, CardId, CardDeck, PlayerId, Player, GameState};
use crate::CookieToken;
use actix::prelude::*;
use std::string::String;
use crate::MyWebSocket;

// Containing all messages which will be commin in from a client to the server
pub mod incomming {
    use crate::messages::*;

    /// When a socket connection has been established and the socket wants to be bound to a match
    pub struct SocketConnectMatch {
        pub addr: Addr<MyWebSocket>,
        pub token: CookieToken,
    }
    impl actix::Message for SocketConnectMatch {
        type Result = Result<(), String>;
    }

    // A request from a client to join a match
    pub struct JoinMatch {
        pub match_name: String,
        pub token: CookieToken,
    }
    impl actix::Message for JoinMatch {
        type Result = Result<GameState, String>;
    }

    #[derive(Message)]
    pub struct Leavematch {
        pub match_name: String,
        pub token: CookieToken,
    }

    #[derive(Message)]
    pub struct RevealCard {
        pub token: CookieToken,
        pub match_name: String,
        pub card_id: CardId,
    }

    // #[derive(Message)]
    // #[rtype(result="Error<(), String>")]
    pub struct RegisterAccount {
        pub email: String,
        pub username: String,
        pub password: String,
    }
    impl actix::Message for RegisterAccount {
        type Result = Result<(), String>;
    }

    // #[derive(Message)]
    // #[rtype(result="Error<CookieToken, String>")]
    pub struct Login {
        pub username_or_email: String,
        pub password: String,
    }
    impl actix::Message for Login {
        type Result = Result<CookieToken, String>;
    }

    /// Disconnect from everything
    #[derive(Message)]
    pub struct Disconnect {
        pub token: CookieToken,
    }

    /// List of available rooms request, this doesn't need to be over a websocket actually.
    #[derive(Default)]
    pub struct ListRooms{
        pub cookie_token: CookieToken,
    }
    impl actix::Message for ListRooms {
        type Result = Vec<String>;
    }

    #[derive(Message)]
    pub struct SubmitCard {
        pub token: CookieToken,
        pub card_id: CardId,
        // pub card_content: String,
    }

    #[derive(Message)]
    pub struct StartMatch {
        pub token: CookieToken,
        pub match_name: String,
    }

    #[derive(Message)]
    pub struct CzarChoice {
        pub token: CookieToken,
        pub match_name: String,
        pub card_id: CardId,
    }

    pub struct GetCards {
        pub token: CookieToken,
        pub deck_name: String,
    }
    impl actix::Message for GetCards {
        type Result = Result<CardDeck, String>;
    }

    pub struct AddCard {
        pub token: CookieToken,
        pub deck_name: String,
        pub card_content: String,
        pub is_black: bool,
    }
    impl actix::Message for AddCard {
        type Result = Result<CardId, String>;
    }

    pub struct DelCard {
        pub token: CookieToken,
        pub deck_name: String,
        pub card_id: CardId
    }
    impl actix::Message for DelCard {
        type Result = Result<(), String>;
    }
}

// Containing all messages which will be going out from the server to a client
pub mod outgoing {
    use crate::messages::*;

    /// Chat server sends this messages to session
    #[derive(Message, Clone)]
    pub struct Message(pub String);

    #[derive(Message)]
    pub struct AddCardToHand {
        pub room: String, 
        pub player: Player, 
        pub card: Card
    }

    #[derive(Message)]
    pub struct RemoveCardFromHand {
        pub room: String,
        pub player: Player,
        pub card: Card,
    }

    // Send when someone joins your match
    #[derive(Message)]
    pub struct PlayerJoinedMatch {
        pub token: CookieToken,
        pub room: String,
        pub player: Player,
    }

    // Send when someone leaves your match
    #[derive(Message)]
    pub struct PlayerLeftMatch {
        pub token: CookieToken,
        pub room: String,
        pub player: Player,
    }
    
    #[derive(Message)]
    pub struct MatchHasStarted {
        pub room: String,
    }

    #[derive(Message)]
    pub struct PlayerWonMatch {
        pub room: String,
        pub player: Player,
    }

    #[derive(Message)]
    pub struct NewRoundStarted {
        pub room: String,
    }

    #[derive(Message)]
    pub struct NewCzar {
        pub room: String,
        pub id: PlayerId,
    }
}