//! Views, types to generate layouts.
//!
//! Most of these types are meant to be returned from a route.

use maplit::hashmap;

use serde::{Serialize, Serializer};

use serde_json::value::{to_value, Value as JsonValue};

use rocket::request::{FromRequest, Outcome};
use rocket::{uri, Request, State};

use crate::config::Banner;
use crate::models::staff::Staff;
use crate::models::*;
use crate::routes::UserOptions;
use crate::{config::Config, Error, Result};

pub mod error;
pub mod staff;
use staff::StaffView;

/// Context that's needed to render a page.
#[derive(Clone)]
pub struct Context<'r> {
    pub database: &'r Database,
    pub config: &'r Config,
    pub options: UserOptions,
    pub staff: Option<Staff>,
}

impl<'a, 'r> FromRequest<'a, 'r> for Context<'r> {
    type Error = Error;

    fn from_request(req: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let database = req
            .guard::<State<Database>>()
            .expect("expected database to be initialized")
            .inner();

        let session = req
            .guard::<Option<Session>>()
            .expect("couldn't load session from cookies");

        let staff =
            session.and_then(|session| database.staff(session.staff_name).ok());

        Outcome::Success(Context {
            database,
            staff,
            config: req
                .guard::<State<Config>>()
                .expect("expected config to be initialized")
                .inner(),
            options: req
                .guard::<UserOptions>()
                .expect("couldn't load user options from cookies"),
        })
    }
}

/// Implement `Responder` for a type which implements `Serialize`, given a path
/// to a template file that should be loaded.
#[macro_export]
macro_rules! impl_template_responder {
    ($t:ty, $template:expr) => {
        impl<'r> ::rocket::response::Responder<'r> for $t {
            fn respond_to(
                self,
                req: &::rocket::request::Request,
            ) -> ::rocket::response::Result<'r> {
                let data = ::serde_json::value::to_value(self)
                    .expect("could not serialize value");
                let template = ::rocket_contrib::templates::Template::render(
                    $template, data,
                );

                log::trace!("Rendering template at {}", $template);

                template.respond_to(req)
            }
        }
    };
}

/// Display information for a page.
#[derive(Debug, Serialize)]
pub struct PageInfo {
    /// The title of the page.
    pub title: String,
    /// The verson of the longboard server.
    pub version: String,
    /// Which style to use.
    pub style: String,
}

impl PageInfo {
    pub fn new<S>(title: S, context: &Context) -> PageInfo
    where
        S: Into<String>,
    {
        PageInfo {
            title: title.into(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            style: context.options.style.clone(),
        }
    }
}

/// Display information for a page footer.
#[derive(Debug, Serialize)]
pub struct PageFooter {
    /// A list of admin-created pages.
    pages: Vec<String>,
}

impl PageFooter {
    pub fn new(context: &Context) -> PageFooter {
        PageFooter {
            pages: context.config.options.custom_pages.clone(),
        }
    }
}

/// The board navigation at the top of the page.
#[derive(Debug, Serialize)]
pub struct PageNav {
    pub boards: Vec<Board>,
}

impl PageNav {
    pub fn new(context: &Context) -> Result<PageNav> {
        Ok(PageNav {
            boards: context.database.all_boards()?,
        })
    }
}

/// The header of a board or thread page.
#[derive(Debug, Serialize)]
pub struct PageHeader {
    /// The board we're on.
    pub board: Board,
    /// The banner to be displayed.
    pub banner: BannerView,
    /// A site notice to be displayed at the top of the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice_html: Option<String>,
}

impl PageHeader {
    fn new<S>(board_name: S, context: &Context) -> Result<PageHeader>
    where
        S: AsRef<str>,
    {
        Ok(PageHeader {
            board: context.database.board(board_name)?,
            banner: BannerView(context.config.choose_banner().clone()),
            notice_html: context.config.notice_html.clone(),
        })
    }
}

/// A wrapper for a banner that can be passed into a template.
#[derive(Debug)]
pub struct BannerView(Banner);

impl Serialize for BannerView {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
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
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let uri = self.0.uri();
        let thumb_uri = self.0.thumb_uri();

