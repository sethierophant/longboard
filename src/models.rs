use chrono::DateTime;
use chrono::offset::Utc;

use diesel::prelude::*;
use diesel::insert_into;
use diesel::r2d2::{Pool, ConnectionManager};

use serde::Serialize;

use crate::Result;
use crate::schema::{thread, post};

pub type ThreadId = i32;
pub type PostId = i32;

/// A collection of post threads about a similar topic.
#[derive(Debug, Queryable, Serialize)]
pub struct Board {
    /// The unique name of the board.
    pub name: String,
    /// The description of the board.
    pub description: String,
}

/// A series of posts about a specific subject.
#[derive(Debug, Queryable, Serialize)]
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
#[derive(Debug, Queryable, Serialize)]
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

/// A new thread to be inserted in the database. See 'Thread' for descriptions
/// of the fields.
#[derive(Debug, Insertable)]
#[table_name = "thread"]
pub struct NewThread {
    pub time_stamp: DateTime<Utc>,
    pub subject: String,
    pub board: String,
}

/// A new post to be inserted in the database. See 'Post' for descriptions of
/// the fields.
#[derive(Debug, Insertable)]
#[table_name = "post"]
pub struct NewPost {
    pub time_stamp: DateTime<Utc>,
    pub body: String,
    pub author_name: Option<String>,
    pub author_contact: Option<String>,
    pub author_ident: Option<String>,
    pub thread: ThreadId,
}

pub static DATABASE_URL: &str = "postgres://longboard:@localhost/longboard";

pub struct Database {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Database {
    /// Open a connection to the database.
    pub fn open() -> Result<Database> {
        let url = "postgres://longboard:@localhost/longboard";
        let pool = Pool::new(ConnectionManager::new(url))?;

        Ok(Database { pool })
    }

    /// Get all boards.
    pub fn all_boards(&self) -> Result<Vec<Board>> {
        use crate::schema::board::dsl::board;

        Ok(board.load(&self.pool.get()?)?)
    }

    /// Get a board.
    pub fn board<S: AsRef<str>>(&self, board_name: S) -> Result<Board> {
        use crate::schema::board::dsl::board;
        use crate::schema::board::columns::name;

        Ok(board
            .filter(name.eq(board_name.as_ref()))
            .first(&self.pool.get()?)?)
    }

    /// Get threads on a board.
    pub fn threads_on_board<S: AsRef<str>>(&self, board_name: S)
        -> Result<Vec<Thread>>
    {
        use crate::schema::thread::dsl::thread;
        use crate::schema::thread::columns::board;

        Ok(thread
           .filter(board.eq(board_name.as_ref()))
           .load(&self.pool.get()?)?)
    }

    /// Get a thread.
    pub fn thread<S: AsRef<str>>(&self, board_name: S, thread_id: ThreadId)
        -> Result<Thread>
    {
        use crate::schema::thread::dsl::thread;
        use crate::schema::thread::columns::{board, id};

        Ok(thread
            .filter(board.eq(board_name.as_ref()))
            .filter(id.eq(thread_id))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    /// Get all of the posts in a thread.
    pub fn posts_in_thread(&self, thread_id: ThreadId) -> Result<Vec<Post>> {
        use crate::schema::post::dsl::post;
        use crate::schema::post::columns::thread;

        Ok(post
           .filter(thread.eq(thread_id))
           .load(&self.pool.get()?)?)
    }

    /// Get the number of threads in the database.
    pub fn num_threads(&self) -> Result<i64> {
        use crate::schema::thread::dsl::thread;
        
        Ok(thread.count().first(&self.pool.get()?)?)
    }

    /// Get the number of posts in the database.
    pub fn num_posts(&self) -> Result<i64> {
        use crate::schema::post::dsl::post;
        
        Ok(post.count().first(&self.pool.get()?)?)
    }

    /// Insert a new thread into the database.
    pub fn insert_thread(&self, new_thread: NewThread) -> Result<ThreadId> {
        use crate::schema::thread::dsl::thread;
        use crate::schema::thread::columns::id;

        Ok(insert_into(thread)
            .values(&new_thread)
            .returning(id)
            .get_result(&self.pool.get()?)?)
    }

    /// Insert a new post into the database.
    pub fn insert_post(&self, new_post: NewPost) -> Result<PostId> {
        use crate::schema::post::dsl::post;
        use crate::schema::post::columns::id;

        Ok(insert_into(post)
            .values(&new_post)
            .returning(id)
            .get_result(&self.pool.get()?)?)
    }
}
