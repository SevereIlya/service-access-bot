pub mod commands;
pub mod queries;

pub use commands::*;
pub use queries::*;

use crate::domain::{
    billing::{DynPaymentGateway, DynTransactionRepository, Money},
    device::DynDeviceRepository,
    notification::DynNotifier,
    referral::{DynReferralRepository, ReferralPolicy},
    subscription::DynSubscriptionRepository,
    uow::DynUnitOfWork,
    user::DynUserRepository,
    vpn::{
        DynNodeRepository, DynVpnAccessRevoker, DynVpnConfigGenerator, DynVpnConnectionRepository,
        DynVpnProvisioner,
    },
};
use std::sync::Arc;
use uuid::Uuid;

pub struct UseCases {
    pub register_user: Arc<RegisterUserCommand>,
    pub start_trial: Arc<StartTrialCommand>,
    pub expire_lapsed_subscriptions: Arc<ExpireLapsedSubscriptionsCommand>,
    pub send_expiry_warnings: Arc<SendExpiryWarningsCommand>,
    pub issue_vpn_config: Arc<IssueVpnConfigCommand>,
    pub request_top_up: Arc<RequestTopUpCommand>,
    pub process_payment_webhook: Arc<ProcessPaymentWebhookCommand>,
    pub purchase_tariff: Arc<PurchaseTariffCommand>,
    pub register_device: Arc<RegisterDeviceCommand>,
    pub apply_referral: Arc<ApplyReferralCommand>,

    pub get_menu_state: Arc<GetMenuStateQuery>,
    pub get_profile: Arc<GetProfileQuery>,
    pub get_user_by_telegram_id: Arc<GetUserByTelegramIdQuery>,
    pub get_user_by_subscription_token: Arc<GetUserBySubscriptionTokenQuery>,
    pub get_purchase_quote: Arc<GetPurchaseQuoteQuery>,
    pub get_stub_profile: Arc<GetStubProfileQuery>,
    pub get_referral_overview: Arc<GetReferralOverviewQuery>,
}

impl UseCases {
    #[rustfmt::skip]
    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "composition root: линейная сборка зависимостей, дробить нечего"
    )]
    pub fn new(
        user_repo: DynUserRepository,
        subscription_repo: DynSubscriptionRepository,
        device_repo: DynDeviceRepository,
        node_repo: DynNodeRepository,
        vpn_connection_repo: DynVpnConnectionRepository,
        transaction_repo: DynTransactionRepository,
        referral_repo: DynReferralRepository,
        uow: DynUnitOfWork,
        vpn_revoker: DynVpnAccessRevoker,
        vpn_provisioner: DynVpnProvisioner,
        config_generator: DynVpnConfigGenerator,
        payment_gateway: DynPaymentGateway,
        notifier: DynNotifier,
        uuid_namespace: Uuid,
        base_price: Money,
        referral_policy: ReferralPolicy,
    ) -> Self {
        // === Commands ===
        let register_user = Arc::new(
            RegisterUserCommand::new(
                user_repo.clone(),
                uuid_namespace,
                base_price,
            )
        );
        let start_trial = Arc::new(
            StartTrialCommand::new(
                uow.clone()
            )
        );
        let expire_lapsed_subscriptions = Arc::new(
            ExpireLapsedSubscriptionsCommand::new(
                subscription_repo.clone(),
                user_repo.clone(),
                vpn_revoker,
                notifier.clone(),
            )
        );
        let send_expiry_warnings = Arc::new(
            SendExpiryWarningsCommand::new(
                subscription_repo.clone(),
                user_repo.clone(),
                notifier.clone(),
            )
        );
        let issue_vpn_config = Arc::new(
            IssueVpnConfigCommand::new(
                subscription_repo.clone(),
                node_repo,
                vpn_connection_repo,
                vpn_provisioner,
                config_generator.clone(),
            )
        );
        let request_top_up = Arc::new(
            RequestTopUpCommand::new(
                transaction_repo.clone(),
                payment_gateway.clone(),
            )
        );
        let process_payment_webhook = Arc::new(
            ProcessPaymentWebhookCommand::new(
                transaction_repo.clone(),
                payment_gateway,
                user_repo.clone(),
                notifier.clone(),
            )
        );
        let purchase_tariff = Arc::new(
            PurchaseTariffCommand::new(
                uow.clone(),
                notifier.clone(),
                referral_policy,
            )
        );
        let register_device = Arc::new(
            RegisterDeviceCommand::new(
                subscription_repo.clone(),
                device_repo.clone(),
                notifier.clone(),
            )
        );
        let apply_referral = Arc::new(
            ApplyReferralCommand::new(
                uow,
                notifier,
                referral_policy,
            )
        );

        // === Queries ===
        let get_user_by_telegram_id = Arc::new(
            GetUserByTelegramIdQuery::new(
                user_repo.clone()
            )
        );
        let get_menu_state = Arc::new(
            GetMenuStateQuery::new(
                subscription_repo.clone()
            )
        );
        let get_profile = Arc::new(
            GetProfileQuery::new(
                subscription_repo,
                transaction_repo.clone(),
                device_repo
            )
        );
        let get_user_by_subscription_token = Arc::new(
            GetUserBySubscriptionTokenQuery::new(
                user_repo
            )
        );
        let get_purchase_quote = Arc::new(
            GetPurchaseQuoteQuery::new(
                transaction_repo.clone()
            )
        );
        let get_stub_profile = Arc::new(
            GetStubProfileQuery::new(
                config_generator,
            )
        );
        let get_referral_overview = Arc::new(
            GetReferralOverviewQuery::new(
                referral_repo,
                transaction_repo,
                referral_policy,
            )
        );

        Self {
            register_user,
            start_trial,
            expire_lapsed_subscriptions,
            send_expiry_warnings,
            issue_vpn_config,
            request_top_up,
            process_payment_webhook,
            purchase_tariff,
            register_device,
            apply_referral,
            get_menu_state,
            get_profile,
            get_user_by_telegram_id,
            get_user_by_subscription_token,
            get_purchase_quote,
            get_stub_profile,
            get_referral_overview,
        }
    }
}
