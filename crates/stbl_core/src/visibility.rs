use crate::header::TemplateId;
use crate::model::Page;

pub fn is_published_page(page: &Page) -> bool {
    page.header.is_published
}

pub fn is_blog_index_excluded(page: &Page, source_page_id: Option<crate::model::DocId>) -> bool {
    if let Some(source_id) = source_page_id {
        if page.id == source_id {
            return true;
        }
    }
    if !is_published_page(page) {
        return true;
    }
    if page.header.exclude_from_blog {
        return true;
    }
    matches!(
        page.header.template,
        Some(TemplateId::BlogIndex) | Some(TemplateId::Info)
    )
}
