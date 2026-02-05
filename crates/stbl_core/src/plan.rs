//! Deterministic build plan construction (no execution).

use crate::assets::{AssetIndex, plan_assets};
use crate::blog_index::{
    FeedItem, blog_index_page_logical_key, blog_pagination_settings, collect_blog_feed,
    collect_tag_map, paginate_blog_index,
};
use crate::header::TemplateId;
use crate::media::{ImagePlanInput, VideoPlanInput, plan_image_tasks, plan_video_tasks};
use crate::model::{
    BuildPlan, BuildTask, ContentId, InputFingerprint, OutputArtifact, Project, TaskId, TaskKind,
};
use crate::templates::templates_hash;
use crate::url::{UrlMapper, logical_key_from_source_path};
use blake3::{Hash, Hasher};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PlanHashContext {
    pub render_config_hash: [u8; 32],
    pub templates_hash: [u8; 32],
    pub doc_content_hashes: HashMap<crate::model::DocId, Hash>,
}

pub fn build_plan(
    project: &Project,
    asset_index: &AssetIndex,
    image_plan: &ImagePlanInput,
    video_plan: &VideoPlanInput,
) -> BuildPlan {
    let ctx = build_plan_hash_context(project);
    let mapper = UrlMapper::new(&project.config);
    let mut tasks = Vec::new();
    let mut edges = Vec::new();

    let published_pages = published_pages_by_path(project);
    let feed_pages = published_feed_pages(project);
    let mut page_tasks: HashMap<_, _> = HashMap::new();
    let mut blog_index_pages = Vec::new();
    for page in &published_pages {
        if page.header.template == Some(TemplateId::BlogIndex) {
            blog_index_pages.push(*page);
            continue;
        }
        let kind = TaskKind::RenderPage { page: page.id };
        let logical_key = logical_key_from_source_path(&page.source_path);
        let id = task_id_render_page(&logical_key);
        let inputs_fingerprint = fingerprint_render_page(&id, &ctx, page);
        let outputs = outputs_for_logical_key(&mapper, &logical_key);
        let task = BuildTask {
            id: id.clone(),
            kind,
            inputs_fingerprint,
            inputs: vec![ContentId::Doc(page.id)],
            outputs,
        };
        tasks.push(task);
        page_tasks.insert(page.id, id);
    }

    let mut series_sorted = project.content.series.clone();
    series_sorted.sort_by(|a, b| a.dir_path.cmp(&b.dir_path));
    let mut series_tasks: HashMap<_, _> = HashMap::new();
    for series in &series_sorted {
        let kind = TaskKind::RenderSeries { series: series.id };
        let logical_key = logical_key_from_source_path(&series.dir_path);
        let id = task_id_render_series(&logical_key);
        let inputs_fingerprint = fingerprint_render_series(&id, &ctx, series);
        let mut inputs = vec![ContentId::Series(series.id)];
        if series.index.header.is_published {
            inputs.push(ContentId::Doc(series.index.id));
        }
        for part in &series.parts {
            if part.page.header.is_published {
                inputs.push(ContentId::Doc(part.page.id));
            }
        }
        let outputs = outputs_for_logical_key(&mapper, &logical_key);
        tasks.push(BuildTask {
            id: id.clone(),
            kind,
            inputs_fingerprint,
            inputs,
            outputs,
        });
        series_tasks.insert(series.id, id.clone());

        if let Some(index_task) = page_tasks.get(&series.index.id).cloned() {
            edges.push((index_task, id.clone()));
        }
        for part in &series.parts {
            if let Some(part_task) = page_tasks.get(&part.page.id).cloned() {
                edges.push((part_task, id.clone()));
            }
        }
    }

    let tag_map = collect_tag_map(project);
    let mut tag_tasks = HashMap::new();
    for (tag, items) in &tag_map {
        let kind = TaskKind::RenderTagIndex { tag: tag.clone() };
        let id = task_id_render_tag(tag);
        let inputs_fingerprint = fingerprint_render_tag_index(&id, &ctx, tag, items);
        let mut input_docs: Vec<_> = items.iter().flat_map(|item| item.input_doc_ids()).collect();
        input_docs.sort_by_key(|doc_id| doc_id.0.as_bytes().to_vec());
        input_docs.dedup_by(|a, b| a.0 == b.0);
        let inputs = input_docs.into_iter().map(ContentId::Doc).collect();
        let outputs = outputs_for_logical_key(&mapper, &format!("tags/{}", tag));
        tasks.push(BuildTask {
            id: id.clone(),
            kind,
            inputs_fingerprint,
            inputs,
            outputs,
        });
        tag_tasks.insert(tag.clone(), id.clone());

        for doc_id in items.iter().flat_map(|item| item.input_doc_ids()) {
            if let Some(page_task) = page_tasks.get(&doc_id).cloned() {
                edges.push((page_task, id.clone()));
            }
        }
    }

    let tags_index_kind = TaskKind::RenderTagsIndex;
    let tags_index_id = task_id_render_tags_index();
    let tags_index_fingerprint = fingerprint_render_tags_index(&tags_index_id, &ctx, &tag_map);
    tasks.push(BuildTask {
        id: tags_index_id.clone(),
        kind: tags_index_kind,
        inputs_fingerprint: tags_index_fingerprint,
        inputs: tag_map
            .keys()
            .map(|tag| ContentId::Tag(tag.clone()))
            .collect(),
        outputs: outputs_for_logical_key(&mapper, "tags"),
    });
    for tag_task in tag_tasks.values() {
        edges.push((tag_task.clone(), tags_index_id.clone()));
    }

    let frontpage_items: Vec<&crate::model::Page> = published_pages.iter().copied().collect();
    let mut frontpage_task_id: Option<TaskId> = None;
    let has_blog_frontpage = blog_index_pages
        .iter()
        .any(|page| logical_key_from_source_path(&page.source_path).as_str() == "index");
    if !has_blog_frontpage {
        let front_kind = TaskKind::RenderFrontPage;
        let front_id = task_id_render_frontpage();
        let front_fingerprint = fingerprint_render_frontpage(&front_id, &ctx, &frontpage_items);
        tasks.push(BuildTask {
            id: front_id.clone(),
            kind: front_kind,
            inputs_fingerprint: front_fingerprint,
            inputs: published_pages
                .iter()
                .map(|page| ContentId::Doc(page.id))
                .collect(),
            outputs: outputs_for_logical_key(&mapper, "index"),
        });
        for page in &published_pages {
            if let Some(page_task) = page_tasks.get(&page.id).cloned() {
                edges.push((page_task, front_id.clone()));
            }
        }
        frontpage_task_id = Some(front_id);
    }

    if rss_enabled(project) {
        let rss_kind = TaskKind::GenerateRss;
        let rss_id = task_id_generate_rss();
        let rss_fingerprint = fingerprint_generate_rss(&rss_id, &ctx, &feed_pages);
        tasks.push(BuildTask {
            id: rss_id.clone(),
            kind: rss_kind,
            inputs_fingerprint: rss_fingerprint,
            inputs: published_pages
                .iter()
                .map(|page| ContentId::Doc(page.id))
                .collect(),
            outputs: vec![OutputArtifact {
                path: PathBuf::from("rss.xml"),
            }],
        });
        for page in &published_pages {
            if let Some(page_task) = page_tasks.get(&page.id).cloned() {
                edges.push((page_task, rss_id.clone()));
            }
        }
    }

    let sitemap_kind = TaskKind::GenerateSitemap;
    let sitemap_id = task_id_generate_sitemap();
    let sitemap_fingerprint =
        fingerprint_generate_sitemap(&sitemap_id, &ctx, &feed_pages, &tag_map);
    tasks.push(BuildTask {
        id: sitemap_id.clone(),
        kind: sitemap_kind,
        inputs_fingerprint: sitemap_fingerprint,
        inputs: published_pages
            .iter()
            .map(|page| ContentId::Doc(page.id))
            .chain(
                series_sorted
                    .iter()
                    .map(|series| ContentId::Series(series.id)),
            )
            .chain(tag_map.keys().map(|tag| ContentId::Tag(tag.clone())))
            .collect(),
        outputs: vec![OutputArtifact {
            path: PathBuf::from("sitemap.xml"),
        }],
    });
    for page in &published_pages {
        if let Some(page_task) = page_tasks.get(&page.id).cloned() {
            edges.push((page_task, sitemap_id.clone()));
        }
    }
    for series_task in series_tasks.values() {
        edges.push((series_task.clone(), sitemap_id.clone()));
    }
    for tag_task in tag_tasks.values() {
        edges.push((tag_task.clone(), sitemap_id.clone()));
    }
    edges.push((tags_index_id.clone(), sitemap_id.clone()));
    if let Some(front_id) = frontpage_task_id {
        edges.push((front_id, sitemap_id.clone()));
    }

    for page in &blog_index_pages {
        let feed_items = collect_blog_feed(project, page.id);
        let base_key = logical_key_from_source_path(&page.source_path);
        let pagination = blog_pagination_settings(project);
        let page_ranges = paginate_blog_index(pagination, &base_key, feed_items.len());
        for page_range in page_ranges {
            let kind = TaskKind::RenderBlogIndex {
                source_page: page.id,
                page_no: page_range.page_no,
            };
            let page_key = blog_index_page_logical_key(&base_key, page_range.page_no);
            let id = task_id_render_blog_index(&base_key, page_range.page_no);
            let inputs_fingerprint =
                fingerprint_render_blog_index(&id, &ctx, page, &feed_items, &page_range);
            let mut inputs: Vec<ContentId> = feed_items
                .iter()
                .flat_map(|item| item.input_doc_ids())
                .map(ContentId::Doc)
                .collect();
            inputs.push(ContentId::Doc(page.id));
            let outputs = outputs_for_logical_key(&mapper, &page_key);
            tasks.push(BuildTask {
                id,
                kind,
                inputs_fingerprint,
                inputs,
                outputs,
            });
        }
    }

    let (asset_tasks, _manifest) =
        plan_assets(asset_index, project.config.assets.cache_busting, ctx.render_config_hash);
    tasks.extend(asset_tasks);

    let image_tasks = plan_image_tasks(
        image_plan,
        &project.config.media.images.widths,
        project.config.media.images.quality,
        project.config.media.images.format_mode,
        ctx.render_config_hash,
    );
    tasks.extend(image_tasks);

    let video_tasks = plan_video_tasks(
        video_plan,
        &project.config.media.video.heights,
        project.config.media.video.poster_time_sec,
        ctx.render_config_hash,
    );
    tasks.extend(video_tasks);

    let theme_vars = crate::model::ThemeVars {
        max_body_width: project.config.theme.max_body_width.clone(),
        desktop_min: project.config.theme.breakpoints.desktop_min.clone(),
        wide_min: project.config.theme.breakpoints.wide_min.clone(),
    };
    let vars_out_rel = "artifacts/css/vars.css".to_string();
    let vars_kind = TaskKind::GenerateVarsCss {
        vars: theme_vars,
        out_rel: vars_out_rel.clone(),
    };
    let vars_id = task_id_generate_vars_css(&vars_out_rel);
    let vars_fingerprint = fingerprint_generate_vars_css(&vars_id, &ctx, &vars_kind);
    tasks.push(BuildTask {
        id: vars_id,
        kind: vars_kind,
        inputs_fingerprint: vars_fingerprint,
        inputs: Vec::new(),
        outputs: vec![OutputArtifact {
            path: PathBuf::from(vars_out_rel),
        }],
    });

    tasks.sort_by_key(|task| task.id.0.as_bytes().to_vec());
    edges.sort_by_key(|(from, to)| (from.0.as_bytes().to_vec(), to.0.as_bytes().to_vec()));

    BuildPlan { tasks, edges }
}

