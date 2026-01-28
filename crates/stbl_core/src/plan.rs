//! Deterministic build plan construction (no execution).

use crate::blog_index::{blog_page_size, collect_blog_feed};
use crate::header::TemplateId;
use crate::model::{BuildPlan, BuildTask, ContentId, OutputArtifact, Project, TaskId, TaskKind};
use crate::url::{UrlMapper, logical_key_from_source_path};
use blake3::{Hash, Hasher};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub fn build_plan(_project: &Project) -> BuildPlan {
    let config_hash = hash_config(_project);
    let mapper = UrlMapper::new(&_project.config);
    let mut tasks = Vec::new();
    let mut edges = Vec::new();

    let published_pages = published_pages_by_path(_project);
    let mut page_tasks: HashMap<_, _> = HashMap::new();
    let mut blog_index_pages = Vec::new();
    for page in &published_pages {
        if page.header.template == Some(TemplateId::BlogIndex) {
            blog_index_pages.push(*page);
            continue;
        }
        let kind = TaskKind::RenderPage { page: page.id };
        let id = task_id(&kind, &[page.content_hash], config_hash);
        let outputs =
            outputs_for_logical_key(&mapper, &logical_key_from_source_path(&page.source_path));
        let task = BuildTask {
            id,
            kind,
            inputs: vec![ContentId::Doc(page.id)],
            outputs,
        };
        tasks.push(task);
        page_tasks.insert(page.id, id);
    }

    let mut series_sorted = _project.content.series.clone();
    series_sorted.sort_by(|a, b| a.dir_path.cmp(&b.dir_path));
    let mut series_tasks: HashMap<_, _> = HashMap::new();
    for series in &series_sorted {
        let kind = TaskKind::RenderSeries { series: series.id };
        let mut input_hashes = Vec::new();
        if series.index.header.is_published {
            input_hashes.push(series.index.content_hash);
        }
        for part in &series.parts {
            if part.page.header.is_published {
                input_hashes.push(part.page.content_hash);
            }
        }
        input_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
        let id = task_id(&kind, &input_hashes, config_hash);
        let mut inputs = vec![ContentId::Series(series.id)];
        if series.index.header.is_published {
            inputs.push(ContentId::Doc(series.index.id));
        }
        for part in &series.parts {
            if part.page.header.is_published {
                inputs.push(ContentId::Doc(part.page.id));
            }
        }
        let outputs =
            outputs_for_logical_key(&mapper, &logical_key_from_source_path(&series.dir_path));
        tasks.push(BuildTask {
            id,
            kind,
            inputs,
            outputs,
        });
        series_tasks.insert(series.id, id);

        if let Some(index_task) = page_tasks.get(&series.index.id).copied() {
            edges.push((index_task, id));
        }
        for part in &series.parts {
            if let Some(part_task) = page_tasks.get(&part.page.id).copied() {
                edges.push((part_task, id));
            }
        }
    }

    let tag_map = collect_tags(&published_pages);
    let mut tag_tasks = HashMap::new();
    for (tag, pages) in &tag_map {
        let kind = TaskKind::RenderTagIndex { tag: tag.clone() };
        let mut input_hashes: Vec<Hash> = pages.iter().map(|page| page.content_hash).collect();
        input_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
        let id = task_id(&kind, &input_hashes, config_hash);
        let inputs = pages.iter().map(|page| ContentId::Doc(page.id)).collect();
        let outputs = outputs_for_logical_key(&mapper, &format!("tags/{}", tag));
        tasks.push(BuildTask {
            id,
            kind,
            inputs,
            outputs,
        });
        tag_tasks.insert(tag.clone(), id);

        for page in pages.iter().copied() {
            if let Some(page_task) = page_tasks.get(&page.id).copied() {
                edges.push((page_task, id));
            }
        }
    }

    let mut tag_hashes: Vec<Hash> = tag_map
        .keys()
        .map(|tag| blake3::hash(tag.as_bytes()))
        .collect();
    tag_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    let tags_index_kind = TaskKind::RenderTagsIndex;
    let tags_index_id = task_id(&tags_index_kind, &tag_hashes, config_hash);
    tasks.push(BuildTask {
        id: tags_index_id,
        kind: tags_index_kind,
        inputs: tag_map
            .keys()
            .map(|tag| ContentId::Tag(tag.clone()))
            .collect(),
        outputs: outputs_for_logical_key(&mapper, "tags"),
    });
    for tag_task in tag_tasks.values() {
        edges.push((*tag_task, tags_index_id));
    }

    let mut frontpage_hashes: Vec<Hash> = published_pages
        .iter()
        .map(|page| page.content_hash)
        .collect();
    frontpage_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    let has_blog_frontpage = blog_index_pages
        .iter()
        .any(|page| logical_key_from_source_path(&page.source_path).as_str() == "index");
    if !has_blog_frontpage {
        let front_kind = TaskKind::RenderFrontPage;
        let front_id = task_id(&front_kind, &frontpage_hashes, config_hash);
        tasks.push(BuildTask {
            id: front_id,
            kind: front_kind,
            inputs: published_pages
                .iter()
                .map(|page| ContentId::Doc(page.id))
                .collect(),
            outputs: outputs_for_logical_key(&mapper, "index"),
        });
        for page in &published_pages {
            if let Some(page_task) = page_tasks.get(&page.id).copied() {
                edges.push((page_task, front_id));
            }
        }
    }

    if rss_enabled(_project) {
        let rss_kind = TaskKind::GenerateRss;
        let rss_id = task_id(&rss_kind, &frontpage_hashes, config_hash);
        tasks.push(BuildTask {
            id: rss_id,
            kind: rss_kind,
            inputs: published_pages
                .iter()
                .map(|page| ContentId::Doc(page.id))
                .collect(),
            outputs: vec![OutputArtifact {
                path: PathBuf::from("rss.xml"),
            }],
        });
        for page in &published_pages {
            if let Some(page_task) = page_tasks.get(&page.id).copied() {
                edges.push((page_task, rss_id));
            }
        }
    }

    let mut sitemap_inputs = frontpage_hashes.clone();
    for series in &series_sorted {
        sitemap_inputs.push(series.id.0);
    }
    for tag in tag_map.keys() {
        sitemap_inputs.push(blake3::hash(tag.as_bytes()));
    }
    sitemap_inputs.sort_by_key(|hash| hash.as_bytes().to_vec());
    let sitemap_kind = TaskKind::GenerateSitemap;
    let sitemap_id = task_id(&sitemap_kind, &sitemap_inputs, config_hash);
    tasks.push(BuildTask {
        id: sitemap_id,
        kind: sitemap_kind,
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
        if let Some(page_task) = page_tasks.get(&page.id).copied() {
            edges.push((page_task, sitemap_id));
        }
    }
    for series_task in series_tasks.values() {
        edges.push((*series_task, sitemap_id));
    }
    for tag_task in tag_tasks.values() {
        edges.push((*tag_task, sitemap_id));
    }
    edges.push((tags_index_id, sitemap_id));
    if !has_blog_frontpage {
        let front_kind = TaskKind::RenderFrontPage;
        let front_id = task_id(&front_kind, &frontpage_hashes, config_hash);
        edges.push((front_id, sitemap_id));
    }

    for page in &blog_index_pages {
        let feed_items = collect_blog_feed(_project, page.id);
        let total_pages = total_pages(feed_items.len(), blog_page_size(_project));
        for page_no in 1..=total_pages {
            let kind = TaskKind::RenderBlogIndex {
                source_page: page.id,
                page_no,
            };
            let mut input_hashes: Vec<Hash> = feed_items
                .iter()
                .flat_map(|item| item.input_hashes())
                .collect();
            input_hashes.push(page.content_hash);
            input_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
            let id = task_id(&kind, &input_hashes, config_hash);
            let mut inputs: Vec<ContentId> = feed_items
                .iter()
                .flat_map(|item| item.input_doc_ids())
                .map(ContentId::Doc)
                .collect();
            inputs.push(ContentId::Doc(page.id));
            let outputs = if page_no == 1 {
                outputs_for_logical_key(&mapper, &logical_key_from_source_path(&page.source_path))
            } else {
                outputs_for_logical_key(&mapper, &format!("page/{}", page_no))
            };
            tasks.push(BuildTask {
                id,
                kind,
                inputs,
                outputs,
            });
        }
    }

    tasks.sort_by_key(|task| task.id.0.as_bytes().to_vec());
    edges.sort_by_key(|(from, to)| (from.0.as_bytes().to_vec(), to.0.as_bytes().to_vec()));

    BuildPlan { tasks, edges }
}

pub fn task_id(kind: &TaskKind, input_hashes: &[Hash], config_hash: Hash) -> TaskId {
    let mut hasher = Hasher::new();
    let kind_key = kind_key(kind);
    add_str(&mut hasher, kind_key);
    add_kind_fields(&mut hasher, kind);
    add_hashes(&mut hasher, input_hashes);
    add_hash(&mut hasher, &config_hash);
    TaskId(hasher.finalize())
}

fn hash_config(project: &Project) -> Hash {
    let encoded = serde_json::to_vec(&project.config).expect("site config should serialize");
    blake3::hash(&encoded)
}

fn published_pages_by_path(project: &Project) -> Vec<&crate::model::Page> {
    let mut pages: Vec<_> = project
        .content
        .pages
        .iter()
        .filter(|page| page.header.is_published)
        .collect();
    pages.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    pages
}

fn collect_tags<'a>(
    pages: &[&'a crate::model::Page],
) -> BTreeMap<String, Vec<&'a crate::model::Page>> {
    let mut tag_map: BTreeMap<String, Vec<&crate::model::Page>> = BTreeMap::new();
    for page in pages {
        for tag in &page.header.tags {
            tag_map.entry(tag.clone()).or_default().push(*page);
        }
    }
    for pages in tag_map.values_mut() {
        pages.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    }
    tag_map
}

