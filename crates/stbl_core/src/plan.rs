//! Deterministic build plan construction (no execution).

use crate::model::{BuildPlan, BuildTask, ContentId, Project, TaskId, TaskKind};
use blake3::{Hash, Hasher};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub fn build_plan(_project: &Project) -> BuildPlan {
    let config_hash = hash_config(_project);
    let mut tasks = Vec::new();
    let mut edges = Vec::new();

    let published_pages = published_pages_by_path(_project);
    let mut page_tasks: HashMap<_, _> = HashMap::new();
    for page in &published_pages {
        let kind = TaskKind::RenderPage { page: page.id };
        let id = task_id(&kind, &[page.content_hash], config_hash);
        let task = BuildTask {
            id,
            kind,
            inputs: vec![ContentId::Doc(page.id)],
            outputs: Vec::new(),
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
        tasks.push(BuildTask {
            id,
            kind,
            inputs,
            outputs: Vec::new(),
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
        let inputs = pages
            .iter()
            .map(|page| ContentId::Doc(page.id))
            .collect();
        tasks.push(BuildTask {
            id,
            kind,
            inputs,
            outputs: Vec::new(),
        });
        tag_tasks.insert(tag.clone(), id);

        for page in pages.iter().copied() {
            if let Some(page_task) = page_tasks.get(&page.id).copied() {
                edges.push((page_task, id));
            }
        }
    }

    let mut tag_hashes: Vec<Hash> = tag_map.keys().map(|tag| blake3::hash(tag.as_bytes())).collect();
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
        outputs: Vec::new(),
    });
    for tag_task in tag_tasks.values() {
        edges.push((*tag_task, tags_index_id));
    }

    let mut frontpage_hashes: Vec<Hash> =
        published_pages.iter().map(|page| page.content_hash).collect();
    frontpage_hashes.sort_by_key(|hash| hash.as_bytes().to_vec());
    let front_kind = TaskKind::RenderFrontPage;
    let front_id = task_id(&front_kind, &frontpage_hashes, config_hash);
    tasks.push(BuildTask {
        id: front_id,
        kind: front_kind,
        inputs: published_pages
            .iter()
            .map(|page| ContentId::Doc(page.id))
            .collect(),
        outputs: Vec::new(),
    });
    for page in &published_pages {
        if let Some(page_task) = page_tasks.get(&page.id).copied() {
            edges.push((page_task, front_id));
        }
    }

    let rss_kind = TaskKind::GenerateRss;
    let rss_id = task_id(&rss_kind, &frontpage_hashes, config_hash);
    tasks.push(BuildTask {
        id: rss_id,
        kind: rss_kind,
        inputs: published_pages
            .iter()
            .map(|page| ContentId::Doc(page.id))
            .collect(),
        outputs: Vec::new(),
    });
    for page in &published_pages {
        if let Some(page_task) = page_tasks.get(&page.id).copied() {
            edges.push((page_task, rss_id));
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
            .chain(series_sorted.iter().map(|series| ContentId::Series(series.id)))
            .chain(tag_map.keys().map(|tag| ContentId::Tag(tag.clone())))
            .collect(),
        outputs: Vec::new(),
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
    edges.push((front_id, sitemap_id));

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
    let encoded = serde_json::to_vec(&project.config)
        .expect("site config should serialize");
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
