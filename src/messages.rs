use crate::cah_server::{Card, CardId, Player};
use crate::CookieToken;
use actix::prelude::*;
use std::string::String;

// Containing all messages which will be commin in from a client to the server
pub mod incomming {
    use crate::messages::*;

    /// When a socket connection has been established and the socket wants to be bound to a match
    pub struct SocketConnectMatch {
        pub addr: Recipient<outgoing::Message>,
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
        type Result = Result<(), String>;
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

    /// Session is disconnected
    #[derive(Message)]
    pub struct Disconnect {
        pub token: CookieToken,
    }

    // /// Send message to specific room
    // #[derive(Message)]
    // pub struct ClientMessage {
    //     /// Id of the client session
    //     pub token: CookieToken,
    //     /// Peer message
    //     pub msg: String,
    //     /// Room name
    //     pub room: String,
    // }

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
    }
}

// Containing all messages which will be going out from the server to a client
pub mod outgoing {
    use crate::messages::*;

    /// Chat server sends this messages to session
    #[derive(Message)]
    pub struct Message(pub String);

    #[derive(Message)]
    pub struct AddCardToHand {
        pub room: String, 
        pub player: Player, 
        pub card: Card
    }
}