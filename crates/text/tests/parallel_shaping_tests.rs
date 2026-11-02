//! Process-wide font identity and shaping on worker threads.
use cove_core::{Font, FontWeight, ShapedGlyph};
use cove_text::{
    FontProvider, SystemFontDefaults, TextBootstrapOptions, TextCacheLimits, TextEnvironment,
    font_registry,
};

fn environment() -> TextEnvironment {
    TextEnvironment::bootstrapped(TextBootstrapOptions {
        cache_limits: TextCacheLimits {
            maximum_entries: 4096,
            maximum_bytes: 64 * 1024 * 1024,
        },
        ..TextBootstrapOptions::default()
    })
}

fn provider(size: f32, weight: FontWeight) -> FontProvider {
    let mut provider = FontProvider::new(size, 2.0);
    provider
        .load_primary_font(&SystemFontDefaults::primary_font_name(), weight, false)
        .expect("a system font");
    provider
}

const SAMPLES: &[&str] = &[
    "The quick brown fox",
    "fi fl ffi office",
    "AVATAR Wave To",
    "Scene Hierarchy 123",
    "你好，世界",
    "Hello 你好 world",
    "مرحبا بالعالم",
    "abc مرحبا def",
    "שלום עולם",
];

fn texts(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("{} {index}", SAMPLES[index % SAMPLES.len()]))
        .collect()
}

#[test]
fn every_thread_gets_the_same_id_for_a_font() {
    let ids = |size| {
        let font = provider(size, FontWeight::Regular).primary_font().unwrap();
        font.id
    };
    let here = ids(17.0);
    let there = std::thread::spawn(move || ids(17.0)).join().unwrap();
    assert_eq!(here, there);
    assert_ne!(here, ids(19.0), "a different size is a different font");
}

#[test]
fn providers_on_a_thread_share_one_instance() {
    let first = provider(15.0, FontWeight::Regular);
    let second = provider(15.0, FontWeight::Regular);
    assert!(std::rc::Rc::ptr_eq(
        &first.primary_font().unwrap(),
        &second.primary_font().unwrap(),
    ));
}

#[test]
fn a_font_file_is_shared_while_it_is_in_use() {
    let font = provider(16.0, FontWeight::Regular).primary_font().unwrap();
    let key = font_registry::key(font.id).expect("the provider's font has a key");
    let first = font_registry::file(key.path()).unwrap();
    let second = font_registry::file(key.path()).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn a_worker_instance_shapes_the_same_glyphs() {
    let mut provider = provider(14.0, FontWeight::Medium);
    for text in SAMPLES {
        let runs = provider.resolve_runs(text);
        let here: Vec<ShapedGlyph> = runs.iter().flat_map(|run| run.shape()).collect();
        let portable: Vec<_> = runs.iter().filter_map(|run| run.portable()).collect();
        assert_eq!(portable.len(), runs.len());
        let there: Vec<ShapedGlyph> = std::thread::spawn(move || {
            portable
                .iter()
                .flat_map(|run| run.shape().expect("the worker opens the font"))
                .collect()
        })
        .join()
        .unwrap();
        assert_eq!(here, there, "{text}");
    }
}

#[test]
fn a_batch_shapes_the_same_glyphs_as_one_at_a_time() {
    let font = Font::system(13.0, FontWeight::Regular);
    let texts = texts(400);
    let serial = environment();
    let expected: Vec<_> = texts
        .iter()
        .map(|text| serial.shape(text, Some(&font)))
        .collect();
    let batched = environment();
    for text in &texts {
        let (text, font) = (text.clone(), font.clone());
        batched.request_shape(move |_| Some((text, font)));
    }
    let before = batched.shaping_cache_snapshot().entries;
    batched.shape("first miss", Some(&font));
    assert_eq!(
        batched.shaping_cache_snapshot().entries,
        before + texts.len() + 1,
        "the miss shapes the whole queue"
    );
    for (text, expected) in texts.iter().zip(&expected) {
        assert_eq!(batched.shape(text, Some(&font)), *expected, "{text}");
    }
}

#[test]
fn a_full_queue_stops_short_of_the_cache_limit() {
    let environment = TextEnvironment::bootstrapped(TextBootstrapOptions {
        cache_limits: TextCacheLimits {
            maximum_entries: 8,
            maximum_bytes: 1024 * 1024,
        },
        ..TextBootstrapOptions::default()
    });
    let font = Font::system(13.0, FontWeight::Regular);
    let called = std::rc::Rc::new(std::cell::Cell::new(0));
    for index in 0..32 {
        let (called, font) = (called.clone(), font.clone());
        environment.request_shape(move |_| {
            called.set(called.get() + 1);
            Some((format!("label {index}"), font))
        });
    }
    environment.shape("miss", Some(&font));
    assert_eq!(called.get(), 8);
}

#[test]
fn trimming_drops_the_queue() {
    let environment = environment();
    let called = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = called.clone();
    environment.request_shape(move |_| {
        flag.set(true);
        None
    });
    environment.trim_text_caches();
    environment.shape("miss", None);
    assert!(!called.get());
}

/// `cargo test -p cove-text --release --test parallel_shaping_tests -- --ignored --nocapture`
#[test]
#[ignore]
fn batch_shaping_cost() {
    let font = Font::system(13.0, FontWeight::Regular);
    for count in [50, 150, 500, 2000] {
        let texts = texts(count);
        let rounds = 20;
        let warm = environment();
        let warm_up = |environment: &TextEnvironment| {
            for text in &texts {
                let (text, font) = (text.clone(), font.clone());
                environment.request_shape(move |_| Some((text, font)));
            }
            environment.shape("warm", Some(&font));
        };
        warm_up(&warm);
        let (mut serial, mut batched) = (0.0, 0.0);
        for _ in 0..rounds {
            warm.trim_text_caches();
            let start = std::time::Instant::now();
            for text in &texts {
                warm.shape(text, Some(&font));
            }
            serial += start.elapsed().as_secs_f64();
            warm.trim_text_caches();
            let start = std::time::Instant::now();
            warm_up(&warm);
            batched += start.elapsed().as_secs_f64();
        }
        let per = |total: f64| total / rounds as f64 * 1e3;
        eprintln!(
            "{count} labels: one at a time={:.2}ms batch={:.2}ms ({} threads)",
            per(serial),
            per(batched),
            cove_core::work::pool().threads(),
        );
    }
}
