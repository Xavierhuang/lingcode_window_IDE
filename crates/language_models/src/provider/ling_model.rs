//! LingModel: LingCode's managed, branded inference provider.
//!
//! LingModel speaks the Anthropic Messages protocol against a LingCode-hosted
//! proxy, so it reuses the `anthropic` crate's request/response machinery. The
//! upstream model the proxy forwards to is an operator/deploy detail and must
//! never appear in any user-visible string here — only "LingModel" or generic
//! "your LingCode API key" copy is allowed.
//!
//! Unlike the other providers, LingModel has no user-configurable settings: the
//! endpoint and model are fixed (it is a managed service), so it does not read
//! from `AllLanguageModelSettings` and adds no settings plumbing.

use anthropic::{AnthropicError, AnthropicModelMode};
use anyhow::{Result, anyhow};
use base64::Engine as _;
use client::UserStore;
use cloud_api_types::Plan;
use credentials_provider::CredentialsProvider;
use futures::{AsyncReadExt as _, FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use gpui::{AnyView, App, AsyncApp, Context, Entity, SharedString, Subscription, Task};
use http_client::{AsyncBody, HttpClient, Request as HttpRequest};
use rand::Rng as _;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::ling_model_auth::{LingModelAuthCallback, LingModelAuthEvent, LingModelAuthListener};
use language_model::{
    ApiKeyState, AuthenticateError, ConfigurationViewTargetAgent, EnvVar, IconOrSvg, LanguageModel,
    LanguageModelCacheConfiguration, LanguageModelCompletionError, LanguageModelCompletionEvent,
    LanguageModelId, LanguageModelName, LanguageModelProvider, LanguageModelProviderId,
    LanguageModelProviderName, LanguageModelProviderState, LanguageModelRequest,
    LanguageModelToolChoice, RateLimiter, env_var,
};
use std::sync::{Arc, LazyLock};
use ui::{ButtonLink, ConfiguredApiCard, List, ListBulletItem, prelude::*};
use ui_input::InputField;
use util::ResultExt;

use anthropic::completion::{AnthropicEventMapper, into_anthropic};

const PROVIDER_ID: LanguageModelProviderId = LanguageModelProviderId::new("lingmodel");
const PROVIDER_NAME: LanguageModelProviderName = LanguageModelProviderName::new("LingModel");

/// Anthropic-compatible base for the LingModel proxy. The Anthropic client
/// appends `/v1/messages`. This is a LingCode-branded host by design.
const LINGMODEL_API_URL: &str = "https://lingcode.dev/api/inference/anthropic";

/// Entitlement endpoint: returns the authenticated account's managed tier
/// (`free` | `pro` | `max_pro`). Authenticated with the same LingModel token
/// used for inference, sent as `Authorization: Bearer <token>`.
const ENTITLEMENT_URL: &str = "https://lingcode.dev/api/entitlement";

const API_KEY_ENV_VAR_NAME: &str = "LINGMODEL_API_KEY";
static API_KEY_ENV_VAR: LazyLock<EnvVar> = env_var!(API_KEY_ENV_VAR_NAME);

// ── Browser OAuth sign-in ────────────────────────────────────────────────────
//
// Mirrors the macOS app's "Sign In with Browser" flow: open the LingCode
// authorize page with PKCE, receive the `lingcode://auth/callback` redirect
// (parsed in the `zed` crate, routed here via `LingModelAuthListener`), and
// store the resulting access token in the same keychain slot the pasted-API-key
// path uses — so all inference code is unchanged. The API-key path remains as a
// fallback; OAuth is purely additive.
//
// Endpoints/client_id are LingCode-hosted; confirm them against the server
// before shipping. The server may either redirect with `access_token` directly
// (handled with no extra request) or with a `code` to exchange (handled below).
const OAUTH_AUTHORIZE_URL: &str = "https://lingcode.dev/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://lingcode.dev/oauth/token";
const OAUTH_CLIENT_ID: &str = "lingcode-ide";
const OAUTH_REDIRECT_URI: &str = "lingcode://auth/callback";

/// In-flight PKCE sign-in: the random `state` we expect echoed back and the
/// code `verifier` to present at token exchange. Lives only in memory, so an app
/// restart mid-flow falls back to the direct-token path (no verifier to exchange).
struct PendingAuth {
    state: String,
    verifier: String,
}

/// The single managed LingModel model. Tier routing (free/pro) happens
/// server-side based on the authenticated account, so the client exposes one
/// model and never names the upstream.
fn lingmodel_model() -> anthropic::Model {
    anthropic::Model::Custom {
        name: "lingmodel".to_string(),
        display_name: Some("LingModel".to_string()),
        max_tokens: 200_000,
        tool_override: None,
        cache_configuration: None,
        max_output_tokens: Some(64_000),
        default_temperature: None,
        extra_beta_headers: Vec::new(),
        mode: AnthropicModelMode::Default,
    }
}

pub struct LingModelLanguageModelProvider {
    http_client: Arc<dyn HttpClient>,
    state: Entity<State>,
}

pub struct State {
    api_key_state: ApiKeyState,
    credentials_provider: Arc<dyn CredentialsProvider>,
    http_client: Arc<dyn HttpClient>,
    /// Used to publish the fetched managed tier so the plan chip / agent panel
    /// reflect the real subscription. `None` in degraded/test paths.
    user_store: Option<Entity<UserStore>>,
    pending: Option<PendingAuth>,
    _auth_subscription: Option<Subscription>,
}

impl State {
    fn new(
        http_client: Arc<dyn HttpClient>,
        credentials_provider: Arc<dyn CredentialsProvider>,
        user_store: Option<Entity<UserStore>>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Subscribe to browser-OAuth callbacks routed in from the `zed` crate.
        // `try_global` (not `global`) so a provider built outside the normal
        // `language_models::init` path — e.g. a test — degrades to API-key-only
        // rather than panicking on a missing global.
        let _auth_subscription = LingModelAuthListener::try_global(cx).map(|listener| {
            let subscription = cx.subscribe(
                &listener,
                |state, _listener, event: &LingModelAuthEvent, cx| {
                    state.on_auth_callback(event.0.clone(), cx);
                },
            );

            // Drain a callback that may have been buffered before this
            // subscription existed (e.g. a cold launch where the OS delivered the
            // URL first).
            if let Some(buffered) = listener.update(cx, |listener, _| listener.take_last()) {
                let handle = cx.entity();
                cx.defer(move |cx| {
                    handle.update(cx, |state, cx| state.on_auth_callback(buffered, cx));
                });
            }

            subscription
        });

        Self {
            api_key_state: ApiKeyState::new(
                LingModelLanguageModelProvider::api_url(),
                (*API_KEY_ENV_VAR).clone(),
            ),
            credentials_provider,
            http_client,
            user_store,
            pending: None,
            _auth_subscription,
        }
    }

    fn is_authenticated(&self) -> bool {
        self.api_key_state.has_key()
    }

    /// Begin the browser PKCE sign-in: generate `state` + verifier/challenge,
    /// remember them, and open the authorize page in the user's browser.
    fn begin_browser_sign_in(&mut self, cx: &mut Context<Self>) {
        let mut verifier_bytes = [0u8; 32];
        rand::rng().fill(&mut verifier_bytes);
        let mut state_bytes = [0u8; 16];
        rand::rng().fill(&mut state_bytes);
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let verifier = engine.encode(verifier_bytes);
        let challenge = engine.encode(Sha256::digest(verifier.as_bytes()));
        let state = engine.encode(state_bytes);

        let url = format!(
            "{OAUTH_AUTHORIZE_URL}?response_type=code&client_id={OAUTH_CLIENT_ID}\
             &redirect_uri={redirect}&code_challenge={challenge}\
             &code_challenge_method=S256&state={state}",
            redirect = OAUTH_REDIRECT_URI,
        );

        self.pending = Some(PendingAuth { state, verifier });
        cx.open_url(&url);
    }

    /// Handle a parsed `lingcode://auth/callback`. Validates `state` against the
    /// in-flight request, then either stores a directly-returned `access_token`
    /// or exchanges the `code` for one. Always lands the token in the shared
    /// `ApiKeyState` keychain slot, so inference is unchanged.
    fn on_auth_callback(&mut self, callback: LingModelAuthCallback, cx: &mut Context<Self>) {
        if let Some(error) = callback.error {
            log::error!("LingModel sign-in failed: {error}");
            self.pending = None;
            return;
        }

        // If we have an in-flight request, the returned state must match. (A
        // cold-launch buffered callback may have no pending request; the direct
        // token path below is still safe because the token itself is the secret.)
        if let Some(pending) = self.pending.as_ref() {
            match callback.state.as_deref() {
                Some(state) if state == pending.state => {}
                _ => {
                    log::error!("LingModel sign-in: state mismatch; ignoring callback");
                    return;
                }
            }
        }

        if let Some(token) = callback.access_token {
            self.store_token(token, cx);
            self.pending = None;
            return;
        }

        let Some(code) = callback.code else {
            log::error!("LingModel sign-in: callback had neither access_token nor code");
            return;
        };
        let Some(pending) = self.pending.take() else {
            log::error!(
                "LingModel sign-in: received an authorization code with no pending verifier \
                 (app restarted mid-flow?); ask the user to sign in again"
            );
            return;
        };

        let http_client = self.http_client.clone();
        cx.spawn(async move |this, cx| {
            let token = exchange_code(http_client, &code, &pending.verifier).await?;
            this.update(cx, |state, cx| state.store_token(token, cx))?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn store_token(&mut self, token: String, cx: &mut Context<Self>) {
        self.set_api_key(Some(token), cx).detach_and_log_err(cx);
    }

    fn set_api_key(&mut self, api_key: Option<String>, cx: &mut Context<Self>) -> Task<Result<()>> {
        let credentials_provider = self.credentials_provider.clone();
        let store = self.api_key_state.store(
            LingModelLanguageModelProvider::api_url(),
            api_key,
            |this| &mut this.api_key_state,
            credentials_provider,
            cx,
        );
        // Once the key is persisted (set or cleared), refresh the managed tier so
        // the plan chip / agent panel track sign-in and sign-out.
        cx.spawn(async move |this, cx| {
            store.await?;
            this.update(cx, |this, cx| this.refresh_plan(cx))?;
            anyhow::Ok(())
        })
    }

    fn authenticate(&mut self, cx: &mut Context<Self>) -> Task<Result<(), AuthenticateError>> {
        let credentials_provider = self.credentials_provider.clone();
        let load = self.api_key_state.load_if_needed(
            LingModelLanguageModelProvider::api_url(),
            |this| &mut this.api_key_state,
            credentials_provider,
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = load.await;
            // After the stored token loads on startup, fetch the tier so a Pro /
            // Max Pro account shows the correct plan immediately. Best-effort.
            this.update(cx, |this, cx| this.refresh_plan(cx)).ok();
            result
        })
    }

    /// Fetch the account's managed tier from the entitlement endpoint using the
    /// stored token and publish it to `UserStore`. Clears the tier when there is
    /// no token. Best-effort: any failure leaves the current plan untouched.
    fn refresh_plan(&self, cx: &mut Context<Self>) {
        let Some(user_store) = self.user_store.clone() else {
            return;
        };
        let http_client = self.http_client.clone();
        let key = self
            .api_key_state
            .key(&LingModelLanguageModelProvider::api_url())
            .map(|key| key.to_string());

        cx.spawn(async move |_this, cx| {
            let plan = match key {
                Some(token) => fetch_plan(http_client, &token).await,
                None => None,
            };
            user_store
                .update(cx, |store, cx| store.set_lingmodel_plan(plan, cx))
                .ok();
        })
        .detach();
    }
}

impl LingModelLanguageModelProvider {
    pub fn new(
        http_client: Arc<dyn HttpClient>,
        credentials_provider: Arc<dyn CredentialsProvider>,
        user_store: Option<Entity<UserStore>>,
        cx: &mut App,
    ) -> Self {
        let state = cx
            .new(|cx| State::new(http_client.clone(), credentials_provider, user_store, cx));

        Self { http_client, state }
    }

    fn create_language_model(&self, model: anthropic::Model) -> Arc<dyn LanguageModel> {
        Arc::new(LingModelModel {
            id: LanguageModelId::from(model.id().to_string()),
            model,
            state: self.state.clone(),
            http_client: self.http_client.clone(),
            request_limiter: RateLimiter::new(4),
        })
    }

    fn api_url() -> SharedString {
        SharedString::new_static(LINGMODEL_API_URL)
    }
}

impl LanguageModelProviderState for LingModelLanguageModelProvider {
    type ObservableEntity = State;

    fn observable_entity(&self) -> Option<Entity<Self::ObservableEntity>> {
        Some(self.state.clone())
    }
}

impl LanguageModelProvider for LingModelLanguageModelProvider {
    fn id(&self) -> LanguageModelProviderId {
        PROVIDER_ID
    }

    fn name(&self) -> LanguageModelProviderName {
        PROVIDER_NAME
    }

    fn icon(&self) -> IconOrSvg {
        IconOrSvg::Icon(IconName::AiLingModel)
    }

    fn default_model(&self, _cx: &App) -> Option<Arc<dyn LanguageModel>> {
        Some(self.create_language_model(lingmodel_model()))
    }

    fn default_fast_model(&self, _cx: &App) -> Option<Arc<dyn LanguageModel>> {
        Some(self.create_language_model(lingmodel_model()))
    }

    // LingModel is LingCode's primary recommended model: it appears first in the
    // model picker's Recommended section (providers contribute in registration
    // order, and LingModel is registered first).
    fn recommended_models(&self, _cx: &App) -> Vec<Arc<dyn LanguageModel>> {
        vec![self.create_language_model(lingmodel_model())]
    }

    fn provided_models(&self, _cx: &App) -> Vec<Arc<dyn LanguageModel>> {
        vec![self.create_language_model(lingmodel_model())]
    }

    fn is_authenticated(&self, cx: &App) -> bool {
        self.state.read(cx).is_authenticated()
    }

    fn authenticate(&self, cx: &mut App) -> Task<Result<(), AuthenticateError>> {
        self.state.update(cx, |state, cx| state.authenticate(cx))
    }

    fn configuration_view(
        &self,
        _target_agent: ConfigurationViewTargetAgent,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        cx.new(|cx| ConfigurationView::new(self.state.clone(), window, cx))
            .into()
    }

    fn reset_credentials(&self, cx: &mut App) -> Task<Result<()>> {
        self.state
            .update(cx, |state, cx| state.set_api_key(None, cx))
    }
}

pub struct LingModelModel {
    id: LanguageModelId,
    model: anthropic::Model,
    state: Entity<State>,
    http_client: Arc<dyn HttpClient>,
    request_limiter: RateLimiter,
}

impl LingModelModel {
    fn stream_completion(
        &self,
        request: anthropic::Request,
        cx: &AsyncApp,
    ) -> BoxFuture<
        'static,
        Result<
            BoxStream<'static, Result<anthropic::Event, AnthropicError>>,
            LanguageModelCompletionError,
        >,
    > {
        let http_client = self.http_client.clone();

        let api_key = self
            .state
            .read_with(cx, |state, _cx| state.api_key_state.key(&Self::api_url()));

        let beta_headers = self.model.beta_headers();

        async move {
            let Some(api_key) = api_key else {
                return Err(LanguageModelCompletionError::NoApiKey {
                    provider: PROVIDER_NAME,
                });
            };
            let api_url = Self::api_url();
            let request = anthropic::stream_completion(
                http_client.as_ref(),
                &api_url,
                &api_key,
                request,
                beta_headers,
            );
            request.await.map_err(Into::into)
        }
        .boxed()
    }

    fn api_url() -> SharedString {
        SharedString::new_static(LINGMODEL_API_URL)
    }
}

impl LanguageModel for LingModelModel {
    fn id(&self) -> LanguageModelId {
        self.id.clone()
    }

    fn name(&self) -> LanguageModelName {
        LanguageModelName::from(self.model.display_name().to_string())
    }

    fn provider_id(&self) -> LanguageModelProviderId {
        PROVIDER_ID
    }

    fn provider_name(&self) -> LanguageModelProviderName {
        PROVIDER_NAME
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn supports_images(&self) -> bool {
        true
    }

    fn supports_streaming_tools(&self) -> bool {
        true
    }

    fn supports_tool_choice(&self, choice: LanguageModelToolChoice) -> bool {
        match choice {
            LanguageModelToolChoice::Auto
            | LanguageModelToolChoice::Any
            | LanguageModelToolChoice::None => true,
        }
    }

    fn supports_thinking(&self) -> bool {
        self.model.supports_thinking()
    }

    fn telemetry_id(&self) -> String {
        format!("lingmodel/{}", self.model.id())
    }

    fn api_key(&self, cx: &App) -> Option<String> {
        self.state.read_with(cx, |state, _cx| {
            state
                .api_key_state
                .key(&Self::api_url())
                .map(|key| key.to_string())
        })
    }

    fn max_token_count(&self) -> u64 {
        self.model.max_token_count()
    }

    fn max_output_tokens(&self) -> Option<u64> {
        Some(self.model.max_output_tokens())
    }

    fn stream_completion(
        &self,
        request: LanguageModelRequest,
        cx: &AsyncApp,
    ) -> BoxFuture<
        'static,
        Result<
            BoxStream<'static, Result<LanguageModelCompletionEvent, LanguageModelCompletionError>>,
            LanguageModelCompletionError,
        >,
    > {
        let mut request = into_anthropic(
            request,
            self.model.request_id().into(),
            self.model.default_temperature(),
            self.model.max_output_tokens(),
            self.model.mode(),
        );
        if !self.model.supports_speed() {
            request.speed = None;
        }
        let request = self.stream_completion(request, cx);
        let future = self.request_limiter.stream(async move {
            let response = request.await?;
            Ok(AnthropicEventMapper::new().map_stream(response))
        });
        async move { Ok(future.await?.boxed()) }.boxed()
    }

    fn cache_configuration(&self) -> Option<LanguageModelCacheConfiguration> {
        self.model
            .cache_configuration()
            .map(|config| LanguageModelCacheConfiguration {
                max_cache_anchors: config.max_cache_anchors,
                should_speculate: config.should_speculate,
                min_total_token: config.min_total_token,
            })
    }
}

/// GET the account's managed tier from the entitlement endpoint. Returns `None`
/// on any failure (network, non-2xx, or unparseable body) so callers no-op and
/// leave the current plan unchanged. An unrecognized tier maps to `Free`.
async fn fetch_plan(http_client: Arc<dyn HttpClient>, token: &str) -> Option<Plan> {
    let request = HttpRequest::builder()
        .method("GET")
        .uri(ENTITLEMENT_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .body(AsyncBody::default())
        .ok()?;

    let mut response = http_client.send(request).await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let mut text = String::new();
    response.body_mut().read_to_string(&mut text).await.ok()?;

    #[derive(Deserialize)]
    struct Entitlement {
        tier: Option<String>,
    }
    let parsed: Entitlement = serde_json::from_str(&text).ok()?;
    match parsed.tier.as_deref() {
        Some("pro") => Some(Plan::Pro),
        Some("max_pro") => Some(Plan::MaxPro),
        _ => Some(Plan::Free),
    }
}

/// Exchange a PKCE authorization code for a LingModel access token.
async fn exchange_code(
    http_client: Arc<dyn HttpClient>,
    code: &str,
    verifier: &str,
) -> Result<String> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code)
        .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
        .append_pair("client_id", OAUTH_CLIENT_ID)
        .append_pair("code_verifier", verifier)
        .finish();

    let request = HttpRequest::builder()
        .method("POST")
        .uri(OAUTH_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(AsyncBody::from(body.into_bytes()))?;

    let mut response = http_client.send(request).await?;
    let mut text = String::new();
    response.body_mut().read_to_string(&mut text).await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "LingModel token exchange failed ({}): {text}",
            response.status()
        ));
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }
    let parsed: TokenResponse = serde_json::from_str(&text)
        .map_err(|err| anyhow!("LingModel token response was not valid JSON: {err}"))?;
    Ok(parsed.access_token)
}

struct ConfigurationView {
    api_key_editor: Entity<InputField>,
    state: Entity<State>,
    load_credentials_task: Option<Task<()>>,
}

impl ConfigurationView {
    const PLACEHOLDER_TEXT: &'static str = "Your LingCode API key";

    fn new(state: Entity<State>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| {
            cx.notify();
        })
        .detach();

        let load_credentials_task = Some(cx.spawn({
            let state = state.clone();
            async move |this, cx| {
                let task = state.update(cx, |state, cx| state.authenticate(cx));
                // We don't log an error, because "not signed in" is also an error.
                let _ = task.await;
                this.update(cx, |this, cx| {
                    this.load_credentials_task = None;
                    cx.notify();
                })
                .log_err();
            }
        }));

        Self {
            api_key_editor: cx.new(|cx| InputField::new(window, cx, Self::PLACEHOLDER_TEXT)),
            state,
            load_credentials_task,
        }
    }

    fn save_api_key(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let api_key = self.api_key_editor.read(cx).text(cx).trim().to_string();
        if api_key.is_empty() {
            return;
        }

        self.api_key_editor
            .update(cx, |editor, cx| editor.set_text("", window, cx));

        let state = self.state.clone();
        cx.spawn_in(window, async move |_, cx| {
            state
                .update(cx, |state, cx| state.set_api_key(Some(api_key), cx))
                .await
        })
        .detach_and_log_err(cx);
    }

    fn sign_in(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, cx| state.begin_browser_sign_in(cx));
    }

    fn reset_api_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.api_key_editor
            .update(cx, |editor, cx| editor.set_text("", window, cx));

        let state = self.state.clone();
        cx.spawn_in(window, async move |_, cx| {
            state
                .update(cx, |state, cx| state.set_api_key(None, cx))
                .await
        })
        .detach_and_log_err(cx);
    }

    fn should_render_editor(&self, cx: &mut Context<Self>) -> bool {
        !self.state.read(cx).is_authenticated()
    }
}

