use gpui::{IntoElement, ParentElement};
use ui::{List, ListBulletItem, prelude::*};

/// Centralized definitions for LingCode AI plans
pub struct PlanDefinitions;

impl PlanDefinitions {
    pub fn free_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new(
                "Full native IDE, /try playground, and CLI",
            ))
            .child(ListBulletItem::new(
                "Unlimited prompts with your own AI API keys",
            ))
            .child(ListBulletItem::new("Unlimited use of external agents"))
    }

    pub fn pro_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new(
                "LingModel managed inference — unlimited prompts, fair-use limits",
            ))
            .child(ListBulletItem::new(
                "Deep Agent (server-side Agent SDK)",
            ))
            .child(ListBulletItem::new("Everything in Free, plus Pro-tier limits"))
    }

    pub fn max_pro_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new(
                "5× higher LingModel daily & monthly throughput",
            ))
            .child(ListBulletItem::new(
                "Priority inference queue and higher Deep Agent budgets",
            ))
            .child(ListBulletItem::new("Everything in Pro"))
    }
}
