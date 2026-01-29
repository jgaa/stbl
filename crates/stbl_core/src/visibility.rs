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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::TemplateId;
    use crate::model::DocId;

    fn make_page(id_seed: &str, header: crate::header::Header) -> Page {
        Page {
            id: DocId(blake3::hash(id_seed.as_bytes())),
            source_path: "articles/page.md".to_string(),
            header,
            body_markdown: String::new(),
            url_path: "page".to_string(),
            content_hash: blake3::hash(b"content"),
        }
    }

    #[test]
    fn published_page_is_visible() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let page = make_page("page", header);
        assert!(is_published_page(&page));
        assert!(!is_blog_index_excluded(&page, None));
    }

    #[test]
    fn unpublished_page_is_excluded() {
        let mut header = crate::header::Header::default();
        header.is_published = false;
        let page = make_page("page", header);
        assert!(!is_published_page(&page));
        assert!(is_blog_index_excluded(&page, None));
    }

    #[test]
    fn blog_index_excludes_info_and_blog_index_templates() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.template = Some(TemplateId::Info);
        let info = make_page("info", header);
        assert!(is_blog_index_excluded(&info, None));

        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.template = Some(TemplateId::BlogIndex);
        let blog_index = make_page("blog-index", header);
        assert!(is_blog_index_excluded(&blog_index, None));
    }

    #[test]
    fn blog_index_excludes_source_page() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let page = make_page("page", header);
        assert!(is_blog_index_excluded(&page, Some(page.id)));
    }
}
