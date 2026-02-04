use stbl_core::assets::{AssetIndex, AssetRelPath, AssetSourceId, ResolvedAsset, plan_assets};

#[test]
fn plan_assets_orders_by_rel_path() {
    let assets = AssetIndex {
        assets: vec![
            ResolvedAsset {
                rel: AssetRelPath("b.css".to_string()),
                source: AssetSourceId("b".to_string()),
                content_hash: "bbbbbbbbbbbbbbbb".to_string(),
            },
            ResolvedAsset {
                rel: AssetRelPath("a.css".to_string()),
                source: AssetSourceId("a".to_string()),
                content_hash: "aaaaaaaaaaaaaaaa".to_string(),
            },
        ],
    };
    let config_hash = *blake3::hash(b"config").as_bytes();
    let (tasks, _manifest) = plan_assets(&assets, false, config_hash);
    assert_eq!(tasks.len(), 2);

    match &tasks[0].kind {
        stbl_core::model::TaskKind::CopyAsset { rel, .. } => {
            assert_eq!(rel.0, "a.css");
        }
        _ => panic!("expected copy asset task"),
    }
    match &tasks[1].kind {
        stbl_core::model::TaskKind::CopyAsset { rel, .. } => {
            assert_eq!(rel.0, "b.css");
        }
        _ => panic!("expected copy asset task"),
    }
}

#[test]
fn cache_busting_disabled_keeps_clean_paths() {
    let assets = AssetIndex {
        assets: vec![ResolvedAsset {
            rel: AssetRelPath("css/common.css".to_string()),
            source: AssetSourceId("src".to_string()),
            content_hash: "abcdef0123456789".to_string(),
        }],
    };
    let (tasks, _manifest) = plan_assets(&assets, false, *blake3::hash(b"config").as_bytes());
    match &tasks[0].kind {
        stbl_core::model::TaskKind::CopyAsset { out_rel, .. } => {
            assert_eq!(out_rel, "artifacts/css/common.css");
        }
        _ => panic!("expected copy asset task"),
    }
}

#[test]
fn cache_busting_enabled_adds_hash() {
    let assets = AssetIndex {
        assets: vec![ResolvedAsset {
            rel: AssetRelPath("css/common.css".to_string()),
            source: AssetSourceId("src".to_string()),
            content_hash: "3a9f12c8deadbeef".to_string(),
        }],
    };
    let (tasks, _manifest) = plan_assets(&assets, true, *blake3::hash(b"config").as_bytes());
    match &tasks[0].kind {
        stbl_core::model::TaskKind::CopyAsset { out_rel, .. } => {
            assert_eq!(out_rel, "artifacts/css/common.3a9f12c8.css");
        }
        _ => panic!("expected copy asset task"),
    }
}

#[test]
fn cache_busting_is_deterministic() {
    let assets = AssetIndex {
        assets: vec![ResolvedAsset {
            rel: AssetRelPath("css/common.css".to_string()),
            source: AssetSourceId("src".to_string()),
            content_hash: "3a9f12c8deadbeef".to_string(),
        }],
    };
    let (tasks_first, _manifest_first) =
        plan_assets(&assets, true, *blake3::hash(b"config").as_bytes());
    let (tasks_second, _manifest_second) =
        plan_assets(&assets, true, *blake3::hash(b"config").as_bytes());
    let out_first = match &tasks_first[0].kind {
        stbl_core::model::TaskKind::CopyAsset { out_rel, .. } => out_rel.clone(),
        _ => panic!("expected copy asset task"),
    };
    let out_second = match &tasks_second[0].kind {
        stbl_core::model::TaskKind::CopyAsset { out_rel, .. } => out_rel.clone(),
        _ => panic!("expected copy asset task"),
    };
    assert_eq!(out_first, out_second);
}
