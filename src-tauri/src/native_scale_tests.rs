//! Explicit native scale acceptance, excluded from the fast library gate.
//! Run with `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
//! native_scale_tests::file_backed_native_scale -- --ignored --nocapture`.
//! Set IMAGELORE_SCALE_EVIDENCE to a local output directory. No user data root is used.

use crate::{db, models::LibraryFilter, search, semantic, state::AppState};
use rusqlite::params;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicU64, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SEED: u64 = 0x49_6d_61_67_65_4c_6f_72;
const DIMENSIONS: usize = 512;
const COLD: usize = 3;
const WARM: usize = 20;
const PAGE_SIZE: i64 = 240;
const FIXED_TIME: i64 = 1_760_000_000;

fn regular(id: i64) -> bool {
    id % 13 != 0 && id % 17 != 0
}

fn asset_path(id: i64) -> String {
    std::env::var_os("IMAGELORE_SCALE_ASSET_DIR")
        .map(|root| {
            PathBuf::from(root)
                .join(format!("{id}.png"))
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| format!("/synthetic-scale/{id}.png"))
}

fn vector_blob(id: i64, count: i64) -> Vec<u8> {
    // Dense deterministic synthetic vectors. The first component is monotonic,
    // so a unit-axis query has independently known ID-descending answers.
    let first = 32.0 * id as f32 / count as f32;
    let norm = (first * first + (DIMENSIONS - 1) as f32).sqrt();
    let mut values = vec![first / norm];
    let mut rng = SEED ^ id as u64;
    for _ in 1..DIMENSIONS {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        values.push(if rng & 1 == 0 {
            -1.0 / norm
        } else {
            1.0 / norm
        });
    }
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn create_fixture(path: &Path, count: i64) -> Value {
    assert!(
        !path.exists(),
        "scale fixture must never overwrite an existing DB"
    );
    let started = Instant::now();
    let mut conn = db::init_db(path).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute(
        "INSERT INTO tags(id,name,created_at) VALUES(1,'scale-tag',?1)",
        [FIXED_TIME],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO collections(id,name,created_at,updated_at) VALUES(1,'scale-collection',?1,?1)",
        [FIXED_TIME],
    )
    .unwrap();
    tx.execute("INSERT INTO model_aliases(alias,canonical,created_at,updated_at) VALUES('Flux raw','Flux',?1,?1)", [FIXED_TIME]).unwrap();
    {
        let mut asset = tx.prepare("INSERT INTO assets(id,path,name,favorite,width,height,file_size,format,mime_type,metadata_type,fingerprint,portable_id,missing,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'png','image/png',?8,?9,?10,?11,?12,?12)").unwrap();
        let mut prompt = tx.prepare("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,'',?3,?4)").unwrap();
        let mut generation = tx.prepare("INSERT INTO generation_index(asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise) VALUES(?1,?2,?3,?4,?5,?6,?7)").unwrap();
        let mut tag = tx
            .prepare("INSERT INTO asset_tags(asset_id,tag_id,created_at) VALUES(?1,1,?2)")
            .unwrap();
        let mut collection = tx
            .prepare(
                "INSERT INTO collection_assets(collection_id,asset_id,created_at) VALUES(1,?1,?2)",
            )
            .unwrap();
        let mut dna = tx
            .prepare("INSERT INTO visual_dna(asset_id,search_text,updated_at) VALUES(?1,?2,?3)")
            .unwrap();
        let mut reference = tx.prepare("INSERT INTO reference_sources(asset_id,page_title,created_at,updated_at) VALUES(?1,?2,?3,?3)").unwrap();
        let mut fts = tx.prepare("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags,visual_dna,reference) VALUES(?1,?2,?3,'',?4,?5,?6,?7)").unwrap();
        let mut cjk = tx
            .prepare("INSERT INTO asset_cjk_search(rowid,asset_id,text) VALUES(?1,?1,?2)")
            .unwrap();
        let mut embedding = tx.prepare("INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at) VALUES(?1,?2,512,?3,?4,?5)").unwrap();
        for id in 1..=count {
            let name = format!("asset {:06}", id % 1000); // duplicate names, unique managed IDs/paths
            let path = asset_path(id);
            let width = (id % 19 != 0).then_some(if id % 2 == 0 { 1536 } else { 768 });
            let height = width.map(|_| 1024);
            let fingerprint = format!("duplicate-fp-{}", id % 997);
            let missing = i64::from(id % 127 == 0);
            let metadata_type = if id % 3 == 0 { "comfyui" } else { "none" };
            let model = if id % 2 == 0 { "Flux raw" } else { "GPT Image" };
            let mut text = if regular(id) {
                format!("blue character 蓝发角色 海洋少女 夜晚卧室 asset {id}")
            } else {
                String::new()
            };
            if regular(id) && id % 100 == 0 {
                text.push_str(&" long-prompt".repeat(100));
            }
            let dna_text = if id % 13 == 0 {
                "dnaanchor 红衣角色"
            } else {
                ""
            };
            let reference_title = if id % 17 == 0 && id % 13 != 0 {
                "referenceanchor 紫色光影"
            } else {
                ""
            };
            let reference_text = if reference_title.is_empty() {
                String::new()
            } else {
                format!("{reference_title} web {{}}")
            };
            let tag_text = if id % 5 == 0 { "scale-tag" } else { "" };
            asset
                .execute(params![
                    id,
                    path,
                    name,
                    i64::from(id % 7 == 0),
                    width,
                    height,
                    1_000_000 + id,
                    metadata_type,
                    fingerprint,
                    format!("scale-{id}"),
                    missing,
                    FIXED_TIME + id
                ])
                .unwrap();
            prompt
                .execute(params![id, text, model, FIXED_TIME])
                .unwrap();
            generation
                .execute(params![
                    id,
                    format!("{}", 100000 + id),
                    20 + id % 31,
                    if id % 3 == 0 { "Euler" } else { "DPM++ 2M" },
                    if id % 2 == 0 { "normal" } else { "karras" },
                    3.0 + (id % 50) as f64 / 10.0,
                    0.5 + (id % 20) as f64 / 100.0
                ])
                .unwrap();
            if !tag_text.is_empty() {
                tag.execute(params![id, FIXED_TIME]).unwrap();
            }
            if id % 11 == 0 {
                collection.execute(params![id, FIXED_TIME]).unwrap();
            }
            if !dna_text.is_empty() {
                dna.execute(params![id, dna_text, FIXED_TIME]).unwrap();
            }
            if !reference_title.is_empty() {
                reference
                    .execute(params![id, reference_title, FIXED_TIME])
                    .unwrap();
            }
            fts.execute(params![
                id,
                name,
                text,
                model,
                tag_text,
                dna_text,
                reference_text
            ])
            .unwrap();
            cjk.execute(params![id,format!("{name}\u{1f}{text}\u{1f}\u{1f}{model}\u{1f}{tag_text}\u{1f}{dna_text}\u{1f}{reference_text}")]).unwrap();
            let embedding_fingerprint = if id % 131 == 0 {
                "stale".to_string()
            } else {
                fingerprint
            };
            embedding
                .execute(params![
                    id,
                    semantic::MODEL_ID,
                    vector_blob(id, count),
                    embedding_fingerprint,
                    FIXED_TIME
                ])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    // Validate direct fixture indexes against the production reindexer on all
    // relevant record shapes; this keeps bulk creation outside query timings.
    for id in [1, 13, 17, 100, 221] {
        let before: (String, String, String) = conn
            .query_row(
                "SELECT prompt,visual_dna,reference FROM asset_search WHERE asset_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        db::reindex_asset(&conn, id).unwrap();
        let after: (String, String, String) = conn
            .query_row(
                "SELECT prompt,visual_dna,reference FROM asset_search WHERE asset_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(before, after);
    }
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query([])
        .unwrap()
        .next()
        .unwrap()
        .is_none());
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    json!({"path":path,"rows":count,"schema":11,"journal_mode":"wal","database_bytes":fs::metadata(path).unwrap().len(),"fixture_build_ms":started.elapsed().as_secs_f64()*1000.0,"seed":format!("{SEED:#x}"),"dimensions":DIMENSIONS,"synthetic_image_bytes":false,"duplicate_names":true,"duplicate_fingerprints":true,"missing_every":127,"stale_embedding_every":131,"long_prompt_every":100})
}

fn state_for(path: &Path) -> AppState {
    let root = path.parent().unwrap().to_path_buf();
    AppState {
        db: Mutex::new(db::init_db(path).unwrap()),
        backup_operation: Mutex::new(()),
        data_dir: root.clone(),
        cache_dir: root.join("unused-cache"),
        database_path: path.to_path_buf(),
        backups_dir: root.join("unused-backups"),
        models_dir: root.join("unused-models"),
        jobs: Mutex::new(HashMap::new()),
        next_job_id: AtomicU64::new(1),
        vision_api_key: Mutex::new(String::new()),
    }
}

struct PageCase {
    name: &'static str,
    filter: LibraryFilter,
    offset: i64,
    expected: Vec<i64>,
    proposed_budget_ms: f64,
}

fn combined_filter() -> LibraryFilter {
    LibraryFilter {
        view: "favorites".into(),
        tag: Some("scale-tag".into()),
        model: Some("Flux".into()),
        collection_id: Some(1),
        metadata_type: Some("none".into()),
        sampler: Some("DPM++ 2M".into()),
        scheduler: Some("normal".into()),
        steps_min: Some(24),
        steps_max: Some(42),
        cfg_min: Some(4.0),
        cfg_max: Some(6.0),
        denoise_min: Some(0.52),
        denoise_max: Some(0.65),
        orientation: Some("landscape".into()),
        sort: "updated_desc".into(),
        ..LibraryFilter::default()
    }
}

fn combined_matches(id: i64) -> bool {
    let cfg = 3.0 + (id % 50) as f64 / 10.0;
    let denoise = 0.5 + (id % 20) as f64 / 100.0;
    id % 7 == 0
        && id % 5 == 0
        && id % 2 == 0
        && id % 11 == 0
        && id % 3 != 0
        && id % 19 != 0
        && (24..=42).contains(&(20 + id % 31))
        && (4.0..=6.0).contains(&cfg)
        && (0.52..=0.65).contains(&denoise)
}

fn page_cases(count: i64) -> Vec<PageCase> {
    let mut cases = Vec::new();
    let all = || (1..=count).rev().collect::<Vec<_>>();
    for (name, offset) in [
        ("page_first", 0),
        ("page_offset_4800", 4800),
        ("page_offset_24000", 24000),
        ("page_offset_48000", 48000),
    ] {
        cases.push(PageCase {
            name,
            filter: LibraryFilter {
                sort: "updated_desc".into(),
                ..LibraryFilter::default()
            },
            offset,
            expected: all(),
            proposed_budget_ms: 500.0,
        });
    }
    for (name, query, kind, budget) in [
        ("english", "blue character", 0, 500.0),
        ("cjk_one", "蓝", 0, 1000.0),
        ("cjk_two", "蓝发", 0, 1000.0),
        ("cjk_trigram", "蓝发角色", 0, 500.0),
        ("mixed_terms", "蓝发角色 blue", 0, 500.0),
        ("quoted_terms", "blue \"character\"", 0, 500.0),
        ("dna_only", "dnaanchor", 1, 500.0),
        ("dna_only_cjk", "红衣角色", 1, 500.0),
        ("reference_only", "referenceanchor", 2, 500.0),
        ("reference_only_cjk", "紫色光影", 2, 500.0),
        ("zero_results", "nonexistentxyz", 3, 500.0),
    ] {
        let expected = (1..=count)
            .rev()
            .filter(|&id| match kind {
                0 => regular(id),
                1 => id % 13 == 0,
                2 => id % 17 == 0 && id % 13 != 0,
                _ => false,
            })
            .collect();
        cases.push(PageCase {
            name,
            filter: LibraryFilter {
                query: query.into(),
                sort: "updated_desc".into(),
                ..LibraryFilter::default()
            },
            offset: 0,
            expected,
            proposed_budget_ms: budget,
        });
    }
    cases.push(PageCase {
        name: "combined_parameters",
        filter: combined_filter(),
        offset: 0,
        expected: (1..=count)
            .rev()
            .filter(|&id| combined_matches(id))
            .collect(),
        proposed_budget_ms: 500.0,
    });
    cases.push(PageCase {
        name: "exact_seed",
        filter: LibraryFilter {
            seed: Some("100221".into()),
            sort: "updated_desc".into(),
            ..LibraryFilter::default()
        },
        offset: 0,
        expected: vec![221],
        proposed_budget_ms: 500.0,
    });
    cases
}

fn stats(samples: &[f64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let p95 = sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    json!({"samples_ms":samples,"median_ms":median,"p95_ms":p95,"max_ms":sorted.last().unwrap(),"errors":0})
}

fn page_sample(state: &AppState, case: &PageCase) -> f64 {
    let started = Instant::now();
    let page = search::library_page(
        &state.db.lock().unwrap(),
        &case.filter,
        case.offset,
        PAGE_SIZE,
    )
    .unwrap();
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        page.total,
        case.expected.len() as i64,
        "{} total",
        case.name
    );
    let expected_page = case
        .expected
        .iter()
        .skip(case.offset as usize)
        .take(PAGE_SIZE as usize)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        page.items.iter().map(|x| x.id).collect::<Vec<_>>(),
        expected_page,
        "{} IDs",
        case.name
    );
    for summary in &page.items {
        assert_eq!(summary.path, asset_path(summary.id));
        assert_eq!(
            summary.fingerprint,
            format!("duplicate-fp-{}", summary.id % 997)
        );
        if summary.id % 19 == 0 {
            assert_eq!((summary.width, summary.height), (None, None));
        }
    }
    elapsed
}

fn benchmark_pages(path: &Path, count: i64) -> Vec<Value> {
    let mut results = Vec::new();
    for case in page_cases(count) {
        let cold = (0..COLD)
            .map(|_| page_sample(&state_for(path), &case))
            .collect::<Vec<_>>();
        let warm_state = state_for(path);
        page_sample(&warm_state, &case); // warm-up is excluded from the samples
        let warm = (0..WARM)
            .map(|_| page_sample(&warm_state, &case))
            .collect::<Vec<_>>();
        let summary = stats(&warm);
        let verdict = if summary["p95_ms"].as_f64().unwrap() <= case.proposed_budget_ms {
            "WITHIN_PROPOSED_BUDGET"
        } else {
            "EXCEEDS_PROPOSED_BUDGET"
        };
        println!(
            "scale {count} {} warm p95={:.1}ms {verdict}",
            case.name,
            summary["p95_ms"].as_f64().unwrap()
        );
        results.push(json!({"name":case.name,"production_path":"search::library_page (mutex + COUNT + summaries + ordering + OFFSET)","filter":case.filter,"offset":case.offset,"limit":PAGE_SIZE,"expected_total":case.expected.len(),"cold":stats(&cold),"warm":summary,"proposed_budget_ms":case.proposed_budget_ms,"budget_verdict":verdict}));
    }
    results
}

fn ranked_sample(state: &AppState, query: &[f32], filter: &LibraryFilter, expected: &[i64]) -> f64 {
    let started = Instant::now();
    let hits = semantic::ranked(state, query, filter.clone(), PAGE_SIZE, None).unwrap();
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        hits.iter().map(|x| x.asset.id).collect::<Vec<_>>(),
        expected
            .iter()
            .take(PAGE_SIZE as usize)
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(hits.windows(2).all(|pair| pair[0].score >= pair[1].score));
    elapsed
}

fn benchmark_ranked(path: &Path, count: i64) -> Vec<Value> {
    let mut query = vec![0.0; DIMENSIONS];
    query[0] = 1.0;
    let mut results = Vec::new();
    for (name, filter, combined) in [
        ("ranked_512_unfiltered", LibraryFilter::default(), false),
        ("ranked_512_combined", combined_filter(), true),
    ] {
        let expected = (1..=count)
            .rev()
            .filter(|&id| id % 127 != 0 && id % 131 != 0 && (!combined || combined_matches(id)))
            .collect::<Vec<_>>();
        let cold = (0..COLD)
            .map(|_| ranked_sample(&state_for(path), &query, &filter, &expected))
            .collect::<Vec<_>>();
        let warm_state = state_for(path);
        ranked_sample(&warm_state, &query, &filter, &expected);
        let warm = (0..WARM)
            .map(|_| ranked_sample(&warm_state, &query, &filter, &expected))
            .collect::<Vec<_>>();
        let summary = stats(&warm);
        let verdict = if summary["p95_ms"].as_f64().unwrap() <= 3000.0 {
            "WITHIN_PROPOSED_BUDGET"
        } else {
            "EXCEEDS_PROPOSED_BUDGET"
        };
        println!(
            "scale {count} {name} warm p95={:.1}ms {verdict}",
            summary["p95_ms"].as_f64().unwrap()
        );
        results.push(json!({"name":name,"production_path":"semantic::ranked (mutex + real filtered summaries + fresh joined embeddings + 512-dimension scan + sorting)","dimensions":DIMENSIONS,"query_kind":"synthetic unit-axis vector, no encoder","filter":filter,"expected_candidates":expected.len(),"expected_top_id":expected.first(),"known_high_id_omission_check":true,"cold":stats(&cold),"warm":summary,"proposed_budget_ms":3000.0,"budget_verdict":verdict}));
    }
    results
}

fn command_text(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|x| String::from_utf8_lossy(&x.stdout).trim().to_string())
        .unwrap_or_else(|e| format!("unavailable: {e}"))
}

fn machine_info() -> Value {
    let script = format!("$cpu=Get-CimInstance Win32_Processor; $os=Get-CimInstance Win32_OperatingSystem; $disks=Get-CimInstance Win32_DiskDrive; $p=Get-Process -Id {}; [ordered]@{{cpu=($cpu.Name -join ', '); logical_processors=($cpu.NumberOfLogicalProcessors | Measure-Object -Sum).Sum; total_memory_bytes=[long]$os.TotalVisibleMemorySize*1024; windows=$os.Caption; windows_version=$os.Version; disks=@($disks | Select-Object Model,InterfaceType,MediaType,Size); process_working_set_bytes=$p.WorkingSet64; process_peak_working_set_bytes=$p.PeakWorkingSet64; process_private_bytes=$p.PrivateMemorySize64}} | ConvertTo-Json -Depth 4 -Compress",std::process::id());
    let raw = command_text("powershell.exe", &["-NoProfile", "-Command", &script]);
    serde_json::from_str(&raw).unwrap_or_else(|_| json!({"unavailable":raw}))
}

#[test]
#[ignore = "explicit 50k/75k file-backed WAL native benchmark; retains large synthetic fixtures and raw evidence"]
fn file_backed_native_scale() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let evidence = std::env::var_os("IMAGELORE_SCALE_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("imagelore-native-scale-evidence"));
    fs::create_dir_all(&evidence).unwrap();
    let keep_dir = std::env::var_os("IMAGELORE_SCALE_KEEP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| evidence.clone());
    fs::create_dir_all(&keep_dir).unwrap();
    let reused = std::env::var_os("IMAGELORE_SCALE_REUSE_REPORT").map(|path| {
        let bytes = fs::read(path).unwrap();
        serde_json::from_slice::<Value>(&bytes).unwrap()
    });
    let mut report = json!({"format_version":1,"run_unix_seconds":stamp,"app_version":env!("CARGO_PKG_VERSION"),"git_head":command_text("git", &["rev-parse","HEAD"]),"rustc":command_text("rustc", &["-V"]),"build_mode":if cfg!(debug_assertions) {"debug-unoptimized"} else {"release"},"cold_samples":COLD,"warm_samples":WARM,"asset_dir":std::env::var("IMAGELORE_SCALE_ASSET_DIR").ok(),"fixture_keep_dir":keep_dir,"cold_definition":"fresh SQLite connection and statement/page caches; operating-system file cache is not flushed","timer_scope":"production Rust call only; SQLite reopen, fixture construction, assertions, and JSON writes excluded","claims_excluded":["model download","real model encoding","IPC","WebView rendering","image decoding","long-running UI memory"],"hardware_before":machine_info(),"fixtures":[]});
    let report_path = evidence.join(format!("native-scale-{stamp}.json"));
    for count in [50_000, 75_000] {
        let (path, fixture) = if let Some(previous) = &reused {
            let fixture = previous["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| &entry["fixture"])
                .find(|fixture| fixture["rows"].as_i64() == Some(count))
                .unwrap()
                .clone();
            let path = PathBuf::from(fixture["path"].as_str().unwrap());
            assert_eq!(fixture["seed"], format!("{SEED:#x}"));
            assert_eq!(fixture["dimensions"], DIMENSIONS);
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("native-scale-"),
                "reuse only explicitly labelled scale fixtures"
            );
            let state = state_for(&path);
            let conn = state.db.lock().unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM assets", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                count
            );
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM semantic_embeddings WHERE dimensions=512 AND LENGTH(vector)=2048", [], |r| r.get::<_, i64>(0)).unwrap(), count);
            println!(
                "reusing same synthetic WAL fixture {count}: {}",
                path.display()
            );
            (path, fixture)
        } else {
            let path = keep_dir.join(format!("native-scale-{stamp}-{count}.sqlite3"));
            println!("building synthetic file-backed WAL fixture {count}");
            let fixture = create_fixture(&path, count);
            (path, fixture)
        };
        let mut measurements = benchmark_pages(&path, count);
        measurements.extend(benchmark_ranked(&path, count));
        report["fixtures"]
            .as_array_mut()
            .unwrap()
            .push(json!({"fixture":fixture,"measurements":measurements}));
        fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    report["hardware_after"] = machine_info();
    fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("native scale evidence {}", report_path.display());
}