fn build_plan_hash_context(project: &Project) -> PlanHashContext {
    let render_config_hash = hash_render_config(&project.config);
    let templates_hash = templates_hash();
    let mut doc_content_hashes = HashMap::new();
    for page in &project.content.pages {
        doc_content_hashes.insert(page.id, page.content_hash);
    }
    for series in &project.content.series {
        doc_content_hashes.insert(series.index.id, series.index.content_hash);
        for part in &series.parts {
            doc_content_hashes.insert(part.page.id, part.page.content_hash);
        }
    }
    PlanHashContext {
        render_config_hash,
        templates_hash,
        doc_content_hashes,
    }
}

#[derive(Serialize)]
struct RenderConfigHash<'a> {
    site: &'a crate::model::SiteMeta,
    banner: &'a Option<crate::model::BannerConfig>,
    menu: &'a Vec<crate::model::MenuItem>,
    nav: &'a Vec<crate::model::NavItem>,
    theme: &'a crate::model::ThemeConfig,
    assets: &'a crate::model::AssetsConfig,
    media: &'a crate::model::MediaConfig,
    footer: &'a crate::model::FooterConfig,
    people: &'a Option<crate::model::PeopleConfig>,
    blog: &'a Option<crate::model::BlogConfig>,
    system: &'a Option<crate::model::SystemConfig>,
    rss: &'a Option<crate::model::RssConfig>,
    seo: &'a Option<crate::model::SeoConfig>,
    comments: &'a Option<serde_yaml::Value>,
    chroma: &'a Option<serde_yaml::Value>,
    plyr: &'a Option<serde_yaml::Value>,
}

