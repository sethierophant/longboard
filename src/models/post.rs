//! Types related to posts.

use std::fmt::Debug;
use std::path::PathBuf;

use chrono::offset::Utc;
use chrono::DateTime;

use diesel::{delete, insert_into, prelude::*};

use mime::Mime;

use rocket::uri;

use serde::{Serialize, Serializer};

use crate::models::{Connection, *};
use crate::schema::{file, post};
use crate::{Error, Result};

/// A post ID.
pub type PostId = i32;

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
    pub author_name: String,
    /// A method of contact for the author such as an e-mail address.
    pub author_contact: Option<String>,
    /// The argon2 hash of the identity the user gave.
    pub author_ident: Option<String>,
    /// The thread that this post was posted on.
    pub thread_id: ThreadId,
    /// The argon2 hash of the password the user gave for deletion.
    pub delete_hash: Option<String>,
    /// The board that this post was posted on.
    pub board_name: String,
    /// The user that made the post.
    pub user_id: UserId,
    /// Whether a post should not bump it's thread.
    pub no_bump: bool,
}

impl Post {
    /// The URI of the post.
    pub fn uri(&self) -> String {
        let uri =
            uri!(crate::routes::thread: &self.board_name, &self.thread_id);
        format!("{}#{}", uri, self.id)
    }
}

/// A new post to be inserted in the database.
#[derive(Debug, Insertable)]
#[diesel(table_name = post)]
pub struct NewPost {
    pub body: String,
    pub author_name: String,
    pub author_contact: Option<String>,
    pub author_ident: Option<String>,
    pub delete_hash: Option<String>,
    pub thread: ThreadId,
    pub board: String,
    pub user_id: UserId,
    pub no_bump: bool,
}

/// A helper for serializing MIME types.
fn se_content_type<S>(
    content_type: &Mime,
    se: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    content_type.to_string().serialize(se)
}

/// A user-uploaded file.
#[derive(Debug, Serialize)]
pub struct File {
    /// The name the file is saved at.
    pub save_name: String,
    /// The name of the thumbnail of the file, if any.
    pub thumb_name: String,
    /// The original name of the file, if any.
    pub orig_name: Option<String>,
    /// The content-type of the file.
    #[serde(serialize_with = "se_content_type")]
    pub content_type: Mime,
    /// The post that the file belongs to.
    pub post_id: PostId,
    /// Whether or not the file should be hidden by default.
    pub is_spoiler: bool,
}

/// A new file to be inserted in the database.
#[derive(Debug, Insertable)]
#[diesel(table_name = file)]
pub struct NewFile {
    pub save_name: String,
    pub thumb_name: String,
    pub orig_name: Option<String>,
    pub content_type: String,
    pub is_spoiler: bool,
    pub post: PostId,
}

impl From<DbFile> for File {
    fn from(file: DbFile) -> File {
        File {
            save_name: file.save_name,
            thumb_name: file.thumb_name,
            orig_name: file.orig_name,
            content_type: file.content_type.parse().unwrap(),
            post_id: file.post_id,
            is_spoiler: file.is_spoiler,
        }
    }
}

/// The database representation of a file.
#[derive(Debug, Queryable, Serialize)]
struct DbFile {
    /// The name the file is saved at.
    pub save_name: String,
    /// The name of the thumbnail of the file, if any.
    pub thumb_name: String,
    /// The original name of the file, if any.
    pub orig_name: Option<String>,
    /// The content-type of the file.
    pub content_type: String,
    /// The post that the file belongs to.
    pub post_id: PostId,
    /// Whether or not the file should be hidden by default.
    pub is_spoiler: bool,
}

impl From<File> for DbFile {
    fn from(file: File) -> DbFile {
        DbFile {
            save_name: file.save_name,
            thumb_name: file.thumb_name,
            orig_name: file.orig_name,
            content_type: file.content_type.to_string(),
            post_id: file.post_id,
            is_spoiler: file.is_spoiler,
        }
    }
}

impl File {
    /// The URI of the file.
    pub fn uri(&self) -> String {
        uri!(crate::routes::upload: PathBuf::from(&self.save_name)).to_string()
    }

    /// The URI of the file's thumbnail.
    pub fn thumb_uri(&self) -> String {
        uri!(crate::routes::upload: PathBuf::from(&self.thumb_name)).to_string()
    }
}

/// Convenience function to convert from diesel's error type into our error
/// type, when we're querying for a post.
fn conv_post_error(
    post_id: PostId,
) -> impl FnOnce(diesel::result::Error) -> Error {
    move |e: diesel::result::Error| match e {
        diesel::result::Error::NotFound => Error::PostNotFound { post_id },
        _ => Error::from(e),
    }
}

