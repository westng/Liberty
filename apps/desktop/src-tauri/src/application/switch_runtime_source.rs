use crate::domain::runtime::{RuntimeComponent, RuntimeSource};

pub trait RuntimeSourcePort {
    type State;

    fn set_source(&self, component: RuntimeComponent, source: RuntimeSource) -> Result<(), String>;
    fn detect_system(&self, component: RuntimeComponent) -> Result<(), String>;
    fn reconcile_managed(&self, component: RuntimeComponent) -> Result<(), String>;
    fn load_state(&self) -> Result<Self::State, String>;
}

pub fn switch_runtime_source<Port: RuntimeSourcePort>(
    port: &Port,
    component: &str,
    source: &str,
) -> Result<Port::State, String> {
    let component = RuntimeComponent::parse_selectable(component)?;
    let source = RuntimeSource::parse(source)?;
    port.set_source(component, source)?;
    match source {
        RuntimeSource::System => port.detect_system(component)?,
        RuntimeSource::Managed => port.reconcile_managed(component)?,
    }
    port.load_state()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct FakeRuntimePort {
        events: RefCell<Vec<String>>,
    }

    impl RuntimeSourcePort for FakeRuntimePort {
        type State = Vec<String>;

        fn set_source(
            &self,
            component: RuntimeComponent,
            source: RuntimeSource,
        ) -> Result<(), String> {
            self.events.borrow_mut().push(format!(
                "set:{}:{}",
                component.as_str(),
                source.as_str()
            ));
            Ok(())
        }

        fn detect_system(&self, component: RuntimeComponent) -> Result<(), String> {
            self.events
                .borrow_mut()
                .push(format!("detect:{}", component.as_str()));
            Ok(())
        }

        fn reconcile_managed(&self, component: RuntimeComponent) -> Result<(), String> {
            self.events
                .borrow_mut()
                .push(format!("reconcile:{}", component.as_str()));
            Ok(())
        }

        fn load_state(&self) -> Result<Self::State, String> {
            self.events.borrow_mut().push("load".into());
            Ok(self.events.borrow().clone())
        }
    }

    #[test]
    fn system_source_starts_detection_without_webview() {
        let port = FakeRuntimePort::default();

        let state = switch_runtime_source(&port, "python", "system").expect("switch");

        assert_eq!(state, ["set:python:system", "detect:python", "load"]);
    }

    #[test]
    fn managed_source_reconciles_and_models_are_rejected() {
        let port = FakeRuntimePort::default();
        let state = switch_runtime_source(&port, "ffmpeg", "managed").expect("switch");
        assert_eq!(state, ["set:ffmpeg:managed", "reconcile:ffmpeg", "load"]);
        assert!(switch_runtime_source(&port, "model", "managed").is_err());
    }
}
