use std::sync::OnceLock;
use crate::web::http::HttpResponse;

pub static INDEX_HTML: &str = include_str!("index.html");
pub static STYLE_CSS: &str = include_str!("style.css");
pub static JS_CORE: &str = include_str!("js/01_core.js");
pub static JS_TOPOLOGY_FACTS: &str = include_str!("js/02_topology_facts.js");
pub static JS_LOCKS_ECO_GOV: &str = include_str!("js/03_locks_eco_gov.js");
pub static JS_DEMO_BENCH_AUDIT: &str = include_str!("js/04_demo_bench_audit.js");
pub static JS_BODY: &str = include_str!("js/05_body.js");

pub fn app_js() -> &'static str {
    static JS_BUNDLE: OnceLock<String> = OnceLock::new();
    JS_BUNDLE.get_or_init(|| {
        [
            JS_CORE,
            JS_TOPOLOGY_FACTS,
            JS_LOCKS_ECO_GOV,
            JS_DEMO_BENCH_AUDIT,
            JS_BODY,
        ]
        .join("\n")
    })
}

pub fn serve_index() -> HttpResponse {
    HttpResponse::html(INDEX_HTML)
}
pub fn serve_css() -> HttpResponse {
    HttpResponse::css(STYLE_CSS)
}
pub fn serve_js() -> HttpResponse {
    HttpResponse::js(app_js())
}

pub static DECK_HTML: &str = include_str!("../../../presentation/agent-body-deck.html");
pub static PRESENTATION_HTML: &str = include_str!("../../../presentation/presentation.html");

pub fn serve_deck() -> HttpResponse {
    HttpResponse::html(DECK_HTML)
}

pub fn serve_presentation() -> HttpResponse {
    HttpResponse::html(PRESENTATION_HTML)
}