fn kind_key(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::RenderPage { .. } => "RenderPage",
        TaskKind::RenderBlogIndex { .. } => "RenderBlogIndex",
        TaskKind::RenderSeries { .. } => "RenderSeries",
        TaskKind::RenderTagIndex { .. } => "RenderTagIndex",
        TaskKind::RenderTagsIndex => "RenderTagsIndex",
        TaskKind::RenderFrontPage => "RenderFrontPage",
        TaskKind::GenerateRss => "GenerateRss",
        TaskKind::GenerateSitemap => "GenerateSitemap",
        TaskKind::CopyAsset { .. } => "CopyAsset",
    }
}

fn add_kind_fields(hasher: &mut Hasher, kind: &TaskKind) {
    match kind {
        TaskKind::RenderPage { page } => add_hash(hasher, &page.0),
        TaskKind::RenderBlogIndex {
            source_page,
            page_no,
        } => {
            add_hash(hasher, &source_page.0);
            add_u64(hasher, *page_no as u64);
        }
        TaskKind::RenderSeries { series } => add_hash(hasher, &series.0),
        TaskKind::RenderTagIndex { tag } => add_str(hasher, tag),
        TaskKind::RenderTagsIndex => {}
        TaskKind::RenderFrontPage => {}
        TaskKind::GenerateRss => {}
        TaskKind::GenerateSitemap => {}
        TaskKind::CopyAsset { rel_path } => add_path(hasher, rel_path),
    }
}

fn add_hashes(hasher: &mut Hasher, hashes: &[Hash]) {
    add_u64(hasher, hashes.len() as u64);
    for hash in hashes {
        add_hash(hasher, hash);
    }
}

fn add_hash(hasher: &mut Hasher, hash: &Hash) {
    hasher.update(hash.as_bytes());
}

fn add_str(hasher: &mut Hasher, value: &str) {
    add_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn add_path(hasher: &mut Hasher, path: &Path) {
    let normalized = path.to_string_lossy().replace('\\', "/");
    add_str(hasher, &normalized);
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

fn total_pages(total_items: usize, page_size: usize) -> u32 {
    let size = page_size.max(1);
    let pages = if total_items == 0 {
        1
    } else {
        (total_items + size - 1) / size
    };
    pages as u32
}