impl Render for ConfigurationView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let env_var_set = self.state.read(cx).api_key_state.is_from_env_var();
        let configured_card_label = if env_var_set {
            format!("API key set in {API_KEY_ENV_VAR_NAME} environment variable")
        } else {
            "LingModel API key configured".to_string()
        };

        if self.load_credentials_task.is_some() {
            div()
                .child(Label::new("Loading credentials..."))
                .into_any_element()
        } else if self.should_render_editor(cx) {
            v_flex()
                .size_full()
                .gap_2()
                .on_action(cx.listener(Self::save_api_key))
                .child(
                    Button::new("lingmodel-sign-in", "Sign In with Browser")
                        .on_click(cx.listener(|this, _, window, cx| this.sign_in(window, cx))),
                )
                .child(
                    Label::new("or use an API key instead:")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(Label::new(
                    "To use LingModel, add your LingCode API key:",
                ))
                .child(
                    List::new()
                        .child(
                            ListBulletItem::new("")
                                .child(Label::new("Get your API key from your"))
                                .child(ButtonLink::new(
                                    "LingCode account",
                                    "https://lingcode.dev/account.html",
                                )),
                        )
                        .child(ListBulletItem::new(
                            "Paste your API key below and hit enter to start using LingModel",
                        )),
                )
                .child(self.api_key_editor.clone())
                .child(
                    Label::new(format!(
                        "You can also set the {API_KEY_ENV_VAR_NAME} environment variable and restart LingCode."
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .mt_0p5(),
                )
                .into_any_element()
        } else {
            ConfiguredApiCard::new(configured_card_label)
                .disabled(env_var_set)
                .on_click(cx.listener(|this, _, window, cx| this.reset_api_key(window, cx)))
                .when(env_var_set, |this| {
                    this.tooltip_label(format!(
                        "To reset your API key, unset the {API_KEY_ENV_VAR_NAME} environment variable."
                    ))
                })
                .into_any_element()
        }
    }
}