pub fn hash_render_config(cfg: &crate::model::SiteConfig) -> [u8; 32] {
    let input = RenderConfigHash {
        site: &cfg.site,
        banner: &cfg.banner,
        menu: &cfg.menu,
        nav: &cfg.nav,
        theme: &cfg.theme,
        assets: &cfg.assets,
        media: &cfg.media,
        footer: &cfg.footer,
        people: &cfg.people,
        blog: &cfg.blog,
        system: &cfg.system,
        rss: &cfg.rss,
        seo: &cfg.seo,
        comments: &cfg.comments,
        chroma: &cfg.chroma,
        plyr: &cfg.plyr,
    };
    let encoded = serde_json::to_vec(&input).expect("render config should serialize");
    *blake3::hash(&encoded).as_bytes()
}

fn published_pages_by_path(project: &Project) -> Vec<&crate::model::Page> {
    let mut pages: Vec<_> = project
        .content
        .pages
        .iter()
        .filter(|page| crate::visibility::is_published_page(page))
        .collect();
    pages.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    pages
}

fn published_feed_pages(project: &Project) -> Vec<&crate::model::Page> {
    let mut pages = Vec::new();
    for page in &project.content.pages {
        if page.header.template == Some(TemplateId::BlogIndex) {
            continue;
        }
        if crate::visibility::is_published_page(page) {
            pages.push(page);
        }
    }
    for series in &project.content.series {
        if crate::visibility::is_published_page(&series.index) {
            pages.push(&series.index);
        }
        for part in &series.parts {
            if crate::visibility::is_published_page(&part.page) {
                pages.push(&part.page);
            }
        }
    }
    pages
}

