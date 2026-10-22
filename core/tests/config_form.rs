//! 設定の画面の中身が Python 版（conftoml.py）と同じか（tools/record_config_fixtures.py で記録）
use egword_core::conftoml::{check_config, dump_toml, form_from_config, form_to_config, ConfigForm};
use egword_core::parse_config;
use serde_json::Value as J;

#[test]
fn config_forms_match_python() {
    let fx: Vec<J> = serde_json::from_str(include_str!("fixtures/config.json")).unwrap();
    let css = |n: &str| n == "book.css";
    let mut n = 0;
    for case in &fx {
        let name = case["name"].as_str().unwrap();
        let data = parse_config(case["text"].as_str().unwrap()).unwrap();
        let form = form_from_config(&data);
        let want: ConfigForm = serde_json::from_value(case["form"].clone()).unwrap();
        assert_eq!(form, want, "form {}", name);
        let probs: Vec<String> = serde_json::from_value(case["problems"].clone()).unwrap();
        assert_eq!(check_config(&data, &css), probs, "problems {}", name);
        for e in case["edits"].as_array().unwrap() {
            let mut f = serde_json::to_value(&form).unwrap();
            for (k, v) in e["change"].as_object().unwrap() { f[k] = v.clone(); }
            let f: ConfigForm = serde_json::from_value(f).unwrap();
            let d = form_to_config(&data, &f);
            assert_eq!(dump_toml(&d), e["dump"].as_str().unwrap(), "dump {} / {}", name, e["name"]);
            let probs: Vec<String> = serde_json::from_value(e["problems"].clone()).unwrap();
            assert_eq!(check_config(&d, &css), probs, "problems {} / {}", name, e["name"]);
            // 書き出したものを読み直すと同じ設定になる
            assert_eq!(parse_config(&dump_toml(&d)).unwrap(), d, "reparse {} / {}", name, e["name"]);
            n += 1;
        }
    }
    assert_eq!(n, 42);
}

#[test]
fn json_keeps_dates() {
    let d = parse_config("[book]\ndate = 2026-01-02\nx = 1.5\n").unwrap();
    let j = egword_core::conftoml::table_to_json(&d);
    let back = egword_core::conftoml::table_from_json(&j).unwrap();
    assert_eq!(back, d);
    assert!(dump_toml(&back).contains("date = 2026-01-02"));
}
