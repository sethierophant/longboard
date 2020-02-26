use chrono::DateTime;
use chrono::offset::Utc;

use diesel::prelude::*;
use diesel::pg::PgConnection;

use crate::{Error, Result};

pub type ThreadId = i32;
pub type PostId = i32;

/// A collection of post threads about a similar topic.
#[derive(Debug, Queryable)]
pub struct Board {
    /// The unique name of the board.
    pub name: String,
    /// The description of the board.
    pub description: String,
}

/// A series of posts about a specific subject.
#[derive(Debug, Queryable)]
pub struct Thread {
    /// The ID of the thread.
    pub id: ThreadId,
    /// When the thread was created.
    pub time_stamp: DateTime<Utc>,
    /// The subject of the thread.
    pub subject: String,
    /// The board that this thread was created on.
    pub board_name: String,
}

/// A user-made post.
#[derive(Debug, Queryable)]
pub struct Post {
    /// The ID of the post.
    pub id: PostId,
    /// When the post was created.
    pub time_stamp: DateTime<Utc>,
    /// The contents of the post.
    pub body: String,
    /// The name of the author.
    pub author_name: Option<String>,
    /// A method of contact for the author such as an e-mail address.
    pub author_contact: Option<String>,
    /// A method of identification for the author such as a tripcode.
    pub author_ident: Option<String>,
    /// The thread that this post was posted on.
    pub thread_id: ThreadId,
}

pub static DATABASE_URL: &str = "postgres://longboard:@localhost/longboard";

pub struct Database {
    conn: PgConnection,
}

impl Database {
    /// Open a connection to the database.
    pub fn open() -> Result<Database> {
        Ok(Database { conn: PgConnection::establish(DATABASE_URL)? })
    }

    /// Get all boards.
    pub fn all_boards(&self) -> Result<Vec<Board>> {
        use crate::schema::board::dsl::board;

        Ok(board.load::<Board>(&self.conn)?)
    }

    /// Get a board by name.
    pub fn board<S: AsRef<str>>(&self, board_name: S) -> Result<Board> {
        use crate::schema::board::dsl::board;
        use crate::schema::board::columns::name;

        let query = board.filter(name.eq(board_name.as_ref())).limit(1);
        let err = Error::NotFoundInDatabase {
            what: format!("Board with name \"{}\"", board_name.as_ref()),
        };

        Ok(query.load::<Board>(&self.conn)?.pop().ok_or(err)?)
    }

    /// Get a thread by ID.
    pub fn thread(&self, thread_id: ThreadId) -> Result<Thread> {
        use crate::schema::thread::dsl::thread;
        use crate::schema::thread::columns::id;

        let query = thread.filter(id.eq(thread_id)).limit(1);
        let err = Error::NotFoundInDatabase {
            what: format!("Thread with id \"{}\"", thread_id),
        };

        Ok(query.load::<Thread>(&self.conn)?.pop().ok_or(err)?)
    }
}