fn task_id_render_page(logical_key: &str) -> TaskId {
    TaskId::new("render_page", &[logical_key])
}

fn task_id_render_blog_index(base_key: &str, page_no: u32) -> TaskId {
    let page = format!("page={page_no}");
    TaskId::new("render_blog_index", &[base_key, &page])
}

fn task_id_render_series(logical_key: &str) -> TaskId {
    TaskId::new("render_series", &[logical_key])
}

fn task_id_render_tag(tag: &str) -> TaskId {
    TaskId::new("render_tag", &[tag])
}

fn task_id_render_tags_index() -> TaskId {
    TaskId::new("render_tags_index", &[])
}

fn task_id_render_frontpage() -> TaskId {
    TaskId::new("render_frontpage", &[])
}

fn task_id_generate_rss() -> TaskId {
    TaskId::new("generate_rss", &[])
}

fn task_id_generate_sitemap() -> TaskId {
    TaskId::new("generate_sitemap", &[])
}

fn task_id_generate_vars_css(out_rel: &str) -> TaskId {
    TaskId::new("generate_vars_css", &[out_rel])
}

fn fingerprint_render_page(task_id: &TaskId, ctx: &PlanHashContext, page: &crate::model::Page) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderPage");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    add_hash(&mut hasher, &page.content_hash);
    finish_fingerprint(hasher)
}

fn fingerprint_render_series(task_id: &TaskId, ctx: &PlanHashContext, series: &crate::model::Series) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderSeries");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    let mut hashes = Vec::new();
    if series.index.header.is_published {
        hashes.push(series.index.content_hash);
    }
    for part in &series.parts {
        if part.page.header.is_published {
            hashes.push(part.page.content_hash);
        }
    }
    hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    add_hash_list(&mut hasher, &hashes);
    finish_fingerprint(hasher)
}

fn fingerprint_render_tag_index(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    tag: &str,
    items: &[FeedItem],
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderTagIndex");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    add_str(&mut hasher, tag);
    add_hash(&mut hasher, &hash_feed_items(items));
    finish_fingerprint(hasher)
}

fn fingerprint_render_tags_index(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    tag_map: &std::collections::BTreeMap<String, Vec<FeedItem>>,
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderTagsIndex");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    let tags = tag_map.keys().cloned().collect::<Vec<_>>();
    add_hash(&mut hasher, &hash_tag_list(&tags));
    finish_fingerprint(hasher)
}

fn fingerprint_render_frontpage(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    pages: &[&crate::model::Page],
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderFrontPage");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    let mut hashes = pages.iter().map(|page| page.content_hash).collect::<Vec<_>>();
    hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    add_hash_list(&mut hasher, &hashes);
    finish_fingerprint(hasher)
}

