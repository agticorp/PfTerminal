//! Provider profile definitions for Claude Code headless panes.

use codex_model_provider_info::AMBIENT_DEFAULT_MODEL;
use codex_model_provider_info::AMBIENT_KIMI_K2_7_CODE_MODEL;
use codex_model_provider_info::BASETEN_DEFAULT_MODEL;
use codex_model_provider_info::OPENROUTER_DEFAULT_MODEL;
use codex_model_provider_info::VERCEL_DEFAULT_MODEL;
use codex_model_provider_info::VERCEL_GLM_5_2_FAST_MODEL;
use codex_model_provider_info::ZAI_DEFAULT_MODEL;
use serde::Deserialize;
use serde::Serialize;

/// Built-in provider profile kinds available for Claude Code panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ClaudeProviderProfileKind {
    ClaudePlan,
    AmbientGlm52,
    AmbientKimiK27,
    ZaiGlm52,
    BasetenGlm52,
    OpenRouterGlm52,
    VercelGlm52,
    VercelGlm52Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaudeProviderTransport {
    DirectAnthropic,
    AmbientChatBridge,
    AnthropicPassthroughBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaudeProviderProfile {
    pub(crate) kind: ClaudeProviderProfileKind,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) claude_model: &'static str,
    pub(crate) provider_model: &'static str,
    pub(crate) small_model: &'static str,
    pub(crate) base_url: Option<&'static str>,
    pub(crate) vault_label: Option<&'static str>,
    pub(crate) uses_bare_mode: bool,
    pub(crate) transport: ClaudeProviderTransport,
}

impl ClaudeProviderProfileKind {
    pub(crate) fn profile(self) -> ClaudeProviderProfile {
        match self {
            Self::ClaudePlan => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - Opus 4.8 Claude Plan",
                description: "Use Claude Code's native auth with Opus 4.8 Claude Plan.",
                claude_model: "opus",
                provider_model: "opus",
                small_model: "haiku",
                base_url: None,
                vault_label: None,
                uses_bare_mode: false,
                transport: ClaudeProviderTransport::DirectAnthropic,
            },
            Self::AmbientGlm52 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 Ambient",
                description: "Use Ambient's Claude Code endpoint with the Ambient vault key.",
                claude_model: "opus",
                provider_model: AMBIENT_DEFAULT_MODEL,
                small_model: "glm-4.7",
                base_url: Some("https://api.ambient.xyz"),
                vault_label: Some("provider/ambient_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::AmbientChatBridge,
            },
            Self::AmbientKimiK27 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - Kimi K2.7 Ambient",
                description: "Use Ambient's Kimi K2.7 Code model with the Ambient vault key.",
                claude_model: "opus",
                provider_model: AMBIENT_KIMI_K2_7_CODE_MODEL,
                small_model: AMBIENT_KIMI_K2_7_CODE_MODEL,
                base_url: Some("https://api.ambient.xyz"),
                vault_label: Some("provider/ambient_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::AmbientChatBridge,
            },
            Self::ZaiGlm52 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 Z.AI",
                description: "Experimental direct Z.AI Anthropic-compatible route; smoke test before relying on it.",
                claude_model: "opus",
                provider_model: "glm-5.2[1m]",
                small_model: "glm-4.7",
                base_url: Some("https://api.z.ai/api/anthropic"),
                vault_label: Some("provider/zai_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::DirectAnthropic,
            },
            Self::BasetenGlm52 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 Baseten",
                description: "Experimental Baseten Anthropic-compatible route; smoke test before relying on it.",
                claude_model: "opus",
                provider_model: "zai-org/GLM-5.2",
                small_model: "zai-org/GLM-5.2",
                base_url: Some("https://inference.baseten.co"),
                vault_label: Some("provider/baseten_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::DirectAnthropic,
            },
            Self::OpenRouterGlm52 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 OpenRouter",
                description: "Experimental OpenRouter Anthropic-compatible route; smoke test before relying on it.",
                claude_model: "opus",
                provider_model: "z-ai/glm-5.2",
                small_model: "z-ai/glm-5.2",
                base_url: Some("https://openrouter.ai/api"),
                vault_label: Some("provider/openrouter_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::DirectAnthropic,
            },
            Self::VercelGlm52 => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 Vercel",
                description: "Use Vercel AI Gateway's Anthropic-compatible Claude Code route with the Vercel vault key.",
                claude_model: "opus",
                provider_model: "zai/glm-5.2",
                small_model: "zai/glm-5.2-fast",
                base_url: Some("https://ai-gateway.vercel.sh"),
                vault_label: Some("provider/ai_gateway_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::AnthropicPassthroughBridge,
            },
            Self::VercelGlm52Fast => ClaudeProviderProfile {
                kind: self,
                title: "Claude Code - GLM 5.2 Fast Vercel",
                description: "Use Vercel AI Gateway's fast GLM 5.2 route with the Vercel vault key.",
                claude_model: "opus",
                provider_model: "zai/glm-5.2-fast",
                small_model: "zai/glm-5.2-fast",
                base_url: Some("https://ai-gateway.vercel.sh"),
                vault_label: Some("provider/ai_gateway_api_key"),
                uses_bare_mode: true,
                transport: ClaudeProviderTransport::AnthropicPassthroughBridge,
            },
        }
    }

    pub(crate) fn status_model_label(self) -> String {
        let profile = self.profile();
        profile
            .title
            .strip_prefix("Claude Code - ")
            .unwrap_or(profile.title)
            .to_string()
    }

    pub(crate) fn native_codex_model(self) -> Option<&'static str> {
        match self {
            Self::ClaudePlan => None,
            Self::AmbientGlm52 => Some(AMBIENT_DEFAULT_MODEL),
            Self::AmbientKimiK27 => Some(AMBIENT_KIMI_K2_7_CODE_MODEL),
            Self::ZaiGlm52 => Some(ZAI_DEFAULT_MODEL),
            Self::BasetenGlm52 => Some(BASETEN_DEFAULT_MODEL),
            Self::OpenRouterGlm52 => Some(OPENROUTER_DEFAULT_MODEL),
            Self::VercelGlm52 => Some(VERCEL_DEFAULT_MODEL),
            Self::VercelGlm52Fast => Some(VERCEL_GLM_5_2_FAST_MODEL),
        }
    }

    pub(crate) fn creation_options() -> &'static [Self] {
        &[
            Self::AmbientGlm52,
            Self::AmbientKimiK27,
            Self::ZaiGlm52,
            Self::BasetenGlm52,
            Self::OpenRouterGlm52,
            Self::VercelGlm52,
            Self::VercelGlm52Fast,
            Self::ClaudePlan,
        ]
    }
}
