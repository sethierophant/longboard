//! Views, types to generate layouts.
//!
//! Most of these types are meant to be returned from a route.

use maplit::hashmap;

use serde::{Serialize, Serializer};

use serde_json::value::{to_value, Value as JsonValue};

use rocket::uri;

use crate::config::Banner;
use crate::models::*;
use crate::{config::Config, Result};

pub mod staff;

#[macro_export]
macro_rules! impl_template_responder {
    ($t:ty, $template:expr) => {
        impl<'r> ::rocket::response::Responder<'r> for $t {
            fn respond_to(
                self,
                req: &::rocket::request::Request,
            ) -> ::rocket::response::Result<'r> {
                let data = ::serde_json::value::to_value(self).expect("could not serialize value");
                let template = ::rocket_contrib::templates::Template::render($template, data);

                log::trace!("Rendering template at {}", $template);

                template.respond_to(req)
            }
        }
    };
}

/// Display information for a page.
#[derive(Debug, Serialize)]
pub struct PageInfo {
    /// A list of boards the user can go to.
    pub boards: Vec<Board>,
    /// The verson of the longboard server.
    pub version: String,
}

impl PageInfo {
    fn new(db: &Database, _config: &Config) -> Result<PageInfo> {
        Ok(PageInfo {
            boards: db.all_boards()?,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

/// Display information about a page on a specific board.
#[derive(Debug, Serialize)]
pub struct BoardPageInfo {
    /// The board we're on.
    pub board: Board,
    /// The banner to be displayed.
    pub banner: BannerView,
    /// A site notice to be displayed at the top of the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice_html: Option<String>,
}

impl BoardPageInfo {
    fn new<S>(board_name: S, db: &Database, config: &Config) -> Result<BoardPageInfo>
    where
        S: AsRef<str>,
    {
        Ok(BoardPageInfo {
            board: db.board(board_name)?,
            banner: BannerView(config.choose_banner().clone()),
            notice_html: config.notice_html.clone(),
        })
    }
}

/// A wrapper for a banner that can be passed into a template.
#[derive(Debug)]
pub struct BannerView(Banner);

impl Serialize for BannerView {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data = hashmap! {
            "name" => self.0.name.clone(),
            "uri" => self.0.uri(),
        };

        data.serialize(serializer)
    }
}

/// A wrapper for file that can be passed into a template.
#[derive(Debug)]
pub struct FileView(File);

impl Serialize for FileView {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let uri = self.0.uri();
        let thumb_uri = self.0.thumb_uri();

        let mut data = to_value(&self.0).expect("could not serialize file");

        let obj = data.as_object_mut().unwrap();

        obj.insert("uri".into(), JsonValue::String(uri));

        thumb_uri.map(|thumb_uri| obj.insert("thumb_uri".into(), JsonValue::String(thumb_uri)));

        data.serialize(serializer)
    }
}

/// A wrapper for post that can be passed into a template.
#[derive(Debug)]
pub struct PostView(Post);

impl Serialize for PostView {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let time_stamp = self.0.time_stamp.format("%F %R").to_string();

        let hash = self
            .0
            .author_ident
            .as_ref()
            .map(|ident| ident.split('$').last().unwrap().to_owned());

        let uri = self.0.uri();

        let report_uri =
            uri!(crate::routes::report: &self.0.board_name, self.0.thread_id, self.0.id)
                .to_string();
        let delete_uri =
            uri!(crate::routes::delete: &self.0.board_name, self.0.thread_id, self.0.id)
                .to_string();

        let mut data = to_value(&self.0).expect("could not serialize post");

        let obj = data.as_object_mut().unwrap();

        obj.insert("time_stamp".into(), JsonValue::String(time_stamp));
        obj.insert("uri".into(), JsonValue::String(uri));
        obj.insert("report_uri".into(), JsonValue::String(report_uri));
        obj.insert("delete_uri".into(), JsonValue::String(delete_uri));

        if let Some(ident) = hash {
            obj.insert("author_ident".into(), JsonValue::String(ident));
        }

        data.serialize(serializer)
    }
}

/// A wrapper for thread that can be passed into a template.
#[derive(Debug)]
pub struct ThreadView(Thread);

impl Serialize for ThreadView {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let uri = self.0.uri();

        let mut data = to_value(&self.0).expect("could not serialize thread");

        data.as_object_mut()
            .unwrap()
            .insert("uri".into(), JsonValue::String(uri));

        data.serialize(serializer)
    }
}

/// A post and it's file, if it has one.
#[derive(Debug)]
pub struct DeepPost(PostView, Option<FileView>);

impl DeepPost {
    fn new(post_id: PostId, db: &Database) -> Result<DeepPost> {
        let post = PostView(db.post(post_id)?);
        let file = db.files_in_post(post_id)?.pop().map(FileView);
        Ok(DeepPost(post, file))
    }
}

impl Serialize for DeepPost {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let DeepPost(post, file) = self;

        let mut post_data = to_value(post).expect("could not serialize post");

        if let Some(file) = file {
            let file_data = to_value(file).expect("could not serialize file");

            post_data
                .as_object_mut()
                .unwrap()
                .insert(String::from("file"), file_data);
        }

