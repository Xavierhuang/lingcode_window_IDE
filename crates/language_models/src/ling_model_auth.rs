//! Cross-crate plumbing for LingModel browser OAuth sign-in.
//!
//! The `lingcode://auth/callback` URL is parsed in the `zed` crate (its
//! `open_listener`), but the LingModel provider that consumes the resulting
//! token lives here in `language_models`. This module is the bridge:
//!
//! - a global listener entity the provider subscribes to, and
//! - [`deliver_ling_model_auth`], which the `zed` crate calls when it parses a
//!   callback.
//!
//! The most recent callback is buffered so a provider that attaches its
//! subscription slightly after delivery — or a cold launch where the OS hands us
//! the URL before the provider is built — can still drain it via
//! [`LingModelAuthListener::take_last`].
//!
//! Modeled on `client::RefreshLlmTokenListener`, the codebase's existing
//! "external event → provider subscribes" pattern.

use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, ReadGlobal as _};

/// Parsed `lingcode://auth/callback` payload. Either `access_token` is present
/// (direct/implicit redirect) or `code` is (authorization-code + PKCE), or
/// `error` describes a failed/denied sign-in.
#[derive(Clone, Default)]
pub struct LingModelAuthCallback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub access_token: Option<String>,
    pub error: Option<String>,
}

impl std::fmt::Debug for LingModelAuthCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log the secret material.
        f.debug_struct("LingModelAuthCallback")
            .field("code", &self.code.as_ref().map(|_| "[redacted]"))
            .field("state", &self.state)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[redacted]"),
            )
            .field("error", &self.error)
            .finish()
    }
}

/// Emitted to subscribers (the LingModel provider) when a callback arrives.
pub struct LingModelAuthEvent(pub LingModelAuthCallback);

struct GlobalLingModelAuthListener(Entity<LingModelAuthListener>);

impl Global for GlobalLingModelAuthListener {}

pub struct LingModelAuthListener {
    last: Option<LingModelAuthCallback>,
}

impl EventEmitter<LingModelAuthEvent> for LingModelAuthListener {}

impl LingModelAuthListener {
    /// Install the global listener. Call once, early in app init — before the
    /// LingModel provider is registered, so the provider's subscription is live
    /// by the time any callback can arrive.
    pub fn register(cx: &mut App) {
        if cx.try_global::<GlobalLingModelAuthListener>().is_some() {
            return;
        }
        let listener = cx.new(|_| LingModelAuthListener { last: None });
        cx.set_global(GlobalLingModelAuthListener(listener));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        GlobalLingModelAuthListener::global(cx).0.clone()
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalLingModelAuthListener>()
            .map(|global| global.0.clone())
    }

    /// Drain the buffered callback, if any. Used by a late/cold-launch subscriber.
    pub fn take_last(&mut self) -> Option<LingModelAuthCallback> {
        self.last.take()
    }

    fn deliver(&mut self, callback: LingModelAuthCallback, cx: &mut Context<Self>) {
        self.last = Some(callback.clone());
        cx.emit(LingModelAuthEvent(callback));
    }
}

/// Called from the `zed` crate when it parses a `lingcode://auth/callback` URL.
/// No-op (with a warning) if the listener global isn't installed yet.
pub fn deliver_ling_model_auth(callback: LingModelAuthCallback, cx: &mut App) {
    if let Some(listener) = LingModelAuthListener::try_global(cx) {
        listener.update(cx, |listener, cx| listener.deliver(callback, cx));
    } else {
        log::warn!("LingModel auth callback arrived before the listener was registered; dropping");
    }
}
