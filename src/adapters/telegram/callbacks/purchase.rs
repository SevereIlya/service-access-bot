use crate::domain::subscription::SubscriptionPlan;

#[derive(Debug, Clone)]
pub enum PurchaseAction {
    View(SubscriptionPlan),
    Pay(SubscriptionPlan),
}

impl PurchaseAction {
    pub(super) fn parse<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<Self> {
        let action = parts.next()?;
        let plan = parts.next()?.parse::<SubscriptionPlan>().ok()?;
        match action {
            "view" => Some(Self::View(plan)),
            "pay" => Some(Self::Pay(plan)),
            _ => None,
        }
    }
}
