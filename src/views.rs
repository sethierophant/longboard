use std::convert::TryFrom;
use std::path::PathBuf;

use maplit::hashmap;

use rocket::request::Request;
use rocket::response::{self, Responder};
use rocket::uri;

use rocket_contrib::templates::Template;

use serde::{Serialize, Serializer};

use serde_json::value::{to_value, Value as JsonValue};

use crate::config::Banner;
use crate::models::*;
use crate::{config::Config, Error, Result};

/// Data to be loaded into a template.
///
/// The reason that we have a wrapper struct around [JsonValue] instead of using
/// [JsonValue] directly is so we can implement our own conversion methods.
#[derive(Debug)]
pub struct TemplateData {
    pub json: JsonValue,
}

impl TemplateData {
    /// Convert a serializable object into template data.
    pub fn from_serialize<S>(obj: S) -> Result<Self>
    where
        S: Serialize,
    {
        Ok(TemplateData {
            json: to_value(obj)?,
        })
    }
}

impl Serialize for TemplateData {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.json.serialize(serializer)
    }
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
    /// Create a new 'PageInfo'.
    fn new(db: &Database, _config: &Config) -> Result<PageInfo> {
        Ok(PageInfo {
            boards: db.all_boards()?,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

impl TryFrom<PageInfo> for TemplateData {
    type Error = Error;

    fn try_from(from: PageInfo) -> Result<TemplateData> {
        TemplateData::from_serialize(from)
    }
}

/// Display information about a page on a specific board.
#[derive(Debug, Serialize)]
pub struct BoardPageInfo {
    /// The board we're on.
    pub board: Board,
    /// The banner to be displayed.
    pub banner: Banner,
}

impl BoardPageInfo {
    fn new<S>(board_name: S, db: &Database, config: &Config) -> Result<BoardPageInfo>
    where
        S: AsRef<str>,
    {
        Ok(BoardPageInfo {
            board: db.board(board_name)?,
            banner: config.choose_banner().clone(),
        })
    }
}

impl TryFrom<BoardPageInfo> for TemplateData {
    type Error = Error;

    fn try_from(from: BoardPageInfo) -> Result<TemplateData> {
        let data = hashmap! {
            "board" => to_value(from.board)?,
            "banner_uri" => JsonValue::String(from.banner.uri().to_string()),
        };

        TemplateData::from_serialize(data)
    }
}

impl TryFrom<File> for TemplateData {
    type Error = Error;

    fn try_from(from: File) -> Result<TemplateData> {
        let uri = from
            .thumb_name
            .as_ref()
            .map(|name| uri!(crate::routes::upload_file: PathBuf::from(name)).to_string());

        let mut data = to_value(from)?;

        if let Some(uri) = uri {
            data.as_object_mut()
                .unwrap()
                .insert("uri".into(), JsonValue::String(uri));
        }

        TemplateData::from_serialize(data)
    }
}

impl TryFrom<Post> for TemplateData {
    type Error = Error;

    fn try_from(from: Post) -> Result<TemplateData> {
        let author = from
            .author_name
            .as_deref()
            .unwrap_or("Anonymous")
            .to_owned();

        let date = from.time_stamp.format("%F %R").to_string();

        let hash = from
            .author_ident
            .as_ref()
            .map(|ident| ident.split('$').last().unwrap().to_owned());

        let mut data = to_value(from)?;

        let obj = data.as_object_mut().unwrap();
        obj.insert("time_stamp".into(), JsonValue::String(date));
        obj.insert("author_name".into(), JsonValue::String(author));

        if let Some(ident) = hash {
            obj.insert("author_ident".into(), JsonValue::String(ident));
        }

        TemplateData::from_serialize(data)
    }
}

impl TryFrom<Thread> for TemplateData {
    type Error = Error;

    fn try_from(from: Thread) -> Result<TemplateData> {
        let uri = uri!(crate::routes::thread: &from.board_name, from.id).to_string();

        let mut data = to_value(from)?;

        data.as_object_mut()
            .unwrap()
            .insert("uri".into(), JsonValue::String(uri));

        TemplateData::from_serialize(data)
    }
}

impl<T> TryFrom<Vec<T>> for TemplateData
where
    TemplateData: TryFrom<T, Error = Error>,
{
    type Error = Error;

    fn try_from(from: Vec<T>) -> Result<TemplateData> {
        let data = from
            .into_iter()
            .map(TemplateData::try_from)
            .collect::<Result<Vec<_>>>()?;

        Ok(TemplateData::from_serialize(data)?)
    }
}

impl TryFrom<(Post, Option<File>)> for TemplateData {
    type Error = Error;

    fn try_from(from: (Post, Option<File>)) -> Result<TemplateData> {
        let (post, file) = from;

        let mut post_data = TemplateData::try_from(post)?.json;

        if let Some(file) = file {
            let file_data = TemplateData::try_from(file)?.json;

            post_data
                .as_object_mut()
                .unwrap()
                .insert(String::from("file"), file_data);
        }

        TemplateData::from_serialize(post_data)
    }
}

impl TryFrom<(Thread, Vec<(Post, Option<File>)>)> for TemplateData {
    type Error = Error;

    fn try_from(from: (Thread, Vec<(Post, Option<File>)>)) -> Result<TemplateData> {
        let (thread, posts) = from;

        let mut thread_data = TemplateData::try_from(thread)?.json;

        thread_data
            .as_object_mut()
            .unwrap()
            .insert("posts".into(), TemplateData::try_from(posts)?.json);

        TemplateData::from_serialize(thread_data)
    }
}

#[derive(Debug, Serialize)]
pub struct HomeView {
    page_info: PageInfo,

    num_threads: i64,
    num_posts: i64,
}

impl HomeView {
    pub fn new(db: &Database, config: &Config) -> Result<HomeView> {
        Ok(HomeView {
            page_info: PageInfo::new(db, config)?,

            num_threads: db.num_threads()?,
            num_posts: db.num_posts()?,
        })
    }
}

impl TryFrom<HomeView> for TemplateData {
    type Error = Error;

    fn try_from(from: HomeView) -> Result<TemplateData> {
        TemplateData::from_serialize(from)
    }
}

impl<'r> Responder<'r> for HomeView {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        TemplateData::try_from(self)
            .map(|data| Template::render("layout/home", &data))
            .respond_to(req)
    }
}

#[derive(Debug)]
pub struct BoardView {
    page_info: PageInfo,
    board_info: BoardPageInfo,

    models: Vec<(Thread, Vec<(Post, Option<File>)>)>,
}

impl BoardView {
    pub fn new<S>(board_name: S, db: &Database, config: &Config) -> Result<BoardView>
    where
        S: AsRef<str>,
    {
        let mut models = Vec::new();

        for thread in db.threads_on_board(&board_name)? {
            let mut post_models = Vec::new();

            for post in db.posts_in_thread(thread.id)? {
                let file = db.files_in_post(post.id)?.pop();
                post_models.push((post, file));
            }

            models.push((thread, post_models));
        }

        Ok(BoardView {
            page_info: PageInfo::new(db, config)?,
            board_info: BoardPageInfo::new(&board_name, db, config)?,
            models,
        })
    }
}

impl TryFrom<BoardView> for TemplateData {
    type Error = Error;

    fn try_from(from: BoardView) -> Result<TemplateData> {
        let data = hashmap! {
            "page_info" => TemplateData::try_from(from.page_info)?.json,
            "board_info" => TemplateData::try_from(from.board_info)?.json,
            "threads" => TemplateData::try_from(from.models)?.json,
        };

        TemplateData::from_serialize(data)
    }
}

impl<'r> Responder<'r> for BoardView {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        TemplateData::try_from(self)
            .map(|data| Template::render("layout/board", &data))
            .respond_to(req)
    }
}

#[derive(Debug)]
pub struct ThreadView {
    page_info: PageInfo,
    board_info: BoardPageInfo,

    models: (Thread, Vec<(Post, Option<File>)>),
}

impl ThreadView {
    pub fn new<S>(
        board_name: S,
        thread_id: ThreadId,
        db: &Database,
        config: &Config,
    ) -> Result<ThreadView>
    where
        S: AsRef<str>,
    {
        let mut models = Vec::new();

        let thread = db.thread(&board_name, thread_id)?;

        for post in db.posts_in_thread(thread_id)? {
            let file = db.files_in_post(post.id)?.pop();
            models.push((post, file));
        }

        Ok(ThreadView {
            page_info: PageInfo::new(db, config)?,
            board_info: BoardPageInfo::new(&board_name, db, config)?,

            models: (thread, models),
        })
    }
}

impl TryFrom<ThreadView> for TemplateData {
    type Error = Error;

    fn try_from(from: ThreadView) -> Result<TemplateData> {
        let data = hashmap! {
            "page_info" => TemplateData::try_from(from.page_info)?.json,
            "board_info" => TemplateData::try_from(from.board_info)?.json,
            "thread" => TemplateData::try_from(from.models)?.json,
        };

        TemplateData::from_serialize(data)
    }
}

impl<'r> Responder<'r> for ThreadView {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        TemplateData::try_from(self)
            .map(|data| Template::render("layout/thread", &data))
            .respond_to(req)
    }
}
