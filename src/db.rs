use actix_web::{web, Error};
use actix_threadpool;

use futures::Future;
use r2d2;
use r2d2_sqlite;
use rusqlite::NO_PARAMS;

use std::error;
use std::fmt;

pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub trait DbQuery: Send{
    type Res: Send;

    fn execute(&mut self, connection: Connection) -> Result<Self::Res, DbError>;
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

pub fn execute<Query: DbQuery + 'static>(pool: &Pool, mut query: Query) -> impl Future<Item=<Query as DbQuery>::Res, Error=DbError>{
    let pool = pool.clone();
    
    web::block(move || {
        let connection = pool.get()?;
        query.execute(connection)
    })
    .from_err()
}
