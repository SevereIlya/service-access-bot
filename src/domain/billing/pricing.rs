use crate::domain::{
    BillingError, DomainResult,
    billing::Money,
    subscription::{SubscriptionDevices, SubscriptionPlan},
    user::User,
};

/// Коэффициент падения цены от срока подписки в показателе `t^(K_T + 1)`.
const K_T: f64 = -0.100;

/// Коэффициент падения цены от количества устройств в показателе `d^(K_D + 1)`.
const K_D: f64 = -0.400;

/// Итоговая цена тарифа для конкретного юзера.
///
/// Формула: `P_base * t^(K_T+1) * d^(K_D+1) * (1 - discount/100)`, где:
/// - `t` - срок подписки в днях ([`SubscriptionPlan::duration_days`]);
/// - `d` - количество устройств;
/// - `P_base` - цена за устройство за день, ЗАМОРОЖЕННАЯ за юзером при регистрации ([`User::frozen_base_price`]);
/// - `discount` - персональная скидка юзера ([`User::discount_percent`]).
///
/// # Errors
/// `PlanNotPurchasable`, если `plan` не продаётся за деньги.
pub fn calculate_price(
    plan: SubscriptionPlan,
    devices: SubscriptionDevices,
    user: &User,
) -> DomainResult<Money> {
    if !plan.is_purchasable() {
        return Err(BillingError::PlanNotPurchasable(plan.as_str().to_string()).into());
    }

    #[allow(clippy::cast_precision_loss)]
    let days_f64: f64 = plan.duration_days() as f64;
    let devices_f64: f64 = f64::from(devices.inner());
    let p_base_f64: f64 = user.frozen_base_price().as_kopecks_f64();

    let mut price: f64 = p_base_f64 * days_f64.powf(K_T + 1.0) * devices_f64.powf(K_D + 1.0);

    let discount: i32 = user.discount_percent().inner();
    if discount > 0 {
        price *= 1.0 - (f64::from(discount) / 100.0);
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let price_kopecks: i64 = price.round() as i64;

    Money::new(price_kopecks)
}

/// Процент скидки для UI. Сравнение итоговой цены с наивной линейной (`P_base * t * d`).
#[must_use]
pub fn effective_discount(
    plan: SubscriptionPlan,
    devices: SubscriptionDevices,
    user: &User,
) -> Option<f64> {
    if !plan.is_purchasable() {
        return None;
    }

    #[allow(clippy::cast_precision_loss)]
    let days_f64: f64 = plan.duration_days() as f64;
    let devices_f64: f64 = f64::from(devices.inner());
    let linear_price_f64: f64 = user.frozen_base_price().as_kopecks_f64() * days_f64 * devices_f64;

    let actual_price_f64: f64 = calculate_price(plan, devices, user).ok()?.as_kopecks_f64();

    if actual_price_f64 >= linear_price_f64 || linear_price_f64 <= 0.0 {
        return None;
    }
    Some((1.0 - actual_price_f64 / linear_price_f64) * 100.0)
}
