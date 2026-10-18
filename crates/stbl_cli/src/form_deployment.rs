//! Add deployable form files to the ordinary asset plan before execution.
use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use stbl_core::assets::{AssetIndex, AssetRelPath, AssetSourceId, ResolvedAsset};
use stbl_core::forms::{Form, FormManifest};
use stbl_core::model::Project;

use crate::assets::{AssetSource, AssetSourceLookup};

pub fn include_form_deployment(
    project: &Project,
    index: &mut AssetIndex,
    lookup: &mut AssetSourceLookup,
) -> Result<()> {
    let mut forms = BTreeMap::new();
    for asset in &index.assets {
        if !stbl_core::forms::is_definition_asset(&asset.rel.0) {
            continue;
        }
        let bytes = match lookup
            .resolve(&asset.source)
            .context("missing form source")?
        {
            AssetSource::File(path) => std::fs::read(path)?,
            AssetSource::Embedded(bytes) => bytes.clone(),
            AssetSource::SiteIcon(_) => bail!("a form definition cannot be a site icon"),
        };
        insert_form(&mut forms, serde_yaml::from_slice(&bytes)?)?;
    }
    let pages = project
        .content
        .pages
        .iter()
        .chain(project.content.series.iter().flat_map(|series| {
            std::iter::once(&series.index).chain(series.parts.iter().map(|part| &part.page))
        }));
    for page in pages.filter(|page| page.header.is_published) {
        for form in
            stbl_core::forms::definitions(&page.body_markdown).map_err(anyhow::Error::msg)?
        {
            insert_form(&mut forms, form)?;
        }
    }
    if forms.is_empty() {
        return Ok(());
    }
    if index
        .assets
        .iter()
        .any(|asset| asset.rel.0.starts_with("stbl-forms/"))
    {
        bail!("stbl-forms/ is reserved for generated form deployment files");
    }
    let manifest = FormManifest { version: 1, forms };
    let binary = receiver_binary()?;
    let binary_hash = blake3::hash(&std::fs::read(&binary)?).to_hex().to_string();
    add_asset(
        index,
        lookup,
        "stbl-form-receiver",
        AssetSource::File(binary),
        binary_hash,
    );
    let manifest_bytes = serde_yaml::to_string(&manifest)?.into_bytes();
    add_bytes(index, lookup, "forms.yaml", manifest_bytes);
    let mut example = serde_yaml::from_str::<serde_yaml::Value>(include_str!(
        "form_deployment/receiver.yaml.example"
    ))?;
    let endpoints = example["sites"]["example"]["forms"]
        .as_mapping_mut()
        .expect("example forms");
    let prototype = endpoints.values().next().expect("example endpoint").clone();
    endpoints.clear();
    for action in manifest.forms.keys() {
        if action.starts_with('/') {
            endpoints.insert(action.clone().into(), prototype.clone());
        }
    }
    add_bytes(
        index,
        lookup,
        "receiver.yaml.example",
        serde_yaml::to_string(&example)?.into_bytes(),
    );
    add_bytes(
        index,
        lookup,
        "nginx-http.conf.example",
        include_bytes!("form_deployment/nginx-http.conf.example").to_vec(),
    );
    add_bytes(
        index,
        lookup,
        "nginx.conf.example",
        include_bytes!("form_deployment/nginx.conf.example").to_vec(),
    );
    add_bytes(
        index,
        lookup,
        "README.md",
        include_bytes!("form_deployment/README.md").to_vec(),
    );
    Ok(())
}

fn insert_form(forms: &mut BTreeMap<String, Form>, form: Form) -> Result<()> {
    stbl_core::forms::render_definition(&form).map_err(anyhow::Error::msg)?;
    if let Some(previous) = forms.get(&form.action) {
        let schema = |form: &Form| {
            form.fields
                .iter()
                .map(|field| (field.name.clone(), (field.kind, field.required)))
                .collect::<BTreeMap<_, _>>()
        };
        if schema(previous) != schema(&form) {
            bail!(
                "forms sharing action {} have conflicting validation rules",
                form.action
            );
        }
    } else {
        forms.insert(form.action.clone(), form);
    }
    Ok(())
}

fn receiver_binary() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = executable.parent().context("executable has no directory")?;
    let mut path = directory.join("stbl-form-receiver");
    if !path.is_file() && directory.file_name().is_some_and(|name| name == "deps") {
        path = directory
            .parent()
            .context("test executable has no parent")?
            .join("stbl-form-receiver");
    }
    if !path.is_file() {
        bail!(
            "form deployment requires stbl-form-receiver alongside stbl_cli; build both with cargo build -p stbl_cli (or cargo build --release -p stbl_cli)"
        );
    }
    Ok(path)
}

fn add_bytes(index: &mut AssetIndex, lookup: &mut AssetSourceLookup, name: &str, bytes: Vec<u8>) {
    let hash = blake3::hash(&bytes).to_hex().to_string();
    add_asset(index, lookup, name, AssetSource::Embedded(bytes), hash);
}

fn add_asset(
    index: &mut AssetIndex,
    lookup: &mut AssetSourceLookup,
    name: &str,
    source: AssetSource,
    hash: String,
) {
    let source_id = AssetSourceId(format!("form-deployment:{name}:{hash}"));
    lookup.insert(source_id.clone(), source);
    index.assets.push(ResolvedAsset {
        rel: AssetRelPath(format!("stbl-forms/{name}")),
        source: source_id,
        content_hash: hash,
    });
}
