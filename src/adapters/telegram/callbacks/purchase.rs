use crate::domain::subscription::SubscriptionPlan;

#[derive(Debug, Clone)]
pub enum PurchaseAction {
    View(SubscriptionPlan, i32),
    Pay(SubscriptionPlan, i32),
    TopUp(SubscriptionPlan, i32),
}

impl PurchaseAction {
    pub(super) fn parse<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<Self> {
        let action = parts.next()?;
        let plan = parts.next()?.parse::<SubscriptionPlan>().ok()?;
        let devices = parts.next()?.parse::<i32>().ok()?;
        match action {
            "view" => Some(Self::View(plan, devices)),
            "pay" => Some(Self::Pay(plan, devices)),
            "topup" => Some(Self::TopUp(plan, devices)),
            _ => None,
        }
    }
}
