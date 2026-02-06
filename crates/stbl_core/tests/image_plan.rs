use std::collections::BTreeMap;

use stbl_core::assets::AssetSourceId;
use stbl_core::media::{ImagePlanInput, plan_image_tasks};
use stbl_core::model::ImageFormatMode;

#[test]
fn image_plan_orders_tasks_and_paths() {
    let mut sources = BTreeMap::new();
    sources.insert("images/a.jpg".to_string(), AssetSourceId("a".to_string()));
    sources.insert("images/b.jpg".to_string(), AssetSourceId("b".to_string()));
    let mut hashes = BTreeMap::new();
    hashes.insert("images/a.jpg".to_string(), blake3::hash(b"a"));
    hashes.insert("images/b.jpg".to_string(), blake3::hash(b"b"));
    let mut alpha = BTreeMap::new();
    alpha.insert("images/a.jpg".to_string(), false);
    alpha.insert("images/b.jpg".to_string(), false);
    let mut dimensions = BTreeMap::new();
    dimensions.insert(
        "images/a.jpg".to_string(),
        stbl_core::media::MediaDimensions {
            width: 1920,
            height: 1080,
        },
    );
    dimensions.insert(
        "images/b.jpg".to_string(),
        stbl_core::media::MediaDimensions {
            width: 1920,
            height: 1080,
        },
    );
    let input = ImagePlanInput {
        sources,
        hashes,
        alpha,
        dimensions,
    };
    let tasks = plan_image_tasks(
        &input,
        &[1280, 480],
        90,
        ImageFormatMode::Normal,
    );
    let out_paths = tasks
        .iter()
        .map(|task| task.outputs[0].path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    for prefix in ["a", "b"] {
        expected.push(format!("artifacts/images/{prefix}.jpg"));
        for width in [480, 1280] {
            if cfg!(feature = "avif") {
                expected.push(format!("artifacts/images/_scale_{width}/{prefix}.avif"));
            }
            expected.push(format!("artifacts/images/_scale_{width}/{prefix}.webp"));
            expected.push(format!("artifacts/images/_scale_{width}/{prefix}.jpg"));
        }
    }
    assert_eq!(out_paths, expected);
}

#[test]
fn fast_mode_skips_avif_outputs() {
    let mut sources = BTreeMap::new();
    sources.insert("images/a.jpg".to_string(), AssetSourceId("a".to_string()));
    let mut hashes = BTreeMap::new();
    hashes.insert("images/a.jpg".to_string(), blake3::hash(b"a"));
    let mut alpha = BTreeMap::new();
    alpha.insert("images/a.jpg".to_string(), false);
    let mut dimensions = BTreeMap::new();
    dimensions.insert(
        "images/a.jpg".to_string(),
        stbl_core::media::MediaDimensions {
            width: 1920,
            height: 1080,
        },
    );
    let input = ImagePlanInput {
        sources,
        hashes,
        alpha,
        dimensions,
    };
    let tasks = plan_image_tasks(
        &input,
        &[480],
        90,
        ImageFormatMode::Fast,
    );
    let out_paths = tasks
        .iter()
        .flat_map(|task| task.outputs.iter())
        .map(|output| output.path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert!(out_paths.iter().any(|path| path.ends_with(".webp")));
    assert!(out_paths.iter().any(|path| path.ends_with(".jpg")));
    assert!(!out_paths.iter().any(|path| path.ends_with(".avif")));
}