impl<C, M> Connection<C, M>
where
    C: InnerConnection<M> + diesel::connection::LoadConnection,
    M: diesel::connection::TransactionManager<C>,
{
    /// Get a post.
    pub fn post(&mut self, post_id: PostId) -> Result<Post> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        post.filter(id.eq(post_id))
            .limit(1)
            .first(&mut self.inner)
            .map_err(conv_post_error(post_id))
    }

    /// Insert a new post into the database.
    pub fn insert_post(&mut self, new_post: NewPost) -> Result<PostId> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        if self.is_locked(new_post.thread)? {
            return Err(Error::ThreadLocked);
        }

        Ok(insert_into(post)
            .values(&new_post)
            .returning(id)
            .get_result(&mut self.inner)?)
    }

    /// Delete a post.
    pub fn delete_post(&mut self, pid: PostId) -> Result<()> {
        self.inner.transaction::<_, Error, _>(|conn| {
            use crate::schema::file::columns::post as file_post;
            use crate::schema::file::dsl::file as table_file;
            use crate::schema::post::columns::id as post_id;
            use crate::schema::post::dsl::post as table_post;
            use crate::schema::report::columns::post as report_post;
            use crate::schema::report::dsl::report as table_report;

            delete(table_report.filter(report_post.eq(pid))).execute(conn)?;

            delete(table_file.filter(file_post.eq(pid))).execute(conn)?;

            delete(table_post.filter(post_id.eq(pid))).execute(conn)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get the URI for a post.
    pub fn post_uri(&mut self, post_id: PostId) -> Result<String> {
        let thread_uri = self.inner.transaction::<_, Error, _>(|conn| {
            let thread_id: ThreadId = {
                use crate::schema::post::columns::{id, thread};
                use crate::schema::post::dsl::post;

                post.filter(id.eq(post_id))
                    .select(thread)
                    .limit(1)
                    .first(conn)
                    .map_err(conv_post_error(post_id))?
            };

            let board_name: String = {
                use crate::schema::thread::columns::{board, id};
                use crate::schema::thread::dsl::thread;

                thread
                    .filter(id.eq(thread_id))
                    .select(board)
                    .limit(1)
                    .first(conn)?
            };

            Ok(uri!(crate::routes::thread: board_name, thread_id))
        })?;

        Ok(format!("{}#{}", thread_uri.to_string(), post_id))
    }

    /// Get the thread that a post belongs to.
    pub fn parent_thread(&mut self, post_id: PostId) -> Result<Thread> {
        let parent = self.inner.transaction::<_, Error, _>(|conn| {
            use crate::schema::thread::columns::id;
            use crate::schema::thread::dsl::thread;

            let thread_id: ThreadId = {
                use crate::schema::post::columns::{id, thread};
                use crate::schema::post::dsl::post;

                post.filter(id.eq(post_id))
                    .select(thread)
                    .limit(1)
                    .first(conn)
                    .map_err(conv_post_error(post_id))?
            };

            Ok(thread.filter(id.eq(thread_id)).limit(1).first(conn)?)
        })?;

        Ok(parent)
    }

    /// Check whether a post is the first post in a thread (the original post).
    pub fn is_first_post(&mut self, post_id: PostId) -> Result<bool> {
        let first_post_id = self.inner.transaction::<_, Error, _>(|conn| {
            use crate::schema::post::columns::{id, thread};
            use crate::schema::post::dsl::post;

            let Post { thread_id, .. } =
                { post.filter(id.eq(post_id)).limit(1).first(conn)? };

            let first_post_id: i32 = post
                .filter(thread.eq(thread_id))
                .select(id)
                .order(id.asc())
                .limit(1)
                .first(conn)
                .map_err(conv_post_error(post_id))?;

            Ok(first_post_id)
        })?;

        Ok(post_id == first_post_id)
    }

    /// Get all of the files in a post.
    pub fn files_in_post(&mut self, post_id: PostId) -> Result<Vec<File>> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;

        let files: Vec<DbFile> = file
            .filter(post.eq(post_id))
            .load(&mut self.inner)
            .map_err(conv_post_error(post_id))?;

        Ok(files.into_iter().map(File::from).collect())
    }

    /// Insert a new file into the database.
    pub fn insert_file(&mut self, new_file: NewFile) -> Result<()> {
        use crate::schema::file::dsl::file;

        insert_into(file)
            .values(&new_file)
            .execute(&mut self.inner)?;

        Ok(())
    }

    /// Delete all the files that belong to a post.
    pub fn delete_files_of_post(&mut self, post_id: PostId) -> Result<()> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;
        delete(file.filter(post.eq(post_id))).execute(&mut self.inner)?;

        Ok(())
    }

    /// Get the number of posts in the database.
    pub fn num_posts(&mut self) -> Result<i64> {
        use crate::schema::post::dsl::post;

        Ok(post.count().first(&mut self.inner)?)
    }

    /// Get up to `limit` recent posts.
    pub fn recent_posts(&mut self, limit: u32) -> Result<Vec<Post>> {
        use crate::schema::post::columns::time_stamp;
        use crate::schema::post::dsl::post;

        Ok(post
            .order(time_stamp.desc())
            .limit(limit.into())
            .load(&mut self.inner)?)
    }

    /// Get up to `limit` recently uploaded files.
    pub fn recent_files(&mut self, limit: u32) -> Result<Vec<File>> {
        use crate::schema::file::columns::*;
        use crate::schema::file::dsl::file;
        use crate::schema::post::columns::time_stamp;
        use crate::schema::post::dsl::post as post_table;

        let files: Vec<DbFile> = file
            .inner_join(post_table)
            .order(time_stamp.desc())
            .limit(limit.into())
            .select((
                save_name,
                thumb_name,
                orig_name,
                content_type,
                post,
                is_spoiler,
            ))
            .load(&mut self.inner)?;

        Ok(files.into_iter().map(File::from).collect())
    }
}
