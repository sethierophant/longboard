use std::fmt::Debug;

use chrono::offset::Utc;
use chrono::DateTime;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{delete, insert_into, prelude::*, sql_query};

use serde::Serialize;

use crate::schema::{file, post, report, thread};
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
    /// The argon2 hash of the identity the user gave.
    pub author_ident: Option<String>,
    /// The thread that this post was posted on.
    pub thread_id: ThreadId,
    /// The argon2 hash of the password the user gave for deletion.
    pub delete_hash: Option<String>,
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

/// A report that a user made about a post.
#[derive(Debug, Queryable, Serialize)]
pub struct Report {
    /// The reason the post should be removed.
    pub reason: String,
    /// When the report was made.
    pub time_stamp: DateTime<Utc>,
    /// The post.
    pub post: PostId,
}

/// A new thread to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "thread"]
pub struct NewThread {
    pub subject: String,
    pub board: String,
}

/// A new post to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "post"]
pub struct NewPost {
    pub body: String,
    pub author_name: Option<String>,
    pub author_contact: Option<String>,
    pub author_ident: Option<String>,
    pub delete_hash: Option<String>,
    pub thread: ThreadId,
}

/// A new file to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "file"]
pub struct NewFile {
    pub save_name: String,
    pub thumb_name: Option<String>,
    pub orig_name: Option<String>,
    pub content_type: Option<String>,
    pub post: PostId,
}

/// A new report to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "report"]
pub struct NewReport {
    pub reason: String,
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
    pub fn open<S>(url: S) -> Result<Database>
    where
        S: AsRef<str>,
    {
        let pool = Pool::new(ConnectionManager::new(url.as_ref()))?;

        Ok(Database { pool })
    }

    /// Get all boards.
    pub fn all_boards(&self) -> Result<Vec<Board>> {
        use crate::schema::board::dsl::board;

        Ok(board.load(&self.pool.get()?)?)
    }

    /// Get a board.
    pub fn board<S>(&self, board_name: S) -> Result<Board>
    where
        S: AsRef<str>,
    {
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        Ok(board
            .filter(name.eq(board_name.as_ref()))
            .first(&self.pool.get()?)?)
    }

    /// Get threads on a board.
    pub fn threads_on_board<S>(&self, board_name: S) -> Result<Vec<Thread>>
    where
        S: AsRef<str>,
    {
        use crate::schema::thread::columns::board;
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(board.eq(board_name.as_ref()))
            .load(&self.pool.get()?)?)
    }

    /// Get a thread.
    pub fn thread<S>(&self, board_name: S, thread_id: ThreadId) -> Result<Thread>
    where
        S: AsRef<str>,
    {
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

    /// Get a post.
    pub fn post(&self, post_id: PostId) -> Result<Post> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        Ok(post
            .filter(id.eq(post_id))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    /// Get the thread that a post belongs to.
    pub fn parent_thread(&self, post_id: PostId) -> Result<Thread> {
        let thread_id: ThreadId = {
            use crate::schema::post::columns::{id, thread};
            use crate::schema::post::dsl::post;

            post.filter(id.eq(post_id))
                .select(thread)
                .limit(1)
                .first(&self.pool.get()?)?
        };

        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(id.eq(thread_id))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    pub fn is_first_post(&self, post_id: PostId) -> Result<bool> {
        let post: Post = {
            use crate::schema::post::columns::id;
            use crate::schema::post::dsl::post;

            post.filter(id.eq(post_id))
                .limit(1)
                .first(&self.pool.get()?)?
        };

        let first_post_id: PostId = {
            use crate::schema::post::columns::{id, thread};
            use crate::schema::post::dsl::post as post_;

            post_
                .filter(thread.eq(post.thread_id))
                .select(id)
                .order(id.asc())
                .limit(1)
                .first(&self.pool.get()?)?
        };

        Ok(post_id == first_post_id)
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
    pub fn insert_file(&self, new_file: NewFile) -> Result<()> {
        use crate::schema::file::dsl::file;

        insert_into(file)
            .values(&new_file)
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Insert a new post report.
    pub fn insert_report(&self, new_report: NewReport) -> Result<()> {
        use crate::schema::report::dsl::report;

        insert_into(report)
            .values(&new_report)
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete a thread.
    pub fn delete_thread(&self, thread_id: ThreadId) -> Result<()> {
        let query = format!(
            "DELETE FROM report R USING post P \
                             WHERE R.post = P.id AND P.thread = {}",
            thread_id
        );
        sql_query(query).execute(&self.pool.get()?)?;

        let query = format!(
            "DELETE FROM file F USING post P \
                             WHERE F.post = P.id AND P.thread = {}",
            thread_id
        );
        sql_query(query).execute(&self.pool.get()?)?;

        {
            use crate::schema::post::columns::thread;
            use crate::schema::post::dsl::post;
            delete(post.filter(thread.eq(thread_id))).execute(&self.pool.get()?)?;
        }

        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;
        delete(thread.filter(id.eq(thread_id))).execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete a post.
    pub fn delete_post(&self, post_id: PostId) -> Result<()> {
        {
            use crate::schema::report::columns::post;
            use crate::schema::report::dsl::report;
            delete(report.filter(post.eq(post_id))).execute(&self.pool.get()?)?;
        }

        {
            use crate::schema::file::columns::post;
            use crate::schema::file::dsl::file;
            delete(file.filter(post.eq(post_id))).execute(&self.pool.get()?)?;
        }

        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;
        delete(post.filter(id.eq(post_id))).execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete all the files that belong to a post.
    pub fn delete_files_of_post(&self, post_id: PostId) -> Result<()> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;
        delete(file.filter(post.eq(post_id))).execute(&self.pool.get()?)?;

        Ok(())
    }
}