fn fingerprint_render_blog_index(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    source_page: &crate::model::Page,
    items: &[FeedItem],
    page_range: &crate::blog_index::BlogIndexPageRange,
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "RenderBlogIndex");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    add_hash_bytes(&mut hasher, &ctx.templates_hash);
    add_hash(&mut hasher, &source_page.content_hash);
    add_u64(&mut hasher, page_range.page_no as u64);
    add_hash(&mut hasher, &hash_feed_items(items));
    finish_fingerprint(hasher)
}

fn fingerprint_generate_rss(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    pages: &[&crate::model::Page],
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "GenerateRss");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    let mut hashes = pages.iter().map(|page| page.content_hash).collect::<Vec<_>>();
    hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    add_hash_list(&mut hasher, &hashes);
    finish_fingerprint(hasher)
}

fn fingerprint_generate_sitemap(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    pages: &[&crate::model::Page],
    tag_map: &std::collections::BTreeMap<String, Vec<FeedItem>>,
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "GenerateSitemap");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    let mut hashes = pages.iter().map(|page| page.content_hash).collect::<Vec<_>>();
    hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    add_hash_list(&mut hasher, &hashes);
    let tags = tag_map.keys().cloned().collect::<Vec<_>>();
    add_hash(&mut hasher, &hash_tag_list(&tags));
    finish_fingerprint(hasher)
}

fn fingerprint_generate_vars_css(
    task_id: &TaskId,
    ctx: &PlanHashContext,
    kind: &TaskKind,
) -> InputFingerprint {
    let mut hasher = task_fingerprint_hasher(task_id, "GenerateVarsCss");
    add_hash_bytes(&mut hasher, &ctx.render_config_hash);
    if let TaskKind::GenerateVarsCss { vars, out_rel } = kind {
        add_str(&mut hasher, &vars.max_body_width);
        add_str(&mut hasher, &vars.desktop_min);
        add_str(&mut hasher, &vars.wide_min);
        add_str(&mut hasher, out_rel);
    }
    finish_fingerprint(hasher)
}

fn hash_feed_items(items: &[FeedItem]) -> Hash {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.feed.v1");
    add_u64(&mut hasher, items.len() as u64);
    for item in items {
        add_str(&mut hasher, item.tie_key());
        add_i64(&mut hasher, item.sort_date());
        let hashes = item.input_hashes();
        add_u64(&mut hasher, hashes.len() as u64);
        for hash in hashes {
            add_hash(&mut hasher, &hash);
        }
    }
    hasher.finalize()
}

fn hash_tag_list(tags: &[String]) -> Hash {
    let mut tags = tags.to_vec();
    tags.sort();
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.tags.v1");
    add_u64(&mut hasher, tags.len() as u64);
    for tag in tags {
        add_str(&mut hasher, &tag);
    }
    hasher.finalize()
}

fn task_fingerprint_hasher(task_id: &TaskId, kind_label: &str) -> Hasher {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, kind_label);
    hasher
}

fn finish_fingerprint(hasher: Hasher) -> InputFingerprint {
    InputFingerprint(*hasher.finalize().as_bytes())
}

fn add_hash(hasher: &mut Hasher, hash: &Hash) {
    hasher.update(hash.as_bytes());
}

fn add_hash_list(hasher: &mut Hasher, hashes: &[Hash]) {
    add_u64(hasher, hashes.len() as u64);
    for hash in hashes {
        add_hash(hasher, hash);
    }
}

fn add_hash_bytes(hasher: &mut Hasher, hash: &[u8; 32]) {
    hasher.update(hash);
}

fn add_str(hasher: &mut Hasher, value: &str) {
    add_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn add_i64(hasher: &mut Hasher, value: i64) {
    hasher.update(&value.to_le_bytes());
}

fn add_u64(hasher: &mut Hasher, value: u64) {
    hasher.update(&value.to_le_bytes());
}

fn outputs_for_logical_key(mapper: &UrlMapper, logical_key: &str) -> Vec<OutputArtifact> {
    let mapping = mapper.map(logical_key);
    let mut outputs = vec![OutputArtifact {
        path: mapping.primary_output,
    }];
    if let Some(fallback) = mapping.fallback {
        outputs.push(OutputArtifact {
            path: fallback.from,
        });
    }
    outputs
}

fn rss_enabled(project: &Project) -> bool {
    project.config.rss.as_ref().is_some_and(|rss| rss.enabled)
}