        let mut data = to_value(&self.0).expect("could not serialize file");

        let obj = data.as_object_mut().unwrap();

        obj.insert("uri".into(), JsonValue::String(uri));

        thumb_uri.map(|thumb_uri| {
            obj.insert("thumb_uri".into(), JsonValue::String(thumb_uri))
        });

        data.serialize(serializer)
    }
}

/// A wrapper for post that can be passed into a template.
#[derive(Debug)]
pub struct PostView(Post);

impl Serialize for PostView {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
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

        let pin_uri = uri!(
            crate::routes::staff::pin:
            &self.0.board_name,
            self.0.thread_id
        )
        .to_string();

        let unpin_uri = uri!(
            crate::routes::staff::unpin:
            &self.0.board_name,
            self.0.thread_id
        )
        .to_string();

        let lock_uri = uri!(
            crate::routes::staff::lock:
            &self.0.board_name,
            self.0.thread_id
        )
        .to_string();

        let unlock_uri = uri!(
            crate::routes::staff::unlock:
            &self.0.board_name,
            self.0.thread_id
        )
        .to_string();

        let report_uri = uri!(
            crate::routes::report:
            &self.0.board_name,
            self.0.thread_id,
            self.0.id
        )
        .to_string();

        let delete_uri = uri!(
            crate::routes::delete:
            &self.0.board_name,
            self.0.thread_id,
            self.0.id
        )
        .to_string();

        let staff_delete_uri = uri!(
            crate::routes::staff::staff_delete:
            &self.0.board_name,
            self.0.thread_id,
            self.0.id
        )
        .to_string();

        let mut data = to_value(&self.0).expect("could not serialize post");

        let obj = data.as_object_mut().unwrap();

        obj.insert("time_stamp".into(), JsonValue::String(time_stamp));
        obj.insert("uri".into(), JsonValue::String(uri));
        obj.insert("pin_uri".into(), JsonValue::String(pin_uri));
        obj.insert("unpin_uri".into(), JsonValue::String(unpin_uri));
        obj.insert("lock_uri".into(), JsonValue::String(lock_uri));
        obj.insert("unlock_uri".into(), JsonValue::String(unlock_uri));
        obj.insert("report_uri".into(), JsonValue::String(report_uri));
        obj.insert("delete_uri".into(), JsonValue::String(delete_uri));
        obj.insert(
            "staff_delete_uri".into(),
            JsonValue::String(staff_delete_uri),
        );

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
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
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
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
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
    /// Load a thread and its posts from the database.
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

