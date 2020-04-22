use serde::Serialize;

use crate::impl_template_responder;
use crate::views::{Context, PageFooter, PageInfo};
use crate::Result;

#[derive(Debug, Serialize)]
pub struct BadRequestPage {
    pub message: String,
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
}

impl BadRequestPage {
    pub fn new<S>(message: S, context: &Context) -> Result<BadRequestPage>
    where
        S: Into<String>,
    {
        Ok(BadRequestPage {
            message: message.into(),
            page_info: PageInfo::new("Error", context),
            page_footer: PageFooter::new(context)?,
        })
    }
}

impl_template_responder!(BadRequestPage, "pages/error/400");

#[derive(Debug, Serialize)]
pub struct SpamDetectedPage {
    pub message: String,
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
}

impl SpamDetectedPage {
    pub fn new<S>(message: S, context: &Context) -> Result<SpamDetectedPage>
    where
        S: Into<String>,
    {
        Ok(SpamDetectedPage {
            message: message.into(),
            page_info: PageInfo::new("Spam Detected", context),
            page_footer: PageFooter::new(context)?,
        })
    }
}

impl_template_responder!(SpamDetectedPage, "pages/error/spam-detected");

#[derive(Debug, Serialize)]
pub struct NotFoundPage {
    pub message: String,
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
}

impl NotFoundPage {
    pub fn new<S>(message: S, context: &Context) -> Result<NotFoundPage>
    where
        S: Into<String>,
    {
        Ok(NotFoundPage {
            message: message.into(),
            page_info: PageInfo::new("Error", context),
            page_footer: PageFooter::new(context)?,
        })
    }
}

impl_template_responder!(NotFoundPage, "pages/error/500");

#[derive(Debug, Serialize)]
pub struct InternalServerErrorPage {
    pub message: String,
    pub page_info: PageInfo,
    pub page_footer: PageFooter,
}

impl InternalServerErrorPage {
    pub fn new<S>(
        message: S,
        context: &Context,
    ) -> Result<InternalServerErrorPage>
    where
        S: Into<String>,
    {
        Ok(InternalServerErrorPage {
            message: message.into(),
            page_info: PageInfo::new("Error", context),
            page_footer: PageFooter::new(context)?,
        })
    }
}

impl_template_responder!(InternalServerErrorPage, "pages/error/500");
