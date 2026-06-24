//! Mirrors Python `lib/ccbrd/services/dispatcher_runtime/restore_runtime/execution.py`.
//! 1:1 file alignment stub.

/// Restore running jobs after daemon restart
pub fn restore_running_jobs(dispatcher: &dyn Dispatcher) -> Result<Vec<JobRestoreResult>, String> {
    // Get all active items from dispatcher state
    let active_items = dispatcher.active_items();

    let mut restored_or_completed = Vec::new();

    for (target_kind, job_id) in active_items {
        match restore_current_job(dispatcher, &target_kind, &job_id) {
            Ok(Some(result)) => restored_or_completed.push(result),
            Ok(None) => continue, // Skip jobs that couldn't be restored
            Err(e) => return Err(e),
        }
    }

    Ok(restored_or_completed)
}

fn restore_current_job(
    dispatcher: &dyn Dispatcher,
    _target_kind: &TargetKind,
    job_id: &str,
) -> Result<Option<JobRestoreResult>, String> {
    // Get the current job
    let current = match dispatcher.get_job(job_id) {
        Some(job) => job,
        None => return Ok(None),
    };

    // Check if job is in running status
    if !current.is_running {
        return Ok(None);
    }

    // Create restore entry
    let _entry = RestoreEntry {
        job_id: current.job_id.clone(),
        agent_name: current.agent_name.clone(),
        provider: current.provider.clone(),
        status: "restored".to_string(),
        reason: "daemon_restart".to_string(),
        resume_capable: true,
        pending_items_count: 0,
    };

    let result = JobRestoreResult {
        job_id: current.job_id.clone(),
        restored: true,
        completed: false,
    };

    Ok(Some(result))
}

// Simplified types for dispatcher interaction

#[derive(Debug, Clone)]
pub struct Job {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub is_running: bool,
}

#[derive(Debug, Clone)]
pub struct RestoreEntry {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub status: String,
    pub reason: String,
    pub resume_capable: bool,
    pub pending_items_count: i32,
}

#[derive(Debug, Clone)]
pub struct JobRestoreResult {
    pub job_id: String,
    pub restored: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TargetKind {
    Agent,
    Tool,
}

// Trait for dispatcher interaction
pub trait Dispatcher {
    fn active_items(&self) -> Vec<(TargetKind, String)>;
    fn get_job(&self, job_id: &str) -> Option<Job>;
    fn complete(&self, job_id: &str, decision: CompletionDecision) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct CompletionDecision {
    pub terminal: bool,
    pub status: String,
    pub reason: String,
}
