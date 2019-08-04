use actix_web::{web, Error};
use actix_threadpool;

use futures::Future;
use futures::future::{ok as fut_ok, err as fut_err};
use r2d2;
use r2d2_sqlite;
use rusqlite::{params, NO_PARAMS};

use uuid::Uuid;
use str_macro::str;

use std::error;
use std::fmt;
use std::sync::Arc;

use crate::cah_server::{Player, PlayerId, PasswordHash, PASSWORD_HASH_BYTE_SIZE};


pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub trait DbQuery: Send{
    type Item: Send;

    fn execute(&mut self, connection: Connection) -> Result<Self::Item, DbError>;
}

#[derive(Debug, Clone)]
pub struct DbError {

}
impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Something went wrong with a db query")
    }
}
impl error::Error for DbError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}
impl From<r2d2::Error> for DbError {
    fn from(_r2d2_error: r2d2::Error) -> Self {
        DbError{}
    }
}
impl From<actix_threadpool::BlockingError<DbError>> for DbError {
    fn from(_blocking_db_error: actix_threadpool::BlockingError<DbError>) -> Self {
        DbError{}
    }
}
impl From<String> for DbError {
    fn from(string: String) -> Self {
        DbError{}
    }
}
impl From<rusqlite::Error> for DbError {
    fn from(rusqlite_err: rusqlite::Error) -> DbError {
        DbError{}
    }
}

#[derive(Default, Debug, Clone)]
pub struct DatabasePlayer {
    player: Player,
    email: String,
    password_hash: PasswordHash, 
    salt: Uuid,
}

pub struct Database {
    connection_pool: Arc<Pool>,
}
impl Database {
    pub fn new(connection_pool: Pool) -> Self {
        if let Ok(connection) = connection_pool.get() {
            let create_tables_stmt = "CREATE TABLE IF NOT EXISTS players (
                                        player_id INTEGER PRIMARY KEY UNIQUE,
                                        player_name VARCHAR(32) NOT NULL,
                                        email VARCHAR(254) NOT NULL UNIQUE,
                                        password_hash CHAR(64) NOT NULL,
                                        salt CHAR(16) NOT NULL
                                        );

                                        CREATE TABLE IF NOT EXISTS cards (
                                        card_id INTEGER PRIMARY KEY UNIQUE,
                                        deck VARCHAR(64) NOT NULL,
                                        card_content VARCHAR(255) NOT NULL,
                                        is_black BIT NOT NULL
                                        );
                                        ";
            let _exec_res = connection.execute(create_tables_stmt, NO_PARAMS).map_err( |err| println!("There was an error initializing db: {:?}", err) );
        } else {
            println!("ERROR: Couldn't aquire a sqlite3 connection, and the default tables are not created");
        }

        Database{ connection_pool: Arc::new(connection_pool) }
    }

    pub fn execute<Query: DbQuery + 'static>(&mut self, mut query: Query) -> impl Future<Item=<Query as DbQuery>::Item, Error=DbError>{
        let pool = self.connection_pool.clone();
        
        web::block(|| {
            let connection = pool.get()?;
            query.execute(connection)
        })
        .from_err()
    }
}

pub struct RegisterPlayer {
    pub username: String,
    pub email: String,
    pub password_hash: PasswordHash,
    pub salt: Uuid,
}
impl DbQuery for RegisterPlayer {
    type Item = ();

    fn execute(&mut self, connection: Connection) -> Result<Self::Item, DbError> {
        let stmt = "INSERT INTO players (player_name, email, password_hash, salt)
                    VALUES
                     (?1, ?2, ?3, ?4)
                    ";
        connection.execute(
            stmt, 
            params![self.username, self.email, base64::encode_config(self.password_hash.as_slice(), base64::STANDARD_NO_PAD), self.salt.to_simple_ref().to_string()])
            .map_err(|_db_err| str!("Inserting player went wrong!"))?;

        Ok(())
    }
}

/// Returns: (player_id, password_hash, salt): (i64, PasswordHash, Uuid)
pub struct LoginPlayer {
    pub username_or_email: String
}
impl DbQuery for LoginPlayer {
    type Item = (i64, PasswordHash, Uuid);

    fn execute(&mut self, connection: Connection) -> Result<Self::Item, DbError> {
        let query_salt_stmt = "
            SELECT 
             player_id, password_hash, salt
            FROM 
             players
            WHERE
             player_name = ?1 OR player_email = ?1
            LIMIT 1
            ";
        
        let mut preped_salt_query = connection.prepare(query_salt_stmt).map_err(|err| format!("Error preparing db statement: {:?}", err))?;
        let player_salt_iter = preped_salt_query.query_map::<(i64, PasswordHash, Uuid), _, _>(
            params![self.username_or_email], 
            |row| { 
                let pw_hash_string: String = row.get(1)?;
                assert!(pw_hash_string.len() == PASSWORD_HASH_BYTE_SIZE);
                
                Ok( (row.get(0)?, PasswordHash::clone_from_slice(pw_hash_string.into_bytes().as_slice()), row.get(2)?) ) 
            }).map_err(|err| format!("Returning playersalt failed: {:?}", err))?;
        
        let players_and_salt: Vec<_> = player_salt_iter.collect();
        if players_and_salt.len() > 0 {
            debug_assert!(players_and_salt.len() == 1, 
                "There should never be duplicates, wait maybe if the username is not unique. Well it shouldn't anyway");

            match &players_and_salt[0] {
                Ok((player_id, db_password_hash, salt)) => Ok((*player_id, *db_password_hash, *salt)),
                Err(db_err) => Err(DbError{}),
            }
        } else {
            Err(DbError{})
        }
    }
}

pub struct GetPlayerById {
    player_id: PlayerId,
}
impl DbQuery for GetPlayerById {
    type Item = Player;

    fn execute(&mut self, connection: Connection) -> Result<Self::Item, DbError> {
        let get_player_stmt = "
            SELECT
             player_name
            FROM
             players
            WHERE
             player_id=?1
            LIMIT
             1
            ";
        
        let get_player_query = connection.prepare(get_player_stmt)?;
        let player_iterator = get_player_query.query_map::<String, _, _>(params![self.player_id], |row| Ok(row.get(0)?) )?;
        let players: Vec<_> = player_iterator.collect();
        
        if players.len() == 0 { return Err(DbError{}); }

        debug_assert!(players.len() == 1, "There are more players, there can only be 1 with a specified id");

        Ok(Player{id: self.player_id, name: players[0].unwrap()})
    }
}

// impl Default for Database {
//     fn default() -> Self {
//         let card_id_counter: u64 = 11;
//         let decks = hashmap!{
//             str!("Default") => CardDeck{
//                 deck_name: str!("Default"), 
//                 black_cards: vec![Card{content: str!("Question 1 ____"), id: 1}, Card{content: str!("Question 2 ______"), id: 2}], 
//                 white_cards: vec![
//                     Card{content: str!("Awnser card 1"), id: 3}, Card{content: str!("Awnser card 2"), id: 4}, Card{content: str!("Awnser card 3"), id: 5}, 
//                     Card{content: str!("Awnser card 4"), id: 6}, Card{content: str!("Awnser card 5"), id: 7}, Card{content: str!("Awnser card 6"), id: 8}, Card{content: str!("Awnser card 7"), id: 9}]
//                 }
//         };
//         let players = vec![];

//         Database{
//             card_decks: decks,
//             players: players,
//             card_id_counter: card_id_counter,
//         }
//     }
// }
