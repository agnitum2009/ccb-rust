use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadTransaction {
    pub transaction_id: String,
    pub plan: crate::reload::plan::ReloadPlan,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub rollback_available: bool,
}

impl ReloadTransaction {
    pub fn new(plan: crate::reload::plan::ReloadPlan) -> Self {
        Self {
            transaction_id: uuid::Uuid::new_v4().to_string(),
            plan,
            status: "pending".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            rollback_available: false,
        }
    }

    pub fn commit(&mut self) {
        self.status = "committed".into();
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn rollback(&mut self) {
        self.status = "rolled_back".into();
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "transaction_id": self.transaction_id,
            "plan": self.plan.to_record(),
            "status": self.status,
            "created_at": self.created_at,
            "completed_at": self.completed_at,
            "rollback_available": self.rollback_available,
        })
    }
}
