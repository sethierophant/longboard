use std::fmt::Debug;

use chrono::offset::Utc;
use chrono::DateTime;

use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

use serde::Serialize;

use crate::schema::{file, post, thread};
use crate::Result;

/// A thread ID.
pub type ThreadId = i32;
/// A post ID.
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

/// A user-uploaded file.
#[derive(Debug, Queryable, Serialize)]
pub struct File {
    /// The name the file is saved at.
    pub save_name: String,
    /// The name of the thumbnail of the file, if any.
    pub thumb_name: Option<String>,
    /// The original name of the file, if any.
    pub orig_name: Option<String>,
    /// The content-type of the file, if any.
    pub content_type: Option<String>,
    /// The post that the file belongs to.
    pub post_id: PostId,
}

/// A new thread to be inserted in the database. See 'Thread' for descriptions
/// of the fields.
#[derive(Debug, Insertable)]
#[table_name = "thread"]
pub struct NewThread<'a> {
    pub time_stamp: DateTime<Utc>,
    pub subject: &'a str,
    pub board: &'a str,
}

/// A new post to be inserted in the database. See 'Post' for descriptions of
/// the fields.
#[derive(Debug, Insertable)]
#[table_name = "post"]
pub struct NewPost<'a> {
    pub time_stamp: DateTime<Utc>,
    pub body: &'a str,
    pub author_name: Option<&'a str>,
    pub author_contact: Option<&'a str>,
    pub author_ident: Option<&'a str>,
    pub thread: ThreadId,
}

/// A new file to be inserted in the database. See 'File' for descriptions of
/// the fields.
#[derive(Debug, Insertable)]
#[table_name = "file"]
pub struct NewFile<'a> {
    pub save_name: &'a str,
    pub thumb_name: Option<&'a str>,
    pub orig_name: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub post: PostId,
}

pub static DATABASE_URL: &str = "postgres://longboard:@localhost/longboard";

/// A connection to the database. Used for creating and retrieving data.
pub struct Database {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Debug for Database {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        let state = self.pool.state();

        write!(
            fmt,
            "<#Database connections={} idle_connections={}>",
            state.connections, state.idle_connections,
        )?;

        Ok(())
    }
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
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        Ok(board
            .filter(name.eq(board_name.as_ref()))
            .first(&self.pool.get()?)?)
    }

    /// Get threads on a board.
    pub fn threads_on_board<S: AsRef<str>>(&self, board_name: S) -> Result<Vec<Thread>> {
        use crate::schema::thread::columns::board;
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(board.eq(board_name.as_ref()))
            .load(&self.pool.get()?)?)
    }

    /// Get a thread.
    pub fn thread<S: AsRef<str>>(&self, board_name: S, thread_id: ThreadId) -> Result<Thread> {
        use crate::schema::thread::columns::{board, id};
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(board.eq(board_name.as_ref()))
            .filter(id.eq(thread_id))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    /// Get all of the posts in a thread.
    pub fn posts_in_thread(&self, thread_id: ThreadId) -> Result<Vec<Post>> {
        use crate::schema::post::columns::thread;
        use crate::schema::post::dsl::post;

        Ok(post.filter(thread.eq(thread_id)).load(&self.pool.get()?)?)
    }

    /// Get all of the files in a post.
    pub fn files_in_post(&self, post_id: PostId) -> Result<Vec<File>> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;

        Ok(file.filter(post.eq(post_id)).load(&self.pool.get()?)?)
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
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        Ok(insert_into(thread)
            .values(&new_thread)
            .returning(id)
            .get_result(&self.pool.get()?)?)
    }

    /// Insert a new post into the database.
    pub fn insert_post(&self, new_post: NewPost) -> Result<PostId> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        Ok(insert_into(post)
            .values(&new_post)
            .returning(id)
            .get_result(&self.pool.get()?)?)
    }

    /// Insert a new file into the database.
    pub fn insert_file(&self, new_file: NewFile) -> Result<usize> {
        use crate::schema::file::dsl::file;

        Ok(insert_into(file)
            .values(&new_file)
            .execute(&self.pool.get()?)?)
    }
}