    /// Load a thread and a preview of its posts from the database.
    fn new_preview(thread_id: ThreadId, db: &Database) -> Result<DeepThread> {
        let thread = ThreadView(db.thread(thread_id)?);
        let posts =
            db.preview_thread(thread_id, crate::DEFAULT_PREVIEW_LIMIT)?;

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
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let DeepThread(thread, posts) = self;

        let mut thread_data =
            to_value(thread).expect("could not serialize thread");

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
pub struct RecentPost(PostView);

impl RecentPost {
    fn load(db: &Database, limit: u32) -> Result<Vec<RecentPost>> {
        Ok(db
            .recent_posts(limit)?
            .into_iter()
            .map(|post| RecentPost(PostView(post)))
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
    page_nav: PageNav,
    page_footer: PageFooter,
    recent_posts: Vec<RecentPost>,
    recent_files: Vec<RecentFile>,
}

impl HomePage {
    pub fn new(context: &Context) -> Result<HomePage> {
        Ok(HomePage {
            page_info: PageInfo::new("LONGBOARD", context),
            page_nav: PageNav::new(context)?,
            page_footer: PageFooter::new(context),
            recent_posts: RecentPost::load(
                context.database,
                crate::DEFAULT_RECENT_POSTS,
            )?,
            recent_files: RecentFile::load(
                context.database,
                crate::DEFAULT_RECENT_FILES,
            )?,
        })
    }
}

impl_template_responder!(HomePage, "pages/home");

/// A style that the user can select.
#[derive(Debug, Serialize)]
pub struct StyleOption {
    name: String,
    selected: bool,
}

/// A page for user options.
#[derive(Debug, Serialize)]
pub struct OptionsPage {
    page_info: PageInfo,
    page_nav: PageNav,
    page_footer: PageFooter,
    styles: Vec<StyleOption>,
}

impl OptionsPage {
    pub fn new(context: &Context) -> Result<OptionsPage> {
        Ok(OptionsPage {
            page_info: PageInfo::new("Options", context),
            page_nav: PageNav::new(context)?,
            page_footer: PageFooter::new(context),
            styles: context
                .config
                .options
                .custom_styles
                .iter()
                .map(|style| StyleOption {
                    name: style.clone(),
                    selected: style == &context.options.style,
                })
                .collect(),
        })
    }
}

impl_template_responder!(OptionsPage, "pages/options");

/// Information about a link to another page.
#[derive(Debug, Serialize)]
pub struct PageNumLink {
    /// The page number that we're linking to.
    num: u32,
    /// Whether or not this link points to the current page.
    current: bool,
}

impl PageNumLink {
    /// Generate a list of links to all pages.
    pub fn generate(page_count: u32, current_page: u32) -> Vec<PageNumLink> {
        (1..=page_count)
            .map(|num| PageNumLink {
                num,
                current: num == current_page,
            })
            .collect()
    }
}

/// A page for a board.
#[derive(Debug, Serialize)]
pub struct BoardPage {
    page_info: PageInfo,
    page_nav: PageNav,
    page_header: PageHeader,
    page_footer: PageFooter,
    threads: Vec<DeepThread>,
    page_num_links: Vec<PageNumLink>,
    catalog_uri: String,
    staff: Option<StaffView>,
}

impl BoardPage {
    pub fn new<S>(
        board_name: S,
        page_num: u32,
        context: &Context,
    ) -> Result<BoardPage>
    where
        S: AsRef<str>,
    {
        let board_name = board_name.as_ref();
        let page_width = crate::DEFAULT_PAGE_WIDTH;

        let threads = context
            .database
            .thread_page(
                board_name,
                Page {
                    num: page_num,
                    width: page_width,
                },
            )?
            .into_iter()
            .map(|thread| DeepThread::new_preview(thread.id, &context.database))
            .collect::<Result<_>>()?;

        let page_count =
            context.database.thread_page_count(board_name, page_width)?;

        let catalog_uri =
            uri!(crate::routes::board_catalog: board_name).to_string();

        Ok(BoardPage {
            page_info: PageInfo::new(board_name, context),
            page_nav: PageNav::new(context)?,
            page_header: PageHeader::new(board_name, context)?,
            page_footer: PageFooter::new(context),
            threads,
            page_num_links: PageNumLink::generate(page_count, page_num),
            catalog_uri,
            staff: context.staff.clone().map(StaffView),
        })
    }
}

impl_template_responder!(BoardPage, "pages/models/board");

/// A catalog item.
#[derive(Debug, Serialize)]
pub struct CatalogItem {
    thumb_uri: String,
    thread_uri: String,
    subject: String,
    body: String,
    pinned: bool,
    locked: bool,
    num_posts: u32,
    num_files: u32,
}

/// A page for a board catalog.
#[derive(Debug, Serialize)]
pub struct BoardCatalogPage {
    page_info: PageInfo,
    page_nav: PageNav,
    page_header: PageHeader,
    page_footer: PageFooter,
    items: Vec<CatalogItem>,
}

impl BoardCatalogPage {
    pub fn new<S>(board_name: S, context: &Context) -> Result<BoardCatalogPage>
    where
        S: AsRef<str>,
    {
        let board_name = board_name.as_ref();

        let first_posts = context.database.all_first_posts(board_name)?;
        let items = first_posts
            .into_iter()
            .map(|post| {
                let thread = context.database.thread(post.thread_id)?;

                let files = context.database.files_in_post(post.id)?;

                let thumb_uri = files
                    .first()
                    .and_then(|file| file.thumb_uri())
                    .unwrap_or_default();

                let thread_uri = uri!(
                    crate::routes::thread:
                    post.board_name,
                    post.thread_id
                )
                .to_string();

                Ok(CatalogItem {
                    thumb_uri,
                    thread_uri,
                    subject: thread.subject,
                    body: post.body,
                    pinned: thread.pinned,
                    locked: thread.locked,
                    num_posts: context
                        .database
                        .thread_post_count(post.thread_id)?,
                    num_files: context
                        .database
                        .thread_file_count(post.thread_id)?,
                })
            })
            .collect::<Result<_>>()?;

        Ok(BoardCatalogPage {
            page_info: PageInfo::new(board_name, context),
            page_nav: PageNav::new(context)?,
            page_header: PageHeader::new(board_name, context)?,
            page_footer: PageFooter::new(context),
            items,
        })
    }
}

impl_template_responder!(BoardCatalogPage, "pages/models/board-catalog");

/// A page for a thread.
#[derive(Debug, Serialize)]
pub struct ThreadPage {
    page_info: PageInfo,
    page_nav: PageNav,
    page_header: PageHeader,
    page_footer: PageFooter,
    thread: DeepThread,
    staff: Option<StaffView>,
}

impl ThreadPage {
    pub fn new<S>(
        board_name: S,
        thread_id: ThreadId,
        context: &Context,
    ) -> Result<ThreadPage>
    where
        S: AsRef<str>,
    {
        let thread = DeepThread::new(thread_id, context.database)?;
        let DeepThread(ThreadView(Thread { ref subject, .. }), ..) = thread;

        Ok(ThreadPage {
            page_info: PageInfo::new(subject.clone(), context),
            page_nav: PageNav::new(context)?,
            page_header: PageHeader::new(board_name.as_ref(), context)?,
            page_footer: PageFooter::new(context),
            thread,
            staff: context.staff.clone().map(StaffView),
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
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
    pub post: Post,
}

impl ReportPage {
    pub fn new(post_id: PostId, context: &Context) -> Result<ReportPage> {
        Ok(ReportPage {
            page_info: PageInfo::new("Report post", context),
            page_footer: PageFooter::new(context),
            post: context.database.post(post_id)?,
        })
    }
}

impl_template_responder!(ReportPage, "pages/actions/report");

/// A page for deleting a post.
#[derive(Debug, Serialize)]
pub struct DeletePage {
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
    pub post: Post,
}

impl DeletePage {
    pub fn new(post_id: PostId, context: &Context) -> Result<DeletePage> {
        Ok(DeletePage {
            page_info: PageInfo::new("Delete post", context),
            page_footer: PageFooter::new(context),
            post: context.database.post(post_id)?,
        })
    }
}

impl_template_responder!(DeletePage, "pages/actions/delete");

/// A page to display a success message about a message.
#[derive(Debug, Serialize)]
pub struct ActionSuccessPage {
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
    pub msg: String,
    pub redirect_uri: String,
}

impl ActionSuccessPage {
    pub fn new<S1, S2>(
        msg: S1,
        redirect_uri: S2,
        context: &Context,
    ) -> ActionSuccessPage
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        ActionSuccessPage {
            page_info: PageInfo::new("Success", context),
            page_footer: PageFooter::new(context),
            msg: msg.into(),
            redirect_uri: redirect_uri.into(),
        }
    }
}

impl_template_responder!(ActionSuccessPage, "pages/actions/action-success");
