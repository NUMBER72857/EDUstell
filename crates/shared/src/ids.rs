use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);
    };
}

typed_id!(UserId);
typed_id!(PlanId);
typed_id!(VaultId);
typed_id!(MilestoneId);
typed_id!(ContributionId);
typed_id!(PayoutId);
typed_id!(SchoolId);
typed_id!(AuditLogId);
