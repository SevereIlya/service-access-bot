use crate::application::AppResult;
use crate::domain::vpn::DynVpnConfigGenerator;

pub struct GetStubProfileQuery {
    config_generator: DynVpnConfigGenerator,
}

impl GetStubProfileQuery {
    pub fn new(config_generator: DynVpnConfigGenerator) -> Self {
        Self { config_generator }
    }

    pub fn execute(&self, messages: &[String]) -> AppResult<String> {
        Ok(self.config_generator.generate_stub(messages)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::AppError;
    use crate::domain::{
        DomainError, DomainResult,
        vpn::{Node, VpnConfigGenerator},
    };
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    struct MockGenerator {
        received: Mutex<Vec<String>>,
        fail: bool,
    }

    impl VpnConfigGenerator for MockGenerator {
        fn generate(&self, _nodes: &[&Node], _user_uuid: Uuid) -> DomainResult<String> {
            unreachable!("generate() не используется в этом запросе")
        }

        fn generate_stub(&self, messages: &[String]) -> DomainResult<String> {
            *self.received.lock().unwrap() = messages.to_vec();
            if self.fail {
                return Err(DomainError::SystemFailure("boom".into()));
            }
            Ok("stub-yaml".to_owned())
        }
    }

    fn generator(fail: bool) -> Arc<MockGenerator> {
        Arc::new(MockGenerator {
            received: Mutex::new(Vec::new()),
            fail,
        })
    }

    #[test]
    fn passes_messages_to_generator_and_returns_its_yaml() {
        let mock = generator(false);
        let query = GetStubProfileQuery::new(mock.clone());

        let yaml = query.execute(&["Подписка не активна".to_owned()]).unwrap();

        assert_eq!(yaml, "stub-yaml");
        assert_eq!(
            *mock.received.lock().unwrap(),
            vec!["Подписка не активна".to_owned()]
        );
    }

    #[test]
    fn generator_failure_becomes_app_error() {
        let query = GetStubProfileQuery::new(generator(true));

        let result = query.execute(&["msg".to_owned()]);

        assert!(matches!(
            result,
            Err(AppError::Domain(DomainError::SystemFailure(_)))
        ));
    }
}
