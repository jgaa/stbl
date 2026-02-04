use std::collections::BTreeMap;

use stbl_core::assets::AssetSourceId;
use stbl_core::media::{ImagePlanInput, plan_image_tasks};

#[test]
fn image_plan_orders_tasks_and_paths() {
    let mut sources = BTreeMap::new();
    sources.insert("images/a.jpg".to_string(), AssetSourceId("a".to_string()));
    sources.insert("images/b.jpg".to_string(), AssetSourceId("b".to_string()));
    let mut hashes = BTreeMap::new();
    hashes.insert("images/a.jpg".to_string(), blake3::hash(b"a"));
    hashes.insert("images/b.jpg".to_string(), blake3::hash(b"b"));
    let input = ImagePlanInput { sources, hashes };
    let tasks = plan_image_tasks(&input, &[1280, 480], 90, *blake3::hash(b"config").as_bytes());
    assert_eq!(tasks.len(), 6);

    let out_paths = tasks
        .iter()
        .map(|task| task.outputs[0].path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        out_paths,
        vec![
            "artifacts/images/a.jpg",
            "artifacts/images/_scale_480/a.jpg",
            "artifacts/images/_scale_1280/a.jpg",
            "artifacts/images/b.jpg",
            "artifacts/images/_scale_480/b.jpg",
            "artifacts/images/_scale_1280/b.jpg",
        ]
    );
}
