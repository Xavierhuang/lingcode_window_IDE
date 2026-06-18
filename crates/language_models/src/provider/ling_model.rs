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
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use gpui::{AnyView, App, AsyncApp, Context, Entity, SharedString, Task};
use http_client::HttpClient;
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

const API_KEY_ENV_VAR_NAME: &str = "LINGMODEL_API_KEY";
static API_KEY_ENV_VAR: LazyLock<EnvVar> = env_var!(API_KEY_ENV_VAR_NAME);

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
}

impl State {
    fn is_authenticated(&self) -> bool {
        self.api_key_state.has_key()
    }

    fn set_api_key(&mut self, api_key: Option<String>, cx: &mut Context<Self>) -> Task<Result<()>> {
        let credentials_provider = self.credentials_provider.clone();
        self.api_key_state.store(
            LingModelLanguageModelProvider::api_url(),
            api_key,
            |this| &mut this.api_key_state,
            credentials_provider,
            cx,
        )
    }

    fn authenticate(&mut self, cx: &mut Context<Self>) -> Task<Result<(), AuthenticateError>> {
        let credentials_provider = self.credentials_provider.clone();
        self.api_key_state.load_if_needed(
            LingModelLanguageModelProvider::api_url(),
            |this| &mut this.api_key_state,
            credentials_provider,
            cx,
        )
    }
}

impl LingModelLanguageModelProvider {
    pub fn new(
        http_client: Arc<dyn HttpClient>,
        credentials_provider: Arc<dyn CredentialsProvider>,
        cx: &mut App,
    ) -> Self {
        let state = cx.new(|_cx| State {
            api_key_state: ApiKeyState::new(Self::api_url(), (*API_KEY_ENV_VAR).clone()),
            credentials_provider,
        });

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
                .on_action(cx.listener(Self::save_api_key))
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
