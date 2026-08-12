use super::settings::AppSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeComponent {
    Python,
    Ffmpeg,
    Model,
}

impl RuntimeComponent {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "python" => Ok(Self::Python),
            "ffmpeg" => Ok(Self::Ffmpeg),
            "model" | "models" => Ok(Self::Model),
            _ => Err("不支持的运行环境组件。".into()),
        }
    }

    pub fn parse_selectable(value: &str) -> Result<Self, String> {
        match Self::parse(value)? {
            Self::Python => Ok(Self::Python),
            Self::Ffmpeg => Ok(Self::Ffmpeg),
            Self::Model => Err("模型来源由 Liberty 托管，不能切换。".into()),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Ffmpeg => "ffmpeg",
            Self::Model => "model",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSource {
    Managed,
    System,
}

impl RuntimeSource {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "managed" => Ok(Self::Managed),
            "system" => Ok(Self::System),
            _ => Err("不支持的运行环境来源。".into()),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::System => "system",
        }
    }
}

pub fn selected_source(settings: &AppSettings, component: RuntimeComponent) -> &str {
    match component {
        RuntimeComponent::Python => &settings.python_runtime_source,
        RuntimeComponent::Ffmpeg => &settings.ffmpeg_runtime_source,
        RuntimeComponent::Model => RuntimeSource::Managed.as_str(),
    }
}

pub fn install_components(settings: &AppSettings) -> Vec<RuntimeComponent> {
    let mut components = Vec::with_capacity(3);
    if settings.python_runtime_source == RuntimeSource::Managed.as_str() {
        components.push(RuntimeComponent::Python);
    }
    components.push(RuntimeComponent::Model);
    if settings.ffmpeg_runtime_source == RuntimeSource::Managed.as_str() {
        components.push(RuntimeComponent::Ffmpeg);
    }
    components
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_plan_respects_selected_sources() {
        let settings = AppSettings {
            python_runtime_source: "system".into(),
            ..AppSettings::default()
        };
        assert_eq!(
            install_components(&settings),
            vec![RuntimeComponent::Model, RuntimeComponent::Ffmpeg]
        );
    }
}
