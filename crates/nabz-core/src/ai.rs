//! Provider-neutral, privacy-aware foundation for Nabz AI.
//!
//! This module intentionally has no network or secret-storage dependency yet.
//! Provider adapters can be added behind AiProvider without changing either UI.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiExecutionMode {
    Auto,
    CloudOnly,
    LocalOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProviderKind {
    Gemini,
    OpenAiCompatible,
    Ollama,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiExecutionLocation {
    Cloud,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiPrivacyMode {
    /// Do not send business data to a remote provider.
    LocalDataOnly,
    /// Cloud requests are allowed only when explicitly enabled.
    AllowCloudWithConsent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiSettings {
    pub enabled: bool,
    pub execution_mode: AiExecutionMode,
    pub privacy_mode: AiPrivacyMode,
    pub cloud_consent: bool,
    pub preferred_provider: Option<AiProviderKind>,
    pub model: Option<String>,
    /// Local or compatible endpoint. Secrets must never be stored here.
    pub endpoint: Option<String>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            execution_mode: AiExecutionMode::Auto,
            privacy_mode: AiPrivacyMode::LocalDataOnly,
            cloud_consent: false,
            preferred_provider: None,
            model: None,
            endpoint: None,
        }
    }
}

impl AiSettings {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.model.as_deref().is_some_and(|m| m.trim().is_empty()) {
            return Err(AiError::InvalidSettings("نام مدل خالی است".into()));
        }
        if self.endpoint.as_deref().is_some_and(|e| {
            let e = e.trim();
            !e.starts_with("http://") && !e.starts_with("https://")
        }) {
            return Err(AiError::InvalidSettings(
                "نشانی سرویس باید با http:// یا https:// شروع شود".into(),
            ));
        }
        if self.privacy_mode == AiPrivacyMode::LocalDataOnly
            && self.execution_mode == AiExecutionMode::CloudOnly
        {
            return Err(AiError::PrivacyPolicy(
                "حالت فقط محلی با اجرای فقط ابری سازگار نیست".into(),
            ));
        }
        if self.execution_mode == AiExecutionMode::CloudOnly && !self.cloud_consent {
            return Err(AiError::CloudConsentRequired);
        }
        Ok(())
    }

    pub fn allows_cloud(&self) -> bool {
        self.enabled
            && self.cloud_consent
            && self.privacy_mode == AiPrivacyMode::AllowCloudWithConsent
            && self.execution_mode != AiExecutionMode::LocalOnly
    }

    pub fn allows_local(&self) -> bool {
        self.enabled && self.execution_mode != AiExecutionMode::CloudOnly
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiCapability {
    Conversation,
    SalesAnalysis,
    ProfitAnalysis,
    InventoryAdvice,
    FinancialInsights,
    AnomalyAlerts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiRequest {
    pub conversation_id: Option<String>,
    pub user_text: String,
    pub capabilities: Vec<AiCapability>,
}

impl AiRequest {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.user_text.trim().is_empty() {
            return Err(AiError::InvalidRequest("متن پرسش خالی است".into()));
        }
        if self.user_text.len() > 16_000 {
            return Err(AiError::InvalidRequest("متن پرسش بیش از حد طولانی است".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiEvidence {
    pub tool_name: String,
    pub period: Option<String>,
    /// A short, already-redacted summary. Never place raw personal records here.
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiResponse {
    pub answer: String,
    pub provider: AiProviderKind,
    pub model: String,
    pub location: AiExecutionLocation,
    pub evidence: Vec<AiEvidence>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiTool {
    SalesSummary { from: String, to: String },
    ProfitLoss { from: String, to: String },
    LowStockProducts,
    InventoryValuation,
    ContactBalancesSummary,
    SalesAnomalies { from: String, to: String },
}

/// Typed allowlist for accounting operations. Deliberately has no raw SQL,
/// shell, filesystem, arbitrary URL or write operation.
impl AiTool {
    pub fn is_read_only(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiError {
    Disabled,
    CloudConsentRequired,
    PrivacyPolicy(String),
    InvalidSettings(String),
    InvalidRequest(String),
    ProviderUnavailable(String),
    ProviderFailure(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "دستیار هوشمند غیرفعال است"),
            Self::CloudConsentRequired => write!(f, "برای ارسال داده به سرویس ابری باید اجازه دهید"),
            Self::PrivacyPolicy(message)
            | Self::InvalidSettings(message)
            | Self::InvalidRequest(message)
            | Self::ProviderUnavailable(message)
            | Self::ProviderFailure(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AiError {}

/// Reference to a credential held by an OS-backed secret store.
/// The secret value itself must never be placed in AiSettings or SQLite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiSecretRef {
    pub account: String,
}

/// Secret-storage boundary. Implementations should use the operating system's
/// credential vault (Secret Service, Windows Credential Manager or macOS Keychain).
/// No plaintext-file fallback is provided by the shared core.
pub trait AiSecretStore: Send + Sync {
    fn store(&self, reference: &AiSecretRef, secret: &str) -> Result<(), AiError>;
    fn retrieve(&self, reference: &AiSecretRef) -> Result<Option<String>, AiError>;
    fn delete(&self, reference: &AiSecretRef) -> Result<(), AiError>;
}

/// Synchronous provider boundary for the first foundation release.
/// Network adapters may wrap async clients behind the frontend's async runtime later.
pub trait AiProvider: Send + Sync {
    fn kind(&self) -> AiProviderKind;
    fn location(&self) -> AiExecutionLocation;
    fn model(&self) -> &str;
    fn generate(&self, request: &AiRequest) -> Result<String, AiError>;
}

pub struct AiService<P> {
    settings: AiSettings,
    provider: Option<P>,
}

impl<P: AiProvider> AiService<P> {
    pub fn new(settings: AiSettings, provider: Option<P>) -> Result<Self, AiError> {
        settings.validate()?;
        Ok(Self { settings, provider })
    }

    pub fn settings(&self) -> &AiSettings {
        &self.settings
    }

    pub fn respond(&self, request: &AiRequest) -> Result<AiResponse, AiError> {
        if !self.settings.enabled {
            return Err(AiError::Disabled);
        }
        request.validate()?;
        let provider = self.provider.as_ref()
            .ok_or_else(|| AiError::ProviderUnavailable("هیچ ارائه‌دهنده‌ای پیکربندی نشده است".into()))?;

        match provider.location() {
            AiExecutionLocation::Local if !self.settings.allows_local() => {
                return Err(AiError::ProviderUnavailable("اجرای محلی در تنظیمات مجاز نیست".into()));
            }
            AiExecutionLocation::Cloud if !self.settings.allows_cloud() => {
                return Err(AiError::CloudConsentRequired);
            }
            _ => {}
        }

        let answer = provider.generate(request)?;
        Ok(AiResponse {
            answer,
            provider: provider.kind(),
            model: provider.model().to_owned(),
            location: provider.location(),
            evidence: Vec::new(),
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeProvider {
        location: AiExecutionLocation,
    }

    impl AiProvider for FakeProvider {
        fn kind(&self) -> AiProviderKind { AiProviderKind::Ollama }
        fn location(&self) -> AiExecutionLocation { self.location }
        fn model(&self) -> &str { "test-model" }
        fn generate(&self, _request: &AiRequest) -> Result<String, AiError> {
            Ok("پاسخ آزمایشی".into())
        }
    }

    fn request() -> AiRequest {
        AiRequest {
            conversation_id: None,
            user_text: "فروش این ماه چقدر است؟".into(),
            capabilities: vec![AiCapability::SalesAnalysis],
        }
    }

    #[test]
    fn ai_is_disabled_by_default() {
        let settings = AiSettings::default();
        assert!(!settings.enabled);
        assert!(!settings.allows_cloud());
        assert!(!settings.allows_local());
        assert!(matches!(
            AiService::<FakeProvider>::new(settings, None).unwrap().respond(&request()),
            Err(AiError::Disabled)
        ));
    }

    #[test]
    fn local_provider_works_without_cloud_consent() {
        let settings = AiSettings {
            enabled: true,
            execution_mode: AiExecutionMode::LocalOnly,
            ..AiSettings::default()
        };
        let service = AiService::new(
            settings,
            Some(FakeProvider { location: AiExecutionLocation::Local }),
        ).unwrap();
        let response = service.respond(&request()).unwrap();
        assert_eq!(response.answer, "پاسخ آزمایشی");
        assert_eq!(response.location, AiExecutionLocation::Local);
    }

    #[test]
    fn cloud_provider_is_rejected_without_explicit_consent() {
        let settings = AiSettings {
            enabled: true,
            execution_mode: AiExecutionMode::Auto,
            ..AiSettings::default()
        };
        let service = AiService::new(
            settings,
            Some(FakeProvider { location: AiExecutionLocation::Cloud }),
        ).unwrap();
        assert_eq!(service.respond(&request()), Err(AiError::CloudConsentRequired));
    }

    #[test]
    fn cloud_provider_requires_both_privacy_permission_and_consent() {
        let settings = AiSettings {
            enabled: true,
            execution_mode: AiExecutionMode::Auto,
            privacy_mode: AiPrivacyMode::AllowCloudWithConsent,
            cloud_consent: true,
            ..AiSettings::default()
        };
        let service = AiService::new(
            settings,
            Some(FakeProvider { location: AiExecutionLocation::Cloud }),
        ).unwrap();
        assert_eq!(service.respond(&request()).unwrap().location, AiExecutionLocation::Cloud);
    }

    #[test]
    fn incompatible_cloud_only_local_privacy_settings_are_rejected() {
        let settings = AiSettings {
            enabled: true,
            execution_mode: AiExecutionMode::CloudOnly,
            ..AiSettings::default()
        };
        assert!(matches!(settings.validate(), Err(AiError::PrivacyPolicy(_))));
    }

    #[test]
    fn blank_prompts_are_rejected() {
        let mut value = request();
        value.user_text = "  ".into();
        assert!(matches!(value.validate(), Err(AiError::InvalidRequest(_))));
    }

    #[test]
    fn tools_are_read_only_typed_operations() {
        let tool = AiTool::LowStockProducts;
        assert!(tool.is_read_only());
    }
}