        post_data.serialize(serializer)
    }
}

/// A thread and all of it's posts.
#[derive(Debug)]
pub struct DeepThread(ThreadView, Vec<DeepPost>);

impl DeepThread {
    fn new(thread_id: ThreadId, db: &Database) -> Result<DeepThread> {
        let thread = ThreadView(db.thread(thread_id)?);
        let posts = db.posts_in_thread(thread_id)?;

        let deep_posts = posts
            .into_iter()
            .map(|post| {
                let file = db.files_in_post(post.id)?.pop();
                Ok(DeepPost(PostView(post), file.map(FileView)))
            })
            .collect::<Result<_>>()?;

        Ok(DeepThread(thread, deep_posts))
    }
}

impl Serialize for DeepThread {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let DeepThread(thread, posts) = self;

        let mut thread_data = to_value(thread).expect("could not serialize thread");

        thread_data.as_object_mut().unwrap().insert(
            "posts".into(),
            to_value(posts).expect("could not serialize posts"),
        );

        thread_data.serialize(serializer)
    }
}

/// A recent post to be displayed on the home page.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct RecentPost(Post);

impl RecentPost {
    fn load(db: &Database, limit: u32) -> Result<Vec<RecentPost>> {
        Ok(db
            .recent_posts(limit)?
            .into_iter()
            .map(RecentPost)
            .collect())
    }
}

/// A recent file to be displayed on the home page.
#[derive(Debug, Serialize)]
pub struct RecentFile {
    post: PostView,
    file: FileView,
}

impl RecentFile {
    fn load(db: &Database, limit: u32) -> Result<Vec<RecentFile>> {
        Ok(db
            .recent_files(limit)?
            .into_iter()
            .map(|file| {
                Ok(RecentFile {
                    post: PostView(db.post(file.post_id)?),
                    file: FileView(file),
                })
            })
            .collect::<Result<_>>()?)
    }
}

/// The home page.
#[derive(Debug, Serialize)]
pub struct HomePage {
    page_info: PageInfo,
    recent_posts: Vec<RecentPost>,
    recent_files: Vec<RecentFile>,
}

impl HomePage {
    pub fn new(db: &Database, config: &Config) -> Result<HomePage> {
        Ok(HomePage {
            page_info: PageInfo::new(db, config)?,
            recent_posts: RecentPost::load(db, 5)?,
            recent_files: RecentFile::load(db, 5)?,
        })
    }
}

impl_template_responder!(HomePage, "pages/home");

/// A page for a board.
#[derive(Debug, Serialize)]
pub struct BoardPage {
    page_info: PageInfo,
    board_info: BoardPageInfo,
    threads: Vec<DeepThread>,
}

impl BoardPage {
    pub fn new<S>(board_name: S, db: &Database, config: &Config) -> Result<BoardPage>
    where
        S: AsRef<str>,
    {
        let board_name = board_name.as_ref();

        let threads = db
            .threads_on_board(board_name)?
            .into_iter()
            .map(|thread| DeepThread::new(thread.id, &db))
            .collect::<Result<_>>()?;

        Ok(BoardPage {
            page_info: PageInfo::new(db, config)?,
            board_info: BoardPageInfo::new(board_name, db, config)?,
            threads,
        })
    }
}

impl_template_responder!(BoardPage, "pages/models/board");

/// A page for a thread.
#[derive(Debug, Serialize)]
pub struct ThreadPage {
    page_info: PageInfo,
    board_info: BoardPageInfo,
    thread: DeepThread,
}

impl ThreadPage {
    pub fn new<S>(
        board_name: S,
        thread_id: ThreadId,
        db: &Database,
        config: &Config,
    ) -> Result<ThreadPage>
    where
        S: AsRef<str>,
    {
        Ok(ThreadPage {
            page_info: PageInfo::new(db, config)?,
            board_info: BoardPageInfo::new(&board_name, db, config)?,
            thread: DeepThread::new(thread_id, db)?,
        })
    }
}

impl_template_responder!(ThreadPage, "pages/models/thread");

/// A post preview.
///
/// This is used with the javascript for displaying post previews when a user
/// hovers over a post reference link.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct PostPreview {
    pub post: DeepPost,
}

impl PostPreview {
    pub fn new(post_id: PostId, db: &Database) -> Result<PostPreview> {
        Ok(PostPreview {
            post: DeepPost::new(post_id, db)?,
        })
    }
}

impl_template_responder!(PostPreview, "models/post");

/// A page for reporting a post.
#[derive(Debug, Serialize)]
pub struct ReportPage {
    pub post: Post,
}

impl_template_responder!(ReportPage, "pages/actions/report");

/// A page for deleting a post.
#[derive(Debug, Serialize)]
pub struct DeletePage {
    pub post: Post,
}

impl_template_responder!(DeletePage, "pages/actions/delete");

/// A page to display a success message about a message.
#[derive(Debug, Serialize)]
pub struct ActionSuccessPage {
    pub msg: String,
    pub redirect_uri: String,
}

impl_template_responder!(ActionSuccessPage, "pages/actions/action-success");
