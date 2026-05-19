use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use super::auth;

const CSV_URL: &str =
    "https://cursor.com/api/dashboard/export-usage-events-csv?strategy=tokens";
const REFERER: &str = "https://www.cursor.com/settings";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageRow {
    pub date: String,
    pub model: String,
    pub kind: String,
    pub max_mode: bool,
    pub input_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
    pub billable: bool,
}

pub fn fetch_usage_rows() -> Result<Vec<CursorUsageRow>, String> {
    let session = auth::extract_session()?;
    let cookie = session.cookie;
    // Run reqwest::blocking on a dedicated OS thread so we don't conflict
    // with any outer tokio runtime (send_all_reports calls us from async ctx).
    let csv = std::thread::spawn(move || fetch_csv(&cookie))
        .join()
        .map_err(|_| "cursor csv fetch thread panicked".to_string())??;
    Ok(parse_csv(&csv))
}

/// Map a fetch error string to a stable status code surfaced to the UI.
/// Lets the frontend decide between "需要登录" vs "网络/服务异常" vs "本地未安装".
///
/// Keep the mapping anchored on substrings that appear in the error strings
/// produced by `extract_session` and `fetch_csv` — when we add new error
/// surfaces, extend this match.
pub fn classify_error(err: &str) -> &'static str {
    let e = err.to_ascii_lowercase();
    if e.contains("session expired")
        || e.contains("access token missing")
        || e.contains("userid unavailable")
        || e.contains("returned 401")
        || e.contains("returned 403")
    {
        return "expired";
    }
    if e.contains("not found")
        || e.contains("not resolvable")
        || e.contains("open state.vscdb")
    {
        return "missing";
    }
    if e.contains("request")
        || e.contains("thread panicked")
        || e.contains("read csv body")
        || e.contains("build http client")
    {
        return "network";
    }
    "unknown"
}

fn fetch_csv(cookie: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("build http client: {}", e))?;

    let resp = client
        .get(CSV_URL)
        .header("Cookie", cookie)
        .header("Referer", REFERER)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "*/*")
        .send()
        .map_err(|e| format!("cursor csv request: {}", e))?;

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err("cursor session expired — re-login in Cursor".to_string());
    }
    if !status.is_success() {
        return Err(format!("cursor api returned {}", status.as_u16()));
    }

    resp.text().map_err(|e| format!("read csv body: {}", e))
}

