use crate::cah_server::{Card, CardId, Player};
use crate::CookieToken;
use actix::prelude::*;
use uuid::Uuid;

// Containing all messages which will be commin in from a client to the server
pub mod incomming {
    use crate::messages::*;

    /// New chat session is created
    #[derive(Message)]
    pub struct Connect {
        pub addr: Recipient<outgoing::Message>,
        pub token: CookieToken,
    }

    /// Session is disconnected
    #[derive(Message)]
    pub struct Disconnect {
        pub token: CookieToken,
    }

    /// Send message to specific room
    #[derive(Message)]
    pub struct ClientMessage {
        /// Id of the client session
        pub token: CookieToken,
        /// Peer message
        pub msg: String,
        /// Room name
        pub room: String,
    }

    /// List of available rooms request, this doesn't need to be over a websocket actually.
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

    /// Response to List of available rooms request, this doesn't need to be over a websocket actually.
    pub struct ListRooms;
    impl actix::Message for ListRooms {
        type Result = Vec<String>;
    }

    #[derive(Message)]
    pub struct AddCardToHand {
        pub room: String, 
        pub player: Player, 
        pub card: Card
    }
}