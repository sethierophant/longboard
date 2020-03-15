//! Views for staff pages.

use serde::{Serialize, Serializer};

use serde_json::value::{to_value, Value as JsonValue};

use crate::models::{Database, Report, Staff};
use crate::Result;

use crate::impl_template_responder;

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
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let time_stamp = self.report.time_stamp.format("%F %R").to_string();
        let uri = self.post_uri.clone();

        let mut data = to_value(&self.report).expect("could not serialize report");

        let obj = data.as_object_mut().unwrap();
        obj.insert("time_stamp".into(), JsonValue::String(time_stamp));
        obj.insert("post_uri".into(), JsonValue::String(uri));

        data.serialize(serializer)
    }
}

#[derive(Debug, Serialize)]
pub struct OverviewPage {
    user: Staff,
    reports: Vec<ReportView>,
}

impl OverviewPage {
    pub fn new<S>(user_name: S, db: &Database) -> Result<OverviewPage>
    where
        S: AsRef<str>,
    {
        Ok(OverviewPage {
            user: db.staff(user_name)?,
            reports: db
                .all_reports()?
                .into_iter()
                .map(|report| ReportView::new(report.id, db))
                .collect::<Result<_>>()?,
        })
    }
}

impl_template_responder!(OverviewPage, "pages/staff/overview");

#[derive(Debug, Serialize)]
pub struct LoginPage;

impl LoginPage {
    pub fn new() -> Result<LoginPage> {
        Ok(LoginPage)
    }
}

impl_template_responder!(LoginPage, "pages/staff/login");

#[derive(Debug, Serialize)]
pub struct LogPage;

impl LogPage {
    pub fn new() -> Result<LogPage> {
        Ok(LogPage)
    }
}

impl_template_responder!(LogPage, "pages/staff/log");
