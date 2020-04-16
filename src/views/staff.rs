//! Views for staff pages.

use serde::{Serialize, Serializer};

use serde_json::value::{to_value, Value as JsonValue};

use crate::impl_template_responder;
use crate::models::staff::{Role, Staff, User};
use crate::models::{Board, Report};
use crate::routes::UserOptions;
use crate::views::PageInfo;
use crate::{Database, Result};

#[derive(Debug)]
pub struct StaffView(pub Staff);

impl Serialize for StaffView {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let is_janitor = self.0.role == Role::Janitor;
        let is_administrator = self.0.role == Role::Administrator;
        let is_moderator = self.0.role == Role::Moderator;

        let mut data =
            to_value(&self.0).expect("could not serialize staff member");
        let obj = data.as_object_mut().unwrap();
        obj.insert("is_janitor".into(), is_janitor.into());
        obj.insert("is_moderator".into(), is_moderator.into());
        obj.insert("is_administrator".into(), is_administrator.into());

        data.serialize(serializer)
    }
}

#[derive(Debug)]
pub struct ReportView {
    report: Report,
    post_uri: String,
}

impl ReportView {
    fn new(report_id: i32, db: &Database) -> Result<ReportView> {
        let report = db.report(report_id)?;
        let post_uri = db.post(report.post_id)?.uri();
        Ok(ReportView { report, post_uri })
    }
}

impl Serialize for ReportView {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let time_stamp = self.report.time_stamp.format("%F %R").to_string();
        let uri = self.post_uri.clone();

        let mut data =
            to_value(&self.report).expect("could not serialize report");

        let obj = data.as_object_mut().unwrap();
        obj.insert("time_stamp".into(), JsonValue::String(time_stamp));
        obj.insert("post_uri".into(), JsonValue::String(uri));

        data.serialize(serializer)
    }
}

#[derive(Debug)]
pub struct UserView {
    user: User,
    post_count: u32,
}

impl Serialize for UserView {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ban_expires = self
            .user
            .ban_expires
            .as_ref()
            .map(|time| time.format("%F %R").to_string());
        let hash = self.user.hash.split('$').last().unwrap().to_string();

        let mut data = to_value(&self.user).expect("could not serialize user");

        let obj = data.as_object_mut().unwrap();
        obj.insert("hash".into(), JsonValue::String(hash));
        obj.insert("post_count".into(), JsonValue::from(self.post_count));

        if let Some(ban_expires) = ban_expires {
            obj.insert("ban_expires".into(), JsonValue::String(ban_expires));
        }

        data.serialize(serializer)
    }
}

#[derive(Debug, Serialize)]
pub struct OverviewPage {
    page_info: PageInfo,
    staff: StaffView,
    reports: Vec<ReportView>,
    boards: Vec<Board>,
    users: Vec<UserView>,
}

impl OverviewPage {
    pub fn new<S>(
        user_name: S,
        db: &Database,
        options: &UserOptions,
    ) -> Result<OverviewPage>
    where
        S: AsRef<str>,
    {
        let users = db
            .all_users()?
            .into_iter()
            .map(|user| {
                Ok(UserView {
                    post_count: db.user_post_count(user.id)?,
                    user,
                })
            })
            .collect::<Result<_>>()?;

        Ok(OverviewPage {
            page_info: PageInfo::new("Overview", options),
            staff: StaffView(db.staff(user_name)?),
            reports: db
                .all_reports()?
                .into_iter()
                .map(|report| ReportView::new(report.id, db))
                .collect::<Result<_>>()?,
            boards: db.all_boards()?,
            users,
        })
    }
}

impl_template_responder!(OverviewPage, "pages/staff/overview");

#[derive(Debug, Serialize)]
pub struct LoginPage {
    page_info: PageInfo,
}

impl LoginPage {
    pub fn new(options: &UserOptions) -> Result<LoginPage> {
        Ok(LoginPage {
            page_info: PageInfo::new("Login", options),
        })
    }
}

impl_template_responder!(LoginPage, "pages/staff/login");

#[derive(Debug, Serialize)]
pub struct HistoryPage {
    page_info: PageInfo,
}

impl HistoryPage {
    pub fn new(options: &UserOptions) -> Result<HistoryPage> {
        Ok(HistoryPage {
            page_info: PageInfo::new("Moderation History", options),
        })
    }
}

impl_template_responder!(HistoryPage, "pages/staff/log");