fn parse_csv(csv: &str) -> Vec<CursorUsageRow> {
    let mut lines = csv.split('\n').filter(|l| !l.trim().is_empty());
    let header = match lines.next() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let header_fields: Vec<String> = parse_csv_line(header)
        .into_iter()
        .map(|f| strip_quotes(&f).to_string())
        .collect();
    let mut idx: HashMap<&str, usize> = HashMap::new();
    for (i, name) in header_fields.iter().enumerate() {
        idx.insert(name.as_str(), i);
    }

    let date_i = match idx.get("Date") { Some(i) => *i, None => return Vec::new() };
    let model_i = match idx.get("Model") { Some(i) => *i, None => return Vec::new() };
    let input_with_i = match idx.get("Input (w/ Cache Write)") {
        Some(i) => *i, None => return Vec::new()
    };
    let input_without_i = match idx.get("Input (w/o Cache Write)") {
        Some(i) => *i, None => return Vec::new()
    };
    let cache_read_i = match idx.get("Cache Read") {
        Some(i) => *i, None => return Vec::new()
    };
    let output_i = match idx.get("Output Tokens") {
        Some(i) => *i, None => return Vec::new()
    };
    let total_i = match idx.get("Total Tokens") {
        Some(i) => *i, None => return Vec::new()
    };
    // 2026-05 起 cursor.com 的 export-usage-events-csv 把 Cost 列移除,
    // 改用 "Requests" 列(单次事件计数)。Cost 列回不来,把它降级为可选 ——
    // 缺失时按 0 处理,UI 的"估算费用"卡片会显示 $0,但 token 维度照常解析。
    // 旧版 CSV (仍可能在客户老缓存里出现) 继续兼容。
    let cost_i = idx.get("Cost").copied();
    let kind_i = idx.get("Kind").copied();
    let max_mode_i = idx.get("Max Mode").copied();

    let min_len = {
        let mut m = [date_i, model_i, input_with_i, input_without_i, cache_read_i, output_i, total_i]
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        if let Some(i) = cost_i { if i > m { m = i; } }
        if let Some(i) = kind_i { if i > m { m = i; } }
        if let Some(i) = max_mode_i { if i > m { m = i; } }
        m + 1
    };

    let mut out = Vec::new();
    for line in lines {
        let fields = parse_csv_line(line);
        if fields.len() < min_len {
            continue;
        }
        let input_with = to_u64(&fields[input_with_i]);
        let input_without = to_u64(&fields[input_without_i]);
        let cache_write = input_with.saturating_sub(input_without);
        let cache_read = to_u64(&fields[cache_read_i]);
        let output = to_u64(&fields[output_i]);
        let total = to_u64(&fields[total_i]);
        let cost = cost_i.map(|i| to_f64(&fields[i])).unwrap_or(0.0);
        let kind = kind_i
            .map(|i| strip_quotes(&fields[i]).to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let max_mode = max_mode_i
            .map(|i| strip_quotes(&fields[i]).eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let billable = is_billable_kind(&kind);

        // Skip empty rows (matches TokenTracker behavior)
        if total == 0 && input_without == 0 && output == 0 {
            continue;
        }

        out.push(CursorUsageRow {
            date: date_to_ymd(strip_quotes(&fields[date_i])),
            model: strip_quotes(&fields[model_i]).to_string(),
            kind,
            max_mode,
            input_tokens: input_without,
            cache_write_tokens: cache_write,
            cache_read_tokens: cache_read,
            output_tokens: output,
            total_tokens: total,
            cost,
            billable,
        });
    }
    out
}

/// Cursor's CSV reports Date as a full ISO timestamp (e.g.
/// `2026-04-27T03:46:43.488Z`). Aggregations and the server's date dimension
/// expect `YYYY-MM-DD`, so we truncate.
fn date_to_ymd(raw: &str) -> String {
    if raw.len() >= 10 {
        raw[..10].to_string()
    } else {
        raw.to_string()
    }
}

fn is_billable_kind(kind: &str) -> bool {
    let k = kind.trim().to_ascii_lowercase();
    if k.is_empty() {
        return true;
    }
    // "no charge" / "free" — 老格式的非计费值
    // "included"          — 2026-05 新版 CSV 引入,订阅内事件 cursor.com 显示 $0
    if k.contains("no charge") || k == "free" || k == "included" {
        return false;
    }
    true
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            current.push(ch);
        } else if ch == ',' && !in_quotes {
            fields.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn strip_quotes(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

fn to_u64(s: &str) -> u64 {
    let cleaned = strip_quotes(s).replace([',', '$'], "");
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0).map(|n| n as u64).unwrap_or(0)
}

fn to_f64(s: &str) -> f64 {
    let cleaned = strip_quotes(s).replace([',', '$'], "");
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_csv() {
        let csv = "Date,Model,Kind,Max Mode,Input (w/ Cache Write),Input (w/o Cache Write),Cache Read,Output Tokens,Total Tokens,Cost\n\
                   2026-05-15T01:23:45.000Z,claude-sonnet-4,Usage-based,No,1000,200,800,300,1300,0.05\n\
                   2026-05-15T02:00:00.000Z,gpt-5,No charge,No,500,100,400,150,650,0.0";
        let rows = parse_csv(csv);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, "2026-05-15", "date should be truncated to YYYY-MM-DD");
        assert_eq!(rows[0].input_tokens, 200);
        assert_eq!(rows[0].cache_write_tokens, 800);
        assert_eq!(rows[0].cache_read_tokens, 800);
        assert_eq!(rows[0].billable, true);
        assert_eq!(rows[1].billable, false);
    }

    #[test]
    fn tolerates_reordered_columns() {
        let csv = "Model,Date,Kind,Max Mode,Cache Read,Input (w/ Cache Write),Input (w/o Cache Write),Output Tokens,Total Tokens,Cost\n\
                   claude-opus-4,2026-05-14,Usage-based,Yes,500,1000,300,200,1000,0.10";
        let rows = parse_csv(csv);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-05-14");
        assert_eq!(rows[0].model, "claude-opus-4");
        assert!(rows[0].max_mode);
    }

    #[test]
    fn rejects_missing_required_columns() {
        let csv = "Date,Model\n2026-05-15,gpt-5";
        assert!(parse_csv(csv).is_empty());
    }

    /// End-to-end smoke test against the real cursor.com API.
    /// Requires Cursor to be installed and logged in on the host machine.
    /// Run with: `cargo test --lib cursor::api -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fetch_real_usage() {
        match fetch_usage_rows() {
            Ok(rows) => {
                println!("rows: {}", rows.len());
                for r in rows.iter().take(5) {
                    println!("  {} {} kind={} max={} in={} out={} cr={} cw={} cost=${:.4} billable={}",
                        r.date, r.model, r.kind, r.max_mode,
                        r.input_tokens, r.output_tokens,
                        r.cache_read_tokens, r.cache_write_tokens,
                        r.cost, r.billable);
                }
                let total_cost: f64 = rows.iter().filter(|r| r.billable).map(|r| r.cost).sum();
                let total_tokens: u64 = rows.iter().filter(|r| r.billable)
                    .map(|r| r.input_tokens + r.output_tokens + r.cache_read_tokens + r.cache_write_tokens)
                    .sum();
                println!("billable total tokens: {} cost: ${:.2}", total_tokens, total_cost);
                assert!(!rows.is_empty(), "expected at least 1 usage row");
            }
            Err(e) => panic!("fetch failed: {}", e),
        }
    }
}
