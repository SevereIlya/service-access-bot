use crate::application::{AppError, AppResult};
use crate::domain::{
    DomainError, UserError,
    billing::Money,
    user::{DynUserRepository, ReferralCode, SubscriptionToken, TelegramId, User},
};
use tracing::{info, warn};
use uuid::Uuid;

/// Результат `/start`-регистрации.
#[derive(Debug)]
pub enum Registration {
    Created(User),
    Existing(User),
}

impl Registration {
    #[must_use]
    pub const fn is_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }

    #[must_use]
    pub fn into_user(self) -> User {
        match self {
            Self::Created(user) | Self::Existing(user) => user,
        }
    }
}

pub struct RegisterUserCommand {
    user_repo: DynUserRepository,
    uuid_namespace: Uuid,
    base_price: Money,
}

impl RegisterUserCommand {
    pub fn new(user_repo: DynUserRepository, uuid_namespace: Uuid, base_price: Money) -> Self {
        Self {
            user_repo,
            uuid_namespace,
            base_price,
        }
    }

    pub async fn execute(
        &self,
        telegram_id: i64,
        username: Option<String>,
        full_name: String,
    ) -> AppResult<Registration> {
        let telegram_id = TelegramId::new(telegram_id);

        if let Some(mut user) = self.user_repo.find_by_telegram_id(telegram_id).await? {
            let username_changed = user.username() != username;
            let full_name_changed = user.full_name() != full_name;

            if username_changed || full_name_changed {
                user.update_profile(username.clone(), full_name.clone());
                self.user_repo.update(&user).await?;

                info!(
                    telegram_id = %telegram_id,
                    username = ?username,
                    "Данные профиля пользователя обновлены"
                );
            }

            return Ok(Registration::Existing(user));
        }

        let user_uuid = Uuid::new_v5(&self.uuid_namespace, telegram_id.to_string().as_bytes());

        let mut attempts = 0;

        loop {
            attempts += 1;

            let ref_code = ReferralCode::generate();
            let sub_token = SubscriptionToken::generate();

            let new_user = User::new(
                telegram_id,
                user_uuid,
                username.clone(),
                full_name.clone(),
                self.base_price,
                ref_code,
                sub_token,
            );

            match self.user_repo.create(&new_user).await {
                Ok(inserted_id) => {
                    info!(
                        telegram_id = %telegram_id,
                        username = ?username,
                        "Новый пользователь зарегистрирован"
                    );

                    let mut saved_user = new_user;
                    saved_user.assign_id(inserted_id);

                    return Ok(Registration::Created(saved_user));
                }
                Err(DomainError::User(UserError::ReferralCodeCollision)) => {
                    if attempts >= 5 {
                        return Err(AppError::MaxRetriesExceeded(
                            "Превышен лимит коллизий реф-кода".into(),
                        ));
                    }
                    warn!(attempts, "Коллизия реф-кода, повторяем...");
                }
                Err(DomainError::User(UserError::AlreadyExists)) => {
                    info!("TOCTOU коллизия по telegram_id, юзер уже создан");
                    let user = self.user_repo.find_by_telegram_id(telegram_id).await?.ok_or_else(
                        || DomainError::SystemFailure("User exists but not found".into()),
                    )?;
                    return Ok(Registration::Existing(user));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        user::{UserId, UserRepository},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;
    // ==========================================
    // Моки
    // ==========================================

    struct MockUserRepository {
        existing_user: Option<User>,
        collisions_to_simulate: Mutex<u8>,
        raced_user: Option<User>,
        create_called: Mutex<bool>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            *self.create_called.lock().unwrap() = true;
            if self.raced_user.is_some() {
                return Err(DomainError::User(UserError::AlreadyExists));
            }
            let mut cols = self.collisions_to_simulate.lock().unwrap();
            if *cols > 0 {
                *cols -= 1;
                drop(cols);
                return Err(DomainError::User(UserError::ReferralCodeCollision));
            }
            Ok(UserId::new(99))
        }
        async fn update(&self, _user: &User) -> DomainResult<()> {
            Ok(())
        }
        async fn find_by_user_id(&self, _user_id: UserId) -> DomainResult<Option<User>> {
            Ok(None)
        }
        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            if *self.create_called.lock().unwrap() {
                return Ok(self.raced_user.clone());
            }
            Ok(self.existing_user.clone())
        }
        async fn find_by_subscription_token(
            &self,
            _token: &SubscriptionToken,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_subscription_token() не используется в этом юзкейсе")
        }
        async fn find_by_referral_code(&self, _code: &ReferralCode) -> DomainResult<Option<User>> {
            unreachable!("find_by_referral_code() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn setup_command(
        existing_user: Option<User>,
        collisions: u8,
        raced_user: Option<User>,
    ) -> RegisterUserCommand {
        let mock_repo = Arc::new(MockUserRepository {
            existing_user,
            collisions_to_simulate: Mutex::new(collisions),
            raced_user,
            create_called: Mutex::new(false),
        });
        RegisterUserCommand::new(mock_repo, uuid::Uuid::new_v4(), Money::new(15000).unwrap())
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_register_new_user_success() {
        let cmd = setup_command(None, 0, None);
        let registration = cmd
            .execute(123, Some("freddie".into()), "Freddie Mercury".into())
            .await
            .unwrap();

        assert!(registration.is_created());

        let user = registration.into_user();

        assert_eq!(user.id(), Some(UserId::new(99)));
        assert_eq!(user.frozen_base_price(), Money::new(15000).unwrap());
    }

    #[tokio::test]
    async fn test_returns_existing_user() {
        let mut existing_user = User::new(
            TelegramId::new(123),
            Uuid::new_v4(),
            Some("old".into()),
            "Old".into(),
            Money::new(10).unwrap(),
            ReferralCode::new("A".into()),
            SubscriptionToken::new("B".into()),
        );
        existing_user.assign_id(UserId::new(42));
        let cmd = setup_command(Some(existing_user), 0, None);
        let registration = cmd
            .execute(123, Some("freddie".into()), "Freddie Mercury".into())
            .await
            .unwrap();

        assert!(
            !registration.is_created(),
            "старый юзер не должен считаться новым"
        );
        assert_eq!(
            registration.into_user().id(),
            Some(UserId::new(42)),
            "Должен вернуть старого юзера"
        );
    }

    #[tokio::test]
    async fn test_retry_works_on_referral_collision() {
        let cmd = setup_command(None, 2, None);
        let registration = cmd
            .execute(123, Some("freddie".into()), "Freddie Mercury".into())
            .await
            .expect("Цикл должен был повторить попытку и успешно сохранить юзера");

        assert!(registration.is_created());
        assert_eq!(registration.into_user().id(), Some(UserId::new(99)));
    }

    #[tokio::test]
    async fn test_fails_after_max_retries() {
        let cmd = setup_command(None, 10, None);
        let result = cmd
            .execute(
                123,
                Some("Freddie Mercury".into()),
                "Freddie Mercury".into(),
            )
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::MaxRetriesExceeded(_)
        ));
    }

    #[tokio::test]
    async fn test_race_loser_is_existing_not_created() {
        let mut winner = User::new(
            TelegramId::new(123),
            Uuid::new_v4(),
            Some("freddie".into()),
            "Freddie Mercury".into(),
            Money::new(10).unwrap(),
            ReferralCode::new("W".into()),
            SubscriptionToken::new("W".into()),
        );
        winner.assign_id(UserId::new(7));
        let cmd = setup_command(None, 0, Some(winner));

        let registration = cmd
            .execute(123, Some("freddie".into()), "Freddie Mercury".into())
            .await
            .unwrap();

        assert!(
            !registration.is_created(),
            "проигравший гонку не должен получить реферальный бонус второй раз"
        );
        assert_eq!(registration.into_user().id(), Some(UserId::new(7)));
    }
}
