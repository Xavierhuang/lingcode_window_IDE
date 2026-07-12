//! Contains helper functions for constructing URLs to various Zed-related pages.
//!
//! These URLs will adapt to the configured server URL in order to construct
//! links appropriate for the environment (e.g., by linking to a local copy of
//! zed.dev in development).

use gpui::App;
use settings::Settings;

use crate::ClientSettings;

fn server_url(cx: &App) -> &str {
    &ClientSettings::get_global(cx).server_url
}

/// Returns the URL to the LingCode account page.
///
/// This is the deployed marketing/account site (served from the LingCode web
/// root), which is independent of the collab `server_url`.
pub fn account_url(_cx: &App) -> String {
    "https://lingcode.dev/account.html".to_string()
}

/// Returns the URL to the LingCode pricing/checkout page.
///
/// LingCode has no separate "start trial" flow; the pricing page hosts the
/// Free / Pro / Max Pro plans and their Stripe checkout links.
pub fn start_trial_url(_cx: &App) -> String {
    "https://lingcode.dev/pricing.html".to_string()
}

/// Returns the URL to the LingCode pricing/upgrade page.
pub fn upgrade_to_zed_pro_url(_cx: &App) -> String {
    "https://lingcode.dev/pricing.html".to_string()
}

/// Returns the URL to Zed's terms of service.
pub fn terms_of_service(cx: &App) -> String {
    format!("{server_url}/terms-of-service", server_url = server_url(cx))
}

/// Returns the URL to Zed AI's privacy and security docs.
pub fn ai_privacy_and_security(cx: &App) -> String {
    format!(
        "{server_url}/docs/ai/privacy-and-security",
        server_url = server_url(cx)
    )
}

/// Returns the URL to Zed's edit prediction documentation.
pub fn edit_prediction_docs(cx: &App) -> String {
    format!(
        "{server_url}/docs/ai/edit-prediction",
        server_url = server_url(cx)
    )
}

/// Returns the URL to Zed's ACP registry blog post.
pub fn acp_registry_blog(cx: &App) -> String {
    format!(
        "{server_url}/blog/acp-registry",
        server_url = server_url(cx)
    )
}

/// Returns the URL to Zed's Parallel Agents blog post.
pub fn parallel_agents_blog(cx: &App) -> String {
    format!("{server_url}/blog", server_url = server_url(cx))
}

pub fn shared_agent_thread_url(session_id: &str) -> String {
    format!("zed://agent/shared/{}", session_id)
}
