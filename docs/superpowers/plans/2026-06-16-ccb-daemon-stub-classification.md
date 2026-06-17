# ccb-daemon stub classification

Generated: 2026-06-16

Method: Parallel explore subagents classified each Rust stub against its Python reference using `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-mapping.md`.

## services/dispatcher_runtime (batch01_dispatcher)

# Stub Classification: batch01_dispatcher

| rust_file | py_file | class | notes | complexity |
|-----------|---------|-------|-------|------------|
| `artifacts.rs` | `services/dispatcher_runtime/finalization_runtime/artifacts.py` | B | F:spill_terminal_reply_if_needed. Text-artifact spill helpers exist in `ccb-storage/src/text_artifacts.rs`, but `spill_terminal_reply_if_needed` is not implemented. | Medium |
| `callbacks.rs` | `services/dispatcher_runtime/callbacks.py` | B | F:request_callback_route; validate_nested_ask_request; register_callback_edge; validate_callback_request; delegated_parent_edge; callback_child_edge; submit_callback_continuation; repair_callback_edges; sweep_callback_timeouts; fail_callback_edge; mark_callback_done; mark_parent_message_waiting; delegated_terminal_job; persist_delegated_terminal_job. Callback edge types/stores exist in `ccb-mailbox/src/models.rs`/`stores.rs`, but the full callback routing/validation/timeout logic is missing. | Large |
| `cancellation.rs` | `services/dispatcher_runtime/cancellation.py` | A | F:cancel_job; cancel_with_decision. `cancel_job` implemented in `JobDispatcher::cancel` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Medium |
| `claims.rs` | `services/dispatcher_runtime/reply_delivery_runtime/claims.py` | B | F:claimable_reply_delivery_job_ids; claim_reply_delivery_start. Reply-delivery claim helpers not implemented; low-level `claim` exists in `ccb-mailbox/src/kernel.rs`. | Small |
| `completion.rs` | `services/dispatcher_runtime/completion.py` | A | re-export module. Snapshot tracking implemented in `ccb-completion/src/tracker.rs`. | Small |
| `constants.rs` | `services/dispatcher_runtime/reply_delivery_runtime/constants.py` | B | constants. Reply-delivery constants not implemented. | Small |
| `context.rs` | `services/dispatcher_runtime/context.py` | B | F:build_runtime_context; build_job_runtime_context. Runtime context builders not implemented. | Small |
| `decisions.rs` | `services/dispatcher_runtime/reply_delivery_runtime/decisions.py` | B | F:reply_delivery_completed_decision; reply_delivery_failed_decision. Reply-delivery terminal decisions not implemented. | Medium |
| `details.rs` | `services/dispatcher_runtime/finalization_retry_runtime/details.py` | B | F:retry_failure_detail; detail_parts; append_detail; error_message. Retry failure detail helpers not implemented. | Small |
| `execution_cleanup.rs` | `services/dispatcher_runtime/execution_cleanup.py` | B | F:cleanup_stale_execution_states; finish_stale_execution_update. Stale execution cleanup not implemented. | Small |
| `facade.rs` | `services/dispatcher_runtime/facade.py` | A | C:DispatcherFacadeMixin(watch,queue,trace,resubmit,retry,inbox,mailbox_head,ack_reply,reconcile_runtime_views). Core facade methods (`watch`, `queue`, `trace`, `submit`, `cancel`, `resubmit`, `retry`, `inbox`, `mailbox_head`, `ack_reply`) implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs`; some return placeholder JSON. | Medium |
| `facade_state.rs` | `services/dispatcher_runtime/facade_state.py` | B | C:DispatcherRuntimeState(); DispatcherRuntimeStateMixin(). No Rust equivalent found for the `DispatcherRuntimeState` / `DispatcherRuntimeStateMixin` property-bag container. | Medium |
| `failure_policy.rs` | `services/dispatcher_runtime/failure_policy.py` | B | F:normalized_error_token; failure_message_text; nonretryable_api_failure_kind; is_nonretryable_api_failure. Non-retryable API failure classification not implemented. | Medium |
| `finalization.rs` | `services/dispatcher_runtime/finalization.py` | A | re-export module. `complete_job` implemented via ccb-mailbox terminal recording. | Small |
| `finalization_retry.rs` | `services/dispatcher_runtime/finalization_retry.py` | B | re-export module. Underlying retry runtime helpers are missing. | Small |
| `formatting.rs` | `services/dispatcher_runtime/reply_delivery_runtime/formatting.py` | B | F:format_reply_delivery_body; format_heartbeat_delivery_body; format_silence_seconds. Reply/heartbeat/silence formatting not implemented. | Medium |
| `head.rs` | `services/dispatcher_runtime/reply_delivery_runtime/head.py` | B | F:rewrite_reply_head. Reply-head rewrite not implemented. | Small |
| `lifecycle_start.rs` | `services/dispatcher_runtime/lifecycle_start.py` | A | re-export module. Underlying `tick_jobs` implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `message_bureau.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau.py` | A | F:record_message_bureau_completion. Message-bureau completion recording implemented in `ccb-mailbox/src/facade_recording.rs` (`record_terminal`, `record_attempt_terminal`, `record_reply`, callback-edge handling). | Medium |
| `message_bureau_persistence.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_persistence.py` | B | F:persist_reply_decision. `persist_reply_decision` not implemented in Rust. | Small |
| `message_bureau_retry.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_retry.py` | B | F:schedule_automatic_retry; reply_decision_without_automatic_retry. Automatic retry scheduling / reply-decision-without-retry not implemented. | Medium |
| `message_bureau_retry_events.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_retry_events.py` | B | F:append_retry_event; append_retry_failed_event; append_retry_scheduled_event; append_retry_exhausted_event; append_nonretryable_failure_event. Retry event appenders not implemented. | Medium |
| `persistence.rs` | `services/dispatcher_runtime/finalization_runtime/persistence.py` | A | F:persist_terminal_completion. Terminal completion persistence maps to `ccb-mailbox/src/facade_recording.rs` terminal recording plus snapshot/completion tracking in ccb-completion. | Medium |
| `plans.rs` | `services/dispatcher_runtime/finalization_retry_runtime/plans.py` | B | F:automatic_retry_plan; retryable_failure_context. Automatic retry plan builders not implemented. | Medium |
| `polling_service.rs` | `services/dispatcher_runtime/polling_service.py` | B | F:poll_completion_updates. `poll_completion_updates` not implemented in Rust. | Medium |
| `preparation.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation.py` | B | re-export module. Underlying preparation helpers are missing. | Small |
| `preparation_head.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_head.py` | B | F:reload_reply_head; reset_stale_reply_head; resolve_existing_delivery_job. Reply-head reload/reset/resolve not implemented. | Medium |
| `preparation_message.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_message.py` | B | F:resolve_workspace_path; build_reply_delivery_request; append_reply_delivery_message; record_reply_delivery_scheduled; build_reply_delivery_job. Reply-delivery message/job construction not implemented. | Medium |
| `preparation_service.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_service.py` | B | F:prepare_reply_deliveries; prepare_agent_reply_delivery. Reply-delivery preparation orchestration not implemented; ccb-mailbox has `queue_reply_delivery` only. | Medium |
| `repair.rs` | `services/dispatcher_runtime/reply_delivery_runtime/repair.py` | B | F:repair_reply_delivery_heads; repair_agent_reply_delivery_head. Reply-delivery head repair not implemented. | Medium |
| `replies.rs` | `services/dispatcher_runtime/finalization_retry_runtime/replies.py` | B | F:with_retry_failure_reply; with_nonretryable_api_failure_reply; should_render_timeout_inspection_reply; with_timeout_inspection_reply. Retry-failure reply builders not implemented. | Medium |
| `reply_delivery.rs` | `services/dispatcher_runtime/reply_delivery.py` | B | re-export module. Underlying reply-delivery runtime helpers are missing. | Small |
| `routing.rs` | `services/dispatcher_runtime/routing.py` | A | F:validate_sender; resolve_targets; validate_targets_available; resolve_watch_target; build_watch_payload. Target validation, target resolution, and watch-target resolution implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs` and `app.rs`. | Medium |
| `runtime_state.rs` | `services/dispatcher_runtime/runtime_state.py` | B | F:sync_runtime. `sync_runtime` helper not implemented. | Small |
| `services/dispatcher_runtime/artifact_maintenance.rs` | `services/dispatcher_runtime/artifact_maintenance.py` | A | F:sweep_text_artifacts_if_due. Text artifact sweep implemented in `ccb-storage/src/text_artifacts.rs::sweep_expired_text_artifacts`. | Small |
| `services/dispatcher_runtime/callbacks.rs` | `services/dispatcher_runtime/callbacks.py` | B | F:request_callback_route; validate_nested_ask_request; register_callback_edge; validate_callback_request; delegated_parent_edge; callback_child_edge; submit_callback_continuation; repair_callback_edges; sweep_callback_timeouts; fail_callback_edge; mark_callback_done; mark_parent_message_waiting; delegated_terminal_job; persist_delegated_terminal_job. Callback edge types/stores exist in `ccb-mailbox/src/models.rs`/`stores.rs`, but the full callback routing/validation/timeout logic is missing. | Large |
| `services/dispatcher_runtime/cancellation.rs` | `services/dispatcher_runtime/cancellation.py` | A | F:cancel_job; cancel_with_decision. `cancel_job` implemented in `JobDispatcher::cancel` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Medium |
| `services/dispatcher_runtime/comms_recover.rs` | `services/dispatcher_runtime/comms_recover.py` | B | C:CommsRecoverTarget(); CommsRecoverability(to_record); _Lineage(); F:comms_recoverability_for_job; comms_recover. Comms recovery orchestration not implemented in Rust. | Large |
| `services/dispatcher_runtime/completion.rs` | `services/dispatcher_runtime/completion.py` | A | re-export module. Snapshot tracking implemented in `ccb-completion/src/tracker.rs`. | Small |
| `services/dispatcher_runtime/completion_runtime/snapshots.rs` | `services/dispatcher_runtime/completion_runtime/snapshots.py` | A | F:apply_tracker_view; resolve_tracker_timestamp. `apply_tracker_view` / timestamp resolution map to `ccb-completion/src/tracker.rs` and snapshot writer. | Small |
| `services/dispatcher_runtime/completion_runtime/terminal.rs` | `services/dispatcher_runtime/completion_runtime/terminal.py` | B | re-export module. Underlying terminal-service merge/build helpers are missing. | Small |
| `services/dispatcher_runtime/completion_runtime/terminal_service.rs` | `services/dispatcher_runtime/completion_runtime/terminal_service.py` | B | F:merge_terminal_decision; build_terminal_state. `merge_terminal_decision` / `build_terminal_state` helpers not found in Rust (models exist in ccb-completion but merge logic is missing). | Medium |
| `services/dispatcher_runtime/context.rs` | `services/dispatcher_runtime/context.py` | B | F:build_runtime_context; build_job_runtime_context. Runtime context builders not implemented. | Small |
| `services/dispatcher_runtime/execution_cleanup.rs` | `services/dispatcher_runtime/execution_cleanup.py` | B | F:cleanup_stale_execution_states; finish_stale_execution_update. Stale execution cleanup not implemented. | Small |
| `services/dispatcher_runtime/facade.rs` | `services/dispatcher_runtime/facade.py` | A | C:DispatcherFacadeMixin(watch,queue,trace,resubmit,retry,inbox,mailbox_head,ack_reply,reconcile_runtime_views). Core facade methods (`watch`, `queue`, `trace`, `submit`, `cancel`, `resubmit`, `retry`, `inbox`, `mailbox_head`, `ack_reply`) implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs`; some return placeholder JSON. | Medium |
| `services/dispatcher_runtime/facade_state.rs` | `services/dispatcher_runtime/facade_state.py` | B | C:DispatcherRuntimeState(); DispatcherRuntimeStateMixin(). No Rust equivalent found for the `DispatcherRuntimeState` / `DispatcherRuntimeStateMixin` property-bag container. | Medium |
| `services/dispatcher_runtime/failure_policy.rs` | `services/dispatcher_runtime/failure_policy.py` | B | F:normalized_error_token; failure_message_text; nonretryable_api_failure_kind; is_nonretryable_api_failure. Non-retryable API failure classification not implemented. | Medium |
| `services/dispatcher_runtime/finalization.rs` | `services/dispatcher_runtime/finalization.py` | A | re-export module. `complete_job` implemented via ccb-mailbox terminal recording. | Small |
| `services/dispatcher_runtime/finalization_retry.rs` | `services/dispatcher_runtime/finalization_retry.py` | B | re-export module. Underlying retry runtime helpers are missing. | Small |
| `services/dispatcher_runtime/finalization_retry_runtime/details.rs` | `services/dispatcher_runtime/finalization_retry_runtime/details.py` | B | F:retry_failure_detail; detail_parts; append_detail; error_message. Retry failure detail helpers not implemented. | Small |
| `services/dispatcher_runtime/finalization_retry_runtime/models.rs` | `services/dispatcher_runtime/finalization_retry_runtime/models.py` | B | C:AutomaticRetryPlan(); RetryableFailureContext(). `AutomaticRetryPlan` / `RetryableFailureContext` models not implemented. | Small |
| `services/dispatcher_runtime/finalization_retry_runtime/plans.rs` | `services/dispatcher_runtime/finalization_retry_runtime/plans.py` | B | F:automatic_retry_plan; retryable_failure_context. Automatic retry plan builders not implemented. | Medium |
| `services/dispatcher_runtime/finalization_retry_runtime/policy.rs` | `services/dispatcher_runtime/finalization_retry_runtime/policy.py` | B | F:safe_int; retryable_reasons; retryable_runtime_reasons; policy_bool; provider_supports_resume; is_retryable_failure. Retry policy / provider resume support helpers not implemented. | Medium |
| `services/dispatcher_runtime/finalization_retry_runtime/replies.rs` | `services/dispatcher_runtime/finalization_retry_runtime/replies.py` | B | F:with_retry_failure_reply; with_nonretryable_api_failure_reply; should_render_timeout_inspection_reply; with_timeout_inspection_reply. Retry-failure reply builders not implemented. | Medium |
| `services/dispatcher_runtime/finalization_runtime/artifacts.rs` | `services/dispatcher_runtime/finalization_runtime/artifacts.py` | B | F:spill_terminal_reply_if_needed. Text-artifact spill helpers exist in `ccb-storage/src/text_artifacts.rs`, but `spill_terminal_reply_if_needed` is not implemented. | Medium |
| `services/dispatcher_runtime/finalization_runtime/message_bureau.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau.py` | A | F:record_message_bureau_completion. Message-bureau completion recording implemented in `ccb-mailbox/src/facade_recording.rs` (`record_terminal`, `record_attempt_terminal`, `record_reply`, callback-edge handling). | Medium |
| `services/dispatcher_runtime/finalization_runtime/message_bureau_persistence.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_persistence.py` | B | F:persist_reply_decision. `persist_reply_decision` not implemented in Rust. | Small |
| `services/dispatcher_runtime/finalization_runtime/message_bureau_retry.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_retry.py` | B | F:schedule_automatic_retry; reply_decision_without_automatic_retry. Automatic retry scheduling / reply-decision-without-retry not implemented. | Medium |
| `services/dispatcher_runtime/finalization_runtime/message_bureau_retry_events.rs` | `services/dispatcher_runtime/finalization_runtime/message_bureau_retry_events.py` | B | F:append_retry_event; append_retry_failed_event; append_retry_scheduled_event; append_retry_exhausted_event; append_nonretryable_failure_event. Retry event appenders not implemented. | Medium |
| `services/dispatcher_runtime/finalization_runtime/persistence.rs` | `services/dispatcher_runtime/finalization_runtime/persistence.py` | A | F:persist_terminal_completion. Terminal completion persistence maps to `ccb-mailbox/src/facade_recording.rs` terminal recording plus snapshot/completion tracking in ccb-completion. | Medium |
| `services/dispatcher_runtime/finalization_runtime/service.rs` | `services/dispatcher_runtime/finalization_runtime/service.py` | A | F:complete_job. High-level `complete_job` orchestration implemented in `ccb-mailbox/src/bureau.rs` and `ccb-mailbox/src/facade_recording.rs` (`record_terminal`, `record_attempt_terminal`, `record_reply`). | Small |
| `services/dispatcher_runtime/lifecycle.rs` | `services/dispatcher_runtime/lifecycle.py` | A | F:submit_jobs; resubmit_message; retry_attempt. `submit_jobs` -> `JobDispatcher::submit`; `tick_jobs` -> `JobDispatcher::tick`; retry/resubmit scaffolding partially present via ccb-mailbox and dispatcher stubs. | Medium |
| `services/dispatcher_runtime/lifecycle_start.rs` | `services/dispatcher_runtime/lifecycle_start.py` | A | re-export module. Underlying `tick_jobs` implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/lifecycle_start_runtime/models.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/models.py` | B | C:QueuedTargetSlot(requires_runtime_sync). `QueuedTargetSlot` data model not implemented. | Small |
| `services/dispatcher_runtime/lifecycle_start_runtime/queue.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/queue.py` | A | F:start_next_queued_job. `start_next_queued_job` subsumed by `JobDispatcher::tick`. | Medium |
| `services/dispatcher_runtime/lifecycle_start_runtime/recovery.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/recovery.py` | B | re-export module. Underlying recovery_runtime helpers are missing. | Small |
| `services/dispatcher_runtime/lifecycle_start_runtime/recovery_runtime/slots.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/recovery_runtime/slots.py` | B | F:refresh_slot_runtime_for_start; iter_runnable_agent_slots. Slot runtime refresh / runnable iteration not implemented. | Medium |
| `services/dispatcher_runtime/lifecycle_start_runtime/recovery_runtime/support.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/recovery_runtime/support.py` | B | F:provider_supports_resume; can_attempt_runtime_recovery. Runtime recovery support helpers not implemented. | Small |
| `services/dispatcher_runtime/lifecycle_start_runtime/start.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/start.py` | B | F:write_running_snapshot; start_running_job; should_start_execution. Running-job start helpers (`start_running_job`, `should_start_execution`, snapshot write) not found in Rust. | Medium |
| `services/dispatcher_runtime/lifecycle_start_runtime/tick.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/tick.py` | A | F:iter_runnable_slots; tick_jobs. `tick_jobs` / runnable-slot promotion implemented in `JobDispatcher::tick` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/polling.rs` | `services/dispatcher_runtime/polling.py` | A | re-export module. Polling maps to completion tracker tick in `ccb-completion/src/tracker.rs`. | Small |
| `services/dispatcher_runtime/polling_service.rs` | `services/dispatcher_runtime/polling_service.py` | B | F:poll_completion_updates. `poll_completion_updates` not implemented in Rust. | Medium |
| `services/dispatcher_runtime/records.rs` | `services/dispatcher_runtime/records.py` | A | F:get_job; latest_for_agent; append_job; append_event; rebuild_dispatcher_state. Job get/latest/append and rebuild covered by `JobDispatcher` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`; event appending lives in ccb-mailbox stores. | Medium |
| `services/dispatcher_runtime/reply_delivery.rs` | `services/dispatcher_runtime/reply_delivery.py` | B | re-export module. Underlying reply-delivery runtime helpers are missing. | Small |
| `services/dispatcher_runtime/reply_delivery_runtime/claims.rs` | `services/dispatcher_runtime/reply_delivery_runtime/claims.py` | B | F:claimable_reply_delivery_job_ids; claim_reply_delivery_start. Reply-delivery claim helpers not implemented; low-level `claim` exists in `ccb-mailbox/src/kernel.rs`. | Small |
| `services/dispatcher_runtime/reply_delivery_runtime/common.rs` | `services/dispatcher_runtime/reply_delivery_runtime/common.py` | B | F:head_reply_event; project_id_for_agent; reply_delivery_inbound_event_id; reply_delivery_reply_id; is_reply_delivery_job; head_reply_id. Reply-delivery common helpers (head reply event, IDs) not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/constants.rs` | `services/dispatcher_runtime/reply_delivery_runtime/constants.py` | B | constants. Reply-delivery constants not implemented. | Small |
| `services/dispatcher_runtime/reply_delivery_runtime/decisions.rs` | `services/dispatcher_runtime/reply_delivery_runtime/decisions.py` | B | F:reply_delivery_completed_decision; reply_delivery_failed_decision. Reply-delivery terminal decisions not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/formatting.rs` | `services/dispatcher_runtime/reply_delivery_runtime/formatting.py` | B | F:format_reply_delivery_body; format_heartbeat_delivery_body; format_silence_seconds. Reply/heartbeat/silence formatting not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/head.rs` | `services/dispatcher_runtime/reply_delivery_runtime/head.py` | B | F:rewrite_reply_head. Reply-head rewrite not implemented. | Small |
| `services/dispatcher_runtime/reply_delivery_runtime/preparation.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation.py` | B | re-export module. Underlying preparation helpers are missing. | Small |
| `services/dispatcher_runtime/reply_delivery_runtime/preparation_head.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_head.py` | B | F:reload_reply_head; reset_stale_reply_head; resolve_existing_delivery_job. Reply-head reload/reset/resolve not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/preparation_message.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_message.py` | B | F:resolve_workspace_path; build_reply_delivery_request; append_reply_delivery_message; record_reply_delivery_scheduled; build_reply_delivery_job. Reply-delivery message/job construction not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/preparation_service.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_service.py` | B | F:prepare_reply_deliveries; prepare_agent_reply_delivery. Reply-delivery preparation orchestration not implemented; ccb-mailbox has `queue_reply_delivery` only. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/repair.rs` | `services/dispatcher_runtime/reply_delivery_runtime/repair.py` | B | F:repair_reply_delivery_heads; repair_agent_reply_delivery_head. Reply-delivery head repair not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/start_completion.rs` | `services/dispatcher_runtime/reply_delivery_runtime/start_completion.py` | B | F:complete_reply_delivery_after_start. Reply-delivery start completion not implemented. | Medium |
| `services/dispatcher_runtime/reply_delivery_runtime/terminal.rs` | `services/dispatcher_runtime/reply_delivery_runtime/terminal.py` | B | F:resolve_reply_delivery_terminal. Reply-delivery terminal resolution not implemented. | Medium |
| `services/dispatcher_runtime/restore.rs` | `services/dispatcher_runtime/restore.py` | B | re-export module. Underlying restore runtime helpers are missing. | Small |
| `services/dispatcher_runtime/restore_runtime/execution.rs` | `services/dispatcher_runtime/restore_runtime/execution.py` | B | F:restore_running_jobs. Rust stub has a simplified `restore_running_jobs`; full restore logic from Python is missing. | Medium |
| `services/dispatcher_runtime/restore_runtime/reporting.rs` | `services/dispatcher_runtime/restore_runtime/reporting.py` | B | F:build_last_restore_report. `build_last_restore_report` not implemented; restore models exist in `rust/crates/ccb-daemon/src/models/restore.rs`. | Small |
| `services/dispatcher_runtime/routing.rs` | `services/dispatcher_runtime/routing.py` | A | F:validate_sender; resolve_targets; validate_targets_available; resolve_watch_target; build_watch_payload. Target validation, target resolution, and watch-target resolution implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs` and `app.rs`. | Medium |
| `services/dispatcher_runtime/runtime_state.rs` | `services/dispatcher_runtime/runtime_state.py` | B | F:sync_runtime. `sync_runtime` helper not implemented. | Small |
| `services/dispatcher_runtime/shutdown.rs` | `services/dispatcher_runtime/shutdown.py` | B | F:terminate_nonterminal_jobs. Non-terminal job termination not implemented in dispatcher.rs; app shutdown exists in `app.rs` but not this helper. | Medium |
| `services/dispatcher_runtime/state.rs` | `services/dispatcher_runtime/state.py` | A | C:DispatcherState(rebuild). Core `DispatcherState` implemented in `rust/crates/ccb-daemon/src/services/dispatcher.rs` (rebuild, job_index, active_jobs, queues). | Medium |
| `services/dispatcher_runtime/state_active.rs` | `services/dispatcher_runtime/state_active.py` | A | C:DispatcherStateActiveMixin(mark_active_for,clear_active_for,active_job_for,active_items,slots). Subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/state_agents.rs` | `services/dispatcher_runtime/state_agents.py` | A | C:DispatcherStateAgentMixin(agent_for_job,queue_depth,has_outstanding,enqueue,pop_next,remove_queued,mark_active,clear_active,active_job). Subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/state_common.rs` | `services/dispatcher_runtime/state_common.py` | A | C:TargetQueue(clear,push,pop,remove). `TargetQueue` logic subsumed by `DispatcherState` queue HashMap in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/state_index.rs` | `services/dispatcher_runtime/state_index.py` | A | C:DispatcherStateIndexMixin(target_for_job,remember_job,record). Job index helpers subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/state_queue.rs` | `services/dispatcher_runtime/state_queue.py` | A | C:DispatcherStateQueueMixin(queue_depth_for,has_outstanding_for,enqueue_for,pop_next_for,queued_items_for,remove_queued_for). Queue-depth/enqueue/pop helpers subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `services/dispatcher_runtime/submission.rs` | `services/dispatcher_runtime/submission.py` | A | re-export module. Core submission implemented in `JobDispatcher::submit` and ccb-mailbox. | Small |
| `services/dispatcher_runtime/submission_models.rs` | `services/dispatcher_runtime/submission_models.py` | B | C:_JobDraft(); C:_SubmissionPlan(). Draft/plan models not implemented. | Small |
| `services/dispatcher_runtime/submission_recording.rs` | `services/dispatcher_runtime/submission_recording.py` | A | F:_append_submission_job; _build_job_record; _enqueue_submitted_job; _submit_plan. Job enqueue/submission persistence implemented in `JobDispatcher::submit` and ccb-mailbox. | Medium |
| `services/dispatcher_runtime/submission_service.rs` | `services/dispatcher_runtime/submission_service.py` | B | F:_ensure_agent_target_ready; _latest_attempts_by_agent; _plan_agent_submission; _plan_message_resubmission; _resolve_retry_attempt. Submission planning helpers not implemented. | Medium |
| `services/dispatcher_runtime/visible_reply.rs` | `services/dispatcher_runtime/visible_reply.py` | B | C:VisibleReply(); F:visible_reply_for_job. VisibleReply logic not implemented in Rust. | Medium |
| `slots.rs` | `services/dispatcher_runtime/lifecycle_start_runtime/recovery_runtime/slots.py` | B | F:refresh_slot_runtime_for_start; iter_runnable_agent_slots. Slot runtime refresh / runnable iteration not implemented. | Medium |
| `snapshots.rs` | `services/dispatcher_runtime/completion_runtime/snapshots.py` | A | F:apply_tracker_view; resolve_tracker_timestamp. `apply_tracker_view` / timestamp resolution map to `ccb-completion/src/tracker.rs` and snapshot writer. | Small |
| `start_completion.rs` | `services/dispatcher_runtime/reply_delivery_runtime/start_completion.py` | B | F:complete_reply_delivery_after_start. Reply-delivery start completion not implemented. | Medium |
| `state_active.rs` | `services/dispatcher_runtime/state_active.py` | A | C:DispatcherStateActiveMixin(mark_active_for,clear_active_for,active_job_for,active_items,slots). Subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `state_agents.rs` | `services/dispatcher_runtime/state_agents.py` | A | C:DispatcherStateAgentMixin(agent_for_job,queue_depth,has_outstanding,enqueue,pop_next,remove_queued,mark_active,clear_active,active_job). Subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `state_common.rs` | `services/dispatcher_runtime/state_common.py` | A | C:TargetQueue(clear,push,pop,remove). `TargetQueue` logic subsumed by `DispatcherState` queue HashMap in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `state_index.rs` | `services/dispatcher_runtime/state_index.py` | A | C:DispatcherStateIndexMixin(target_for_job,remember_job,record). Job index helpers subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `state_queue.rs` | `services/dispatcher_runtime/state_queue.py` | A | C:DispatcherStateQueueMixin(queue_depth_for,has_outstanding_for,enqueue_for,pop_next_for,queued_items_for,remove_queued_for). Queue-depth/enqueue/pop helpers subsumed by `DispatcherState` in `rust/crates/ccb-daemon/src/services/dispatcher.rs`. | Small |
| `submission.rs` | `services/dispatcher_runtime/submission.py` | A | re-export module. Core submission implemented in `JobDispatcher::submit` and ccb-mailbox. | Small |
| `submission_models.rs` | `services/dispatcher_runtime/submission_models.py` | B | C:_JobDraft(); C:_SubmissionPlan(). Draft/plan models not implemented. | Small |
| `submission_recording.rs` | `services/dispatcher_runtime/submission_recording.py` | A | F:_append_submission_job; _build_job_record; _enqueue_submitted_job; _submit_plan. Job enqueue/submission persistence implemented in `JobDispatcher::submit` and ccb-mailbox. | Medium |
| `submission_service.rs` | `services/dispatcher_runtime/submission_service.py` | B | F:_ensure_agent_target_ready; _latest_attempts_by_agent; _plan_agent_submission; _plan_message_resubmission; _resolve_retry_attempt. Submission planning helpers not implemented. | Medium |
| `terminal.rs` | `services/dispatcher_runtime/reply_delivery_runtime/terminal.py` | B | F:resolve_reply_delivery_terminal. Reply-delivery terminal resolution not implemented. | Medium |
| `terminal_service.rs` | `services/dispatcher_runtime/completion_runtime/terminal_service.py` | B | F:merge_terminal_decision; build_terminal_state. `merge_terminal_decision` / `build_terminal_state` helpers not found in Rust (models exist in ccb-completion but merge logic is missing). | Medium |
| `visible_reply.rs` | `services/dispatcher_runtime/visible_reply.py` | B | C:VisibleReply(); F:visible_reply_for_job. VisibleReply logic not implemented in Rust. | Medium |


## services/project_namespace_runtime (batch02_project_namespace)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| additive_patch.rs | services/project_namespace_runtime/additive_patch.py | B | Re-export stub; underlying `apply_additive_patch`/`apply_reload_patch` and preservation helpers are not implemented elsewhere. | Small |
| additive_patch_agents.rs | services/project_namespace_runtime/additive_patch_agents.py | B | `append_agent_panes` and window-agent appending logic missing; tmux split/identity primitives exist in `ccb-terminal` but orchestration is absent. | Medium |
| additive_patch_apply.rs | services/project_namespace_runtime/additive_patch_apply.py | B | Full patch-orchestration (`apply_additive_patch`, `apply_reload_patch`, `NamespacePatchApplyResult`) missing. Reload transaction logic in `reload_transaction.rs` mutates registry/namespace state, not tmux panes. | Large |
| additive_patch_namespace.rs | services/project_namespace_runtime/additive_patch_namespace.py | B | `ready_namespace_or_blocked` and namespace readiness checks missing. | Small |
| additive_patch_preservation.rs | services/project_namespace_runtime/additive_patch_preservation.py | B | Preserved agent pane snapshot/validation logic missing. | Small |
| additive_patch_validation.rs | services/project_namespace_runtime/additive_patch_validation.py | B | Additive patch step/target validation missing; only loosely related scope logic exists in `reload_patch.rs`. | Medium |
| additive_patch_windows.rs | services/project_namespace_runtime/additive_patch_windows.py | B | `create_new_windows`, sidebar/agent/tool window materialization missing; layout splitting primitives exist in `ccb-terminal`. | Large |
| controller.rs | services/project_namespace_runtime/controller.py | B | `ProjectNamespaceController` (ensure/destroy/reflow/apply patch) missing. A simpler load/mount controller exists in `services/project_namespace.rs` but does not cover these operations. | Medium |
| controller_state.rs | services/project_namespace_runtime/controller_state.py | B | `ProjectNamespaceControllerState` and mixin missing. | Small |
| destroy.rs | services/project_namespace_runtime/destroy.py | B | `destroy_project_namespace` (kill server, persist destroyed state, emit event) missing. | Small |
| ensure.rs | services/project_namespace_runtime/ensure.py | B | `ensure_project_namespace` orchestration missing. `supervisor_runtime/namespace.rs` has a kwargs-based wrapper with a different interface. | Medium |
| ensure_context.rs | services/project_namespace_runtime/ensure_context.py | B | Stub contains partial `NamespaceEnsureContext` scaffolding, but `Backend`/`StateStore` are placeholders and liveness checks return false; not functionally complete. | Medium |
| ensure_identity.rs | services/project_namespace_runtime/ensure_identity.py | B | `prepare_namespace_root_pane` / `apply_namespace_identity` missing; `apply_ccb_pane_identity` exists in `ccb-terminal`. | Medium |
| ensure_state.rs | services/project_namespace_runtime/ensure_state.py | B | `build_created_namespace`, `persist_refreshed_namespace`, force-recreate helpers missing. | Medium |
| materialize_topology.rs | services/project_namespace_runtime/materialize_topology.py | B | Full topology materialization, sidebar sync, pane discovery, and recreate-reason logic missing. | Large |
| patch_validation_scope.rs | services/project_namespace_runtime/patch_validation_scope.py | B | Step scope/identity proof validation missing. | Medium |
| patch_validation_steps.rs | services/project_namespace_runtime/patch_validation_steps.py | B | Patch step accessors (`planned_create_windows`, `planned_agent_targets`, etc.) missing. | Small |
| reflow.rs | services/project_namespace_runtime/reflow.py | B | `reflow_project_workspace` missing; `supervisor_runtime/namespace.rs` has a different kwargs-based wrapper. | Medium |
| remove_patch_agents.rs | services/project_namespace_runtime/remove_patch_agents.py | B | `remove_agent_panes` and window/agent removal orchestration missing. | Medium |
| remove_patch_tools.rs | services/project_namespace_runtime/remove_patch_tools.py | B | `remove_tool_windows` missing. | Small |
| services/project_namespace_runtime/additive_patch.rs | services/project_namespace_runtime/additive_patch.py | B | Same as root-level `additive_patch.rs`: re-export stub with no real implementation. | Small |
| services/project_namespace_runtime/additive_patch_agents.rs | services/project_namespace_runtime/additive_patch_agents.py | B | Same as root-level stub; agent appending orchestration missing. | Medium |
| services/project_namespace_runtime/additive_patch_apply.rs | services/project_namespace_runtime/additive_patch_apply.py | B | Same as root-level stub; full patch apply orchestration missing. | Large |
| services/project_namespace_runtime/additive_patch_namespace.rs | services/project_namespace_runtime/additive_patch_namespace.py | B | Same as root-level stub; namespace readiness check missing. | Small |
| services/project_namespace_runtime/additive_patch_preservation.rs | services/project_namespace_runtime/additive_patch_preservation.py | B | Same as root-level stub; preservation logic missing. | Small |
| services/project_namespace_runtime/additive_patch_validation.rs | services/project_namespace_runtime/additive_patch_validation.py | B | Same as root-level stub; validation logic missing. | Medium |
| services/project_namespace_runtime/additive_patch_windows.rs | services/project_namespace_runtime/additive_patch_windows.py | B | Same as root-level stub; window materialization missing. | Large |
| services/project_namespace_runtime/backend.rs | services/project_namespace_runtime/backend.py | B | Minimal placeholder `Backend`/`BackendFactory`; real tmux backend primitives exist in `ccb-terminal` and `terminal_adapter.rs`, but the namespace-specific backend wrapper is not implemented. | Medium |
| services/project_namespace_runtime/controller.rs | services/project_namespace_runtime/controller.py | B | Same as root-level stub; full controller missing. | Medium |
| services/project_namespace_runtime/controller_state.rs | services/project_namespace_runtime/controller_state.py | B | Same as root-level stub; state struct/mixin missing. | Small |
| services/project_namespace_runtime/destroy.rs | services/project_namespace_runtime/destroy.py | B | Same as root-level stub; destroy flow missing. | Small |
| services/project_namespace_runtime/ensure.rs | services/project_namespace_runtime/ensure.py | B | Same as root-level stub; ensure orchestration missing. | Medium |
| services/project_namespace_runtime/ensure_context.rs | services/project_namespace_runtime/ensure_context.py | B | Partial context scaffolding exists in this stub, but backend/state integrations are placeholders. Same root-level stub is empty. | Medium |
| services/project_namespace_runtime/ensure_identity.rs | services/project_namespace_runtime/ensure_identity.py | B | Same as root-level stub; identity application orchestration missing. | Medium |
| services/project_namespace_runtime/ensure_state.rs | services/project_namespace_runtime/ensure_state.py | B | Same as root-level stub; state builders missing. | Medium |
| services/project_namespace_runtime/materialize_topology.rs | services/project_namespace_runtime/materialize_topology.py | B | Same as root-level stub; topology materialization missing. | Large |
| services/project_namespace_runtime/models.rs | services/project_namespace_runtime/models.py | B | `ProjectNamespace`/`ProjectNamespaceDestroySummary` missing. A simpler `ProjectNamespace` struct exists in `services/project_namespace.rs` but lacks the same fields/behavior. | Small |
| services/project_namespace_runtime/patch_validation_scope.rs | services/project_namespace_runtime/patch_validation_scope.py | B | Same as root-level stub; scope validation missing. | Medium |
| services/project_namespace_runtime/patch_validation_steps.rs | services/project_namespace_runtime/patch_validation_steps.py | B | Same as root-level stub; step accessors missing. | Small |
| services/project_namespace_runtime/patch_validation_targets.rs | services/project_namespace_runtime/patch_validation_targets.py | B | `removed_agent_targets` missing. | Small |
| services/project_namespace_runtime/records.rs | services/project_namespace_runtime/records.py | B | Only `normalized_layout_signature` is present in the stub; state/event builders (`build_active_state`, `build_created_event`, etc.) are missing. | Medium |
| services/project_namespace_runtime/reflow.rs | services/project_namespace_runtime/reflow.py | B | Same as root-level stub; reflow orchestration missing. | Medium |
| services/project_namespace_runtime/remove_patch_agents.rs | services/project_namespace_runtime/remove_patch_agents.py | B | Same as root-level stub; agent removal orchestration missing. | Medium |
| services/project_namespace_runtime/remove_patch_tools.rs | services/project_namespace_runtime/remove_patch_tools.py | B | Same as root-level stub; tool window removal missing. | Small |
| services/project_namespace_runtime/sidebar_helper.rs | services/project_namespace_runtime/sidebar_helper.py | B | Same as root-level stub; sidebar binary resolution/respawn args missing. Release builder builds `ccb-agent-sidebar` but does not resolve it at runtime. | Medium |
| services/project_namespace_runtime/slot_replacement.rs | services/project_namespace_runtime/slot_replacement.py | B | Same as root-level stub; project slot recovery context and relabeling missing. | Medium |
| services/project_namespace_runtime/topology_plan.rs | services/project_namespace_runtime/topology_plan.py | B | `NamespaceTopologyPlan` builders missing. `reload_additive_agents.rs::build_namespace_topology` builds a different `Vec<TopologyWindow>` model. | Medium |
| sidebar_helper.rs | services/project_namespace_runtime/sidebar_helper.py | B | Same as services-level `sidebar_helper.rs`; sidebar helper resolution missing. | Medium |
| slot_replacement.rs | services/project_namespace_runtime/slot_replacement.py | B | Same as services-level `slot_replacement.rs`; slot recovery missing. | Medium |
| topology_plan.rs | services/project_namespace_runtime/topology_plan.py | B | Same as services-level `topology_plan.rs`; topology plan builders missing. | Medium |


## supervision (batch03_supervision)

| rust_file | py_file | class | notes | complexity |
| --- | --- | --- | --- | --- |
| cmd_slot.rs | supervision/cmd_slot.py | B | Genuinely missing. Python has `reconcile_cmd_slot`, `replace_cmd_slot_locally`, `request_cmd_workspace_reflow`, `split_before_anchor_pane`, etc. No equivalent Rust implementation; root stub is empty. | Large |
| events.rs | supervision/mount_runtime/events.py | B | Missing `record_mount_started/failed/superseded/succeeded` event helpers. No matching Rust implementation. | Small |
| failure.rs | supervision/mount_runtime/failure.py | B | Missing `persist_mount_failure`. No matching Rust implementation. | Small |
| loop_actions.rs | supervision/loop_actions.py | B | Missing `ensure_agent_mounted`, `recover_agent_runtime`, `persist_mount_failure` wrappers. No matching Rust implementation. | Small |
| loop_context.rs | supervision/loop_context.py | B | Missing `RuntimeSupervisionContext` dataclass and `build_runtime_supervision_context`. No matching Rust implementation. | Small |
| loop_helpers.rs | supervision/loop.py | B | Missing `RuntimeSupervisionLoop` class and `reconcile_once`/`_reconcile_agent`. Rust `supervision/loop_runner.rs` has a different `SupervisionLoop`, not this one. | Medium |
| loop_runtime.rs | supervision/loop_runtime.py | B | Missing `resolved_runtime`, `align_runtime_authority`, `runtime_requires_mount`, `runtime_requires_recovery`, `should_reflow_project_namespace`, etc. No matching Rust implementation. | Medium |
| recovery_context.rs | supervision/recovery_context.py | B | Missing `RecoveryContext` dataclass and `build_recovery_context`. No matching Rust implementation. | Small |
| recovery_events.rs | supervision/recovery_events.py | B | Missing `append_recovery_event`. No matching Rust implementation. | Small |
| recovery_transitions.rs | supervision/recovery_transitions.py | A | Root stub only. Real implementation exists at `rust/crates/ccb-daemon/src/supervision/recovery_transitions.rs`. | - |
| starting.rs | supervision/mount_runtime/starting.py | B | Missing `build_starting_runtime`, `authority_adopt_required`. No matching Rust implementation. | Small |
| store.rs | supervision/store.py | B | Python `SupervisionEventStore`/`SupervisionEvent` not implemented. Rust `supervision/store.rs` has a different `SupervisionStore`/`SupervisionRecord`. | Small |
| supervision/cmd_slot.rs | supervision/cmd_slot.py | B | Genuinely missing. Same as root `cmd_slot.rs`; no implementation in either location. | Large |
| supervision/loop_.rs | supervision/loop.py | B | Missing `RuntimeSupervisionLoop`. Same as `loop_helpers.rs` mapping. | Medium |
| supervision/loop_actions.rs | supervision/loop_actions.py | B | Missing loop action wrappers. No matching Rust implementation. | Small |
| supervision/loop_context.rs | supervision/loop_context.py | B | Missing `RuntimeSupervisionContext`. No matching Rust implementation. | Small |
| supervision/loop_runtime.rs | supervision/loop_runtime.py | B | Missing all loop runtime helpers. No matching Rust implementation. | Medium |
| supervision/mount_runtime/events.rs | supervision/mount_runtime/events.py | B | Missing mount event recorders. No matching Rust implementation. | Small |
| supervision/mount_runtime/failure.rs | supervision/mount_runtime/failure.py | B | Missing `persist_mount_failure`. No matching Rust implementation. | Small |
| supervision/mount_runtime/service.rs | supervision/mount_runtime/service.py | B | Missing `ensure_mounted` and `stabilize_superseded_runtime`. This is the core mount orchestrator (~246 LOC in Python). No matching Rust implementation. | Large |
| supervision/mount_runtime/starting.rs | supervision/mount_runtime/starting.py | B | Missing `build_starting_runtime`. No matching Rust implementation. | Small |
| supervision/mount_runtime/transitions.rs | supervision/mount_runtime/transitions.py | B | Missing `mount_or_reflow`, `start_mount_attempt`, `persist_mount_*` helpers, `SUCCESS_RUNTIME_HEALTHS`. No matching Rust implementation. | Medium |
| supervision/recovery_context.rs | supervision/recovery_context.py | B | Missing `RecoveryContext`. No matching Rust implementation. | Small |
| supervision/recovery_events.rs | supervision/recovery_events.py | B | Missing `append_recovery_event`. No matching Rust implementation. | Small |
| supervision/recovery_transitions.rs | supervision/recovery_transitions.py | A | Contains a real implementation of `start_recovery`, `attempt_recovery_action`, `mark_recovery_*`, and `SUCCESS_RUNTIME_HEALTHS` that matches the Python logic. Uses local `AgentRuntime`/`RecoveryContext` types that may need alignment with crate models. | - |
| transitions.rs | supervision/mount_runtime/transitions.py | B | Missing mount transition helpers. No matching Rust implementation. | Medium |

**Summary:** 24 entries are **B** (genuinely missing in Rust), 2 entries are **A** (root `recovery_transitions.rs` points to `supervision/recovery_transitions.rs`, which itself has a real implementation). No **C** entries.


## start/stop flow (batch04_start_stop_flow)

# Batch 04 Start/Stop Flow Stub Classification

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| agent_runtime.rs | start_runtime/agent_runtime.py | A | Translation exists in `rust/crates/ccb-daemon/src/start_runtime/agent_runtime.rs` (currently unlinked from `lib.rs`); mirrors the attach/restore orchestration. | — |
| agent_runtime_binding.rs | start_runtime/agent_runtime_binding.py | B | 3-line TODO stub; `resolve_runtime_binding_state` and helpers are missing. | Medium |
| agent_runtime_models.rs | start_runtime/agent_runtime_models.py | B | 3-line stub; `StartAgentExecution`/`RuntimeBindingState` dataclasses missing. | Small |
| binding.rs | start_runtime/binding.py | B | 3-line stub; re-exports from `binding_runtime` missing. | Small |
| deps.rs | start_flow_runtime/deps.py | B | 3-line stub; `StartFlowDeps` dependency bag missing. | Small |
| layout.rs | start_runtime/layout.py | A | Equivalent tmux layout/respawn/shell helpers already exist in `ccb-terminal` (`layouts`, `respawn`, `env`). | — |
| pid_cleanup.rs | stop_flow_runtime/pid_cleanup.py | A | Thin wrapper around `runtime_pid_cleanup`; implemented in `rust/crates/ccb-runtime-pid-cleanup`. | — |
| runtime_records.rs | stop_flow_runtime/runtime_records.py | B | 3-line stub; shutdown snapshot/runtime-record helpers missing (`AgentRuntimeStore` exists but not these helpers). | Small |
| service_agents.rs | start_flow_runtime/service_agents.py | B | 3-line stub; `prepare_agents` wrapper missing. | Small |
| service_context.rs | start_flow_runtime/service_context.py | B | 3-line stub; `build_start_context`/`record_namespace_action` missing. | Small |
| service_tmux.rs | start_flow_runtime/service_tmux.py | B | 3-line stub; `tmux_namespace_runtime`, layout helpers, active-pane tracking missing. | Medium |
| session_file.rs | start_runtime/binding_runtime/session_file.py | B | 3-line stub; `declared_binding_tmux_socket_path` missing. | Small |
| start_flow_runtime/binding.rs | start_flow_runtime/binding.py | B | 3-line stub; binding wrapper functions missing. | Small |
| start_flow_runtime/deps.rs | start_flow_runtime/deps.py | B | 3-line stub; `StartFlowDeps` dataclass missing. | Small |
| start_flow_runtime/layout.rs | start_flow_runtime/layout.py | B | 3-line stub; layout wrapper functions missing (underlying tmux layout code lives in `ccb-terminal`). | Small |
| start_flow_runtime/service.rs | start_flow_runtime/service.py | A | Partial real code exists here but is unlinked; canonical start-flow orchestration is `ccb-daemon::start_flow::service`. | — |
| start_flow_runtime/service_agents.rs | start_flow_runtime/service_agents.py | B | 3-line stub; `prepare_agents` wrapper missing. | Small |
| start_flow_runtime/service_context.rs | start_flow_runtime/service_context.py | B | 3-line stub; `build_start_context`/`record_namespace_action` missing. | Small |
| start_flow_runtime/service_tmux.rs | start_flow_runtime/service_tmux.py | B | 3-line stub; tmux namespace/layout/active-pane helpers missing. | Medium |
| start_flow_runtime/summary.rs | start_flow_runtime/summary.py | B | 3-line stub; `StartFlowSummary` dataclass missing. | Small |
| start_flow_runtime_service.rs | start_flow_runtime/service.py | A | Compiled partial implementation, but functional start-flow orchestration already lives in `ccb-daemon::start_flow::service`. | — |
| start_runtime/agent_runtime.rs | start_runtime/agent_runtime.py | A | Real translation present in this file (unlinked); mirrors Python `start_agent_runtime` attach/restore logic. | — |
| start_runtime/agent_runtime_binding.rs | start_runtime/agent_runtime_binding.py | B | 3-line stub; `resolve_runtime_binding_state` and helpers missing. | Medium |
| start_runtime/agent_runtime_models.rs | start_runtime/agent_runtime_models.py | B | 3-line stub; dataclasses missing. | Small |
| start_runtime/binding.rs | start_runtime/binding.py | B | 3-line stub; re-exports from `binding_runtime` missing. | Small |
| start_runtime/binding_runtime/common.rs | start_runtime/binding_runtime/common.py | B | 3-line stub; `binding_pane_id`, `matching_project_namespace_record` helpers missing. | Small |
| start_runtime/binding_runtime/lifecycle.rs | start_runtime/binding_runtime/lifecycle.py | B | 3-line stub; `launch_binding_hint`/`relabel_project_namespace_pane` missing. | Small |
| start_runtime/binding_runtime/session_file.rs | start_runtime/binding_runtime/session_file.py | B | 3-line stub; `declared_binding_tmux_socket_path` missing. | Small |
| start_runtime/binding_runtime/validation.rs | start_runtime/binding_runtime/validation.py | B | 3-line stub; `usable_*` binding validators missing. | Medium |
| start_runtime/binding_runtime/validation_context.rs | start_runtime/binding_runtime/validation_context.py | B | 3-line stub; `BindingValidationContext` and predicate helpers missing. | Medium |
| start_runtime/binding_runtime/validation_rules.rs | start_runtime/binding_runtime/validation_rules.py | B | 3-line stub; `usable_project_namespace_binding_for_context` rules missing. | Small |
| start_runtime/cleanup.rs | start_runtime/cleanup.py | B | 3-line stub; `cleanup_start_tmux_orphans` wrapper missing. | Small |
| start_runtime/layout.rs | start_runtime/layout.py | A | Equivalent tmux layout/respawn/shell helpers exist in `ccb-terminal`. | — |
| start_runtime/restore.rs | start_runtime/restore.py | B | 3-line stub; `build_restore_state` missing (`AgentRestoreState` model exists). | Small |
| stop_flow_runtime/models.rs | stop_flow_runtime/models.py | B | 3-line stub; `StopAllSummary`/`StopAllExecution` dataclasses missing. | Small |
| stop_flow_runtime/pid_cleanup.rs | stop_flow_runtime/pid_cleanup.py | A | Thin wrapper around `runtime_pid_cleanup`; implemented in `rust/crates/ccb-runtime-pid-cleanup`. | — |
| stop_flow_runtime/runtime_records.rs | stop_flow_runtime/runtime_records.py | B | 3-line stub; shutdown snapshot helpers missing. | Small |
| stop_flow_runtime/service.rs | stop_flow_runtime/service.py | A | Partial real code exists here but is unlinked; canonical stop-flow orchestration is `ccb-daemon::stop_flow::service`, with PID cleanup from `ccb-runtime-pid-cleanup`. | — |
| stop_flow_runtime/tmux_cleanup.rs | stop_flow_runtime/tmux_cleanup.py | B | 3-line stub; `cleanup_stop_tmux_orphans` wrapper missing. | Small |
| summary.rs | start_flow_runtime/summary.py | B | 3-line stub; `StartFlowSummary` dataclass missing. | Small |
| tmux_cleanup.rs | stop_flow_runtime/tmux_cleanup.py | B | 3-line stub; `cleanup_stop_tmux_orphans` wrapper missing. | Small |
| validation.rs | start_runtime/binding_runtime/validation.py | B | 3-line stub; `usable_*` binding validators missing. | Medium |
| validation_context.rs | start_runtime/binding_runtime/validation_context.py | B | 3-line stub; `BindingValidationContext` and predicate helpers missing. | Medium |
| validation_rules.rs | start_runtime/binding_runtime/validation_rules.py | B | 3-line stub; validation rules missing. | Small |

## Summary

- **A (already implemented elsewhere):** 9 entries — mostly PID cleanup (`ccb-runtime-pid-cleanup`), tmux layout/shell helpers (`ccb-terminal`), and the start/stop orchestration services (`ccb-daemon::start_flow::service`, `ccb-daemon::stop_flow::service`). A usable translation of `start_runtime/agent_runtime.py` also exists but is not wired into `lib.rs`.
- **B (genuinely missing):** 35 entries — the bulk are 3-line TODO stubs for dataclasses, dependency bags, validation predicates, binding helpers, and start/stop wrappers.
- **C (not applicable):** 0 entries.


## health & heartbeat (batch05_health_heartbeat)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| `backend.rs` | `services/health_assessment/tmux_runtime/backend.py` | C | Re-export wrapper for `session_backend`. In Rust the session backend is accessed directly via `Session.backend` / the `SessionBackend` trait in `ccb-provider-core/src/session_binding.rs`. | N/A |
| `degraded.rs` | `services/health_monitor_runtime/updates_runtime/degraded.py` | B | `mark_degraded` is not implemented. `AgentState` already exists in `ccb-agents/src/models.rs`, but the runtime-field helpers it calls are still missing. | Medium |
| `facts.rs` | `services/health_monitor_runtime/updates_runtime/facts.py` | B | `provider_runtime_facts` is missing; it depends on `build_provider_runtime_facts`, which has no Rust equivalent. | Medium |
| `provider.rs` | `services/health_monitor_runtime/provider.py` | B | `provider_pane_health` orchestration is missing; depends on assessment/rebind/mark-degraded pieces. | Small |
| `provider_pane.rs` | `services/health_assessment/provider_pane.py` | A | Equivalent pane assessment lives in `rust/crates/ccb-daemon/src/services/health.rs` (`assess_provider_panes`, `assess_tmux_pane_state`, `TmuxPaneState`). The Python session-binding load path is not mirrored. | N/A |
| `rebind.rs` | `services/health_monitor_runtime/updates_runtime/rebind.py` | B | `rebind_runtime` and its helpers are missing. `session_ref` and field primitives exist in `ccb-provider-core/src/session_binding.rs`, but the rebind logic is not implemented. | Medium |
| `services/health_assessment/models.rs` | `services/health_assessment/models.py` | A | `ProviderPaneAssessment` is defined in `rust/crates/ccb-daemon/src/services/health.rs` (fields differ: `agent_name`/`provider`/`pane_id`/`pane_state`/`health`). | N/A |
| `services/health_assessment/provider_pane.rs` | `services/health_assessment/provider_pane.py` | A | Same as the top-level `provider_pane.rs` row; real implementation is in `services/health.rs`. | N/A |
| `services/health_assessment/tmux.rs` | `services/health_assessment/tmux.py` | C | Re-export shim. The underlying functions have their own entries; `inspect_tmux_pane_ownership` is in `ccb-provider-core/src/tmux_ownership.rs`, and pane-state logic is in `ccb-provider-core/src/session_binding.rs`. | N/A |
| `services/health_assessment/tmux_runtime/backend.rs` | `services/health_assessment/tmux_runtime/backend.py` | C | Same as the top-level `backend.rs` row: re-export wrapper; session backend is available via `Session.backend` / `SessionBackend`. | N/A |
| `services/health_assessment/tmux_runtime/namespace.rs` | `services/health_assessment/tmux_runtime/namespace.py` | B | `pane_outside_project_namespace` and project-namespace matching logic are missing. `ProjectNamespace` struct exists in `services/project_namespace.rs`. | Medium |
| `services/health_assessment/tmux_runtime/ownership.rs` | `services/health_assessment/tmux_runtime/ownership.py` | A | `inspect_tmux_pane_ownership` is implemented in `ccb-provider-core/src/tmux_ownership.rs` (`TmuxPaneOwnership`). | N/A |
| `services/health_assessment/tmux_runtime/state.rs` | `services/health_assessment/tmux_runtime/state.py` | A | Pane existence/ownership/alive checks exist in `ccb-provider-core/src/session_binding.rs` (`inspect_session_pane`, `resolve_pane_state`) and in `services/health.rs` (`assess_tmux_pane_state`). The small wrapper functions are not present. | N/A |
| `services/health_monitor_runtime/provider.rs` | `services/health_monitor_runtime/provider.py` | B | Same as the top-level `provider.rs` row: `provider_pane_health` orchestration is missing. | Small |
| `services/health_monitor_runtime/status.rs` | `services/health_monitor_runtime/status.py` | B | `runtime_health`, `pane_health`, `check_all`, `collect_orphans`, and `daemon_health` are missing. `HealthMonitor::daemon_health`/`inspect_registry` in `services/health.rs` cover only the daemon/aggregate parts. `AgentState`/`RuntimeBindingSource` exist in `ccb-agents`. | Medium |
| `services/health_monitor_runtime/updates.rs` | `services/health_monitor_runtime/updates.py` | C | Re-export shim for `mark_degraded`/`provider_runtime_facts`/`rebind_runtime`; each has its own stub entry. | N/A |
| `services/health_monitor_runtime/updates_runtime/common.rs` | `services/health_monitor_runtime/updates_runtime/common.py` | B | Runtime field helpers (`runtime_fields_from_facts`, `runtime_fields_from_session`, `pane_state_for_health`, etc.) are missing. Primitive field extractors exist in `ccb-provider-core/src/session_binding.rs` and `AgentBinding`. | Medium |
| `services/health_monitor_runtime/updates_runtime/degraded.rs` | `services/health_monitor_runtime/updates_runtime/degraded.py` | B | Same as the top-level `degraded.rs` row: `mark_degraded` is missing. | Medium |
| `services/health_monitor_runtime/updates_runtime/facts.rs` | `services/health_monitor_runtime/updates_runtime/facts.py` | B | Same as the top-level `facts.rs` row: `provider_runtime_facts` is missing. | Medium |
| `services/health_monitor_runtime/updates_runtime/rebind.rs` | `services/health_monitor_runtime/updates_runtime/rebind.py` | B | Same as the top-level `rebind.rs` row: `rebind_runtime` is missing. | Medium |
| `services/job_heartbeat_runtime/common.rs` | `services/job_heartbeat_runtime/common.py` | B | Heartbeat formatting/decision helpers (`heartbeat_notice_body`, `heartbeat_timeout_decision`, `heartbeat_diagnostics`, etc.) are missing. `CompletionDecision`/`CompletionStatus` models exist in `ccb-completion/src/models.rs`. | Medium |
| `services/job_heartbeat_runtime/tick.rs` | `services/job_heartbeat_runtime/tick.py` | B | `tick_job_heartbeat` orchestration is missing. A partial skeleton exists in `rust/crates/ccb-daemon/src/tick.rs`, but it uses a stub `evaluate_heartbeat` and does not match Python semantics; the real heartbeat engine is `ccb-heartbeat/src/engine.rs`. | Medium |
| `services/job_heartbeat_runtime/tracking.rs` | `services/job_heartbeat_runtime/tracking.py` | B | `should_track_heartbeat_job`, `tracked_running_jobs`, and `cleanup_inactive_heartbeats` are missing. | Small |
| `status.rs` | `services/health_monitor_runtime/status.py` | B | Same as `services/health_monitor_runtime/status.rs` row: health-status helpers are missing. | Medium |
| `tick.rs` | `services/job_heartbeat_runtime/tick.py` | B | Same as `services/job_heartbeat_runtime/tick.rs` row: orchestration missing/incomplete. | Medium |
| `tracking.rs` | `services/job_heartbeat_runtime/tracking.py` | B | Same as `services/job_heartbeat_runtime/tracking.rs` row: tracking helpers are missing. | Small |
| `updates.rs` | `services/health_monitor_runtime/updates.py` | C | Same as `services/health_monitor_runtime/updates.rs` row: re-export shim. | N/A |


## runtime & reload (batch06_runtime)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| attach_models.rs | services/runtime_runtime/attach_models.py | B | AttachRuntimeValues dataclass; runtime attach value container not implemented elsewhere. | Small |
| attach_records.rs | services/runtime_runtime/attach_records.py | B | new_runtime / updated_runtime record builders not implemented elsewhere. | Small |
| attach_values.rs | services/runtime_runtime/attach_values.py | B | resolve_attach_runtime_values + helpers; ~270 LOC of binding/session/epoch resolution logic missing. | Large |
| execution.rs | services/runtime_runtime/restore_runtime/execution.py | B | restore_runtime restore-from-store flow not implemented elsewhere. | Small |
| files.rs | runtime_runtime/files.py | A | run_dir / state_file_path / log_path already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| helpers.rs | services/runtime_runtime/restore_runtime/helpers.py | B | restore_attachment_kwargs / touch_active_runtime helpers not implemented elsewhere. | Small |
| readiness.rs | services/runtime_runtime/restore_runtime/readiness.py | B | ensure_runtime_ready readiness orchestrator not implemented elsewhere. | Small |
| refresh.rs | services/runtime_runtime/refresh.py | B | refresh_provider_binding; `services/runtime.rs` has only a simplified stub. | Medium |
| reload_append_layout.rs | reload_append_layout.py | B | AppendAgentPlan layout append planning not implemented elsewhere. | Medium |
| reload_apply.rs | reload_apply.py | B | re-export shim not implemented. | Small |
| reload_apply_graph.rs | reload_apply_graph.py | B | build_reload_service_graph wrapper not implemented elsewhere. | Medium |
| reload_apply_models.rs | reload_apply_models.py | A | AdditiveReloadApplyResult struct + to_record already implemented in this stub. | - |
| reload_apply_namespace.rs | reload_apply_namespace.py | B | apply_namespace_patch wrapper missing (partial coverage in `reload_transaction.rs`). | Medium |
| reload_apply_plan.rs | reload_apply_plan.py | A | plan_blocker / plan_blocked_result already implemented in this stub. | - |
| reload_apply_publish.rs | reload_apply_publish.py | B | publish_transaction delegation not implemented elsewhere. | Small |
| reload_apply_results.rs | reload_apply_results.py | B | stage_result / noop_result partial; residue extraction only handles `partial`. | Medium |
| reload_apply_runtime.rs | reload_apply_runtime.py | B | run_runtime_mount stage not implemented elsewhere. | Medium |
| reload_apply_service.rs | reload_apply_service.py | B | run_additive_reload_apply full additive reload orchestration missing. | Large |
| reload_apply_stages.rs | reload_apply_stages.py | B | namespace_patch_failed partial; `publish_stage` is still a stub. | Medium |
| reload_drain.rs | reload_drain.py | B | DrainQueue / DrainIntent / transitions; ~438 LOC missing. | Large |
| reload_handoff.rs | reload_handoff.py | B | ReloadHandoff store / validation; ~204 LOC missing. | Medium |
| reload_patch_additive_agents.rs | reload_patch_additive_agents.py | B | additive_agent_steps planning missing. | Small |
| reload_runtime_mount_models.rs | reload_runtime_mount_models.py | B | AdditiveRuntimeMountResult partial struct; missing result helper functions. | Small |
| reload_runtime_mount_service.rs | reload_runtime_mount_service.py | B | run_additive_agent_mounts returns success unconditionally; does not match Python. | Medium |
| reload_runtime_mount_start.rs | reload_runtime_mount_start.py | B | call_start_flow_for_additive_mount adapter missing. | Small |
| reload_runtime_mount_state.rs | reload_runtime_mount_state.py | B | agent_panes_from_record / snapshot / change helpers missing. | Small |
| reload_runtime_mount_validation.rs | reload_runtime_mount_validation.py | B | blocked_mount_reason partial; local type defs differ from project models. | Small |
| reload_runtime_unload.rs | reload_runtime_unload.py | B | run_removed_agent_unloads / pre_namespace_unload_blocker missing. | Small |
| reload_transaction_context.rs | reload_transaction_context.py | B | TransactionContext / pre_publish_blocker missing. | Small |
| reload_transaction_models.rs | reload_transaction_models.py | B | ReloadPublishTransactionResult dataclass missing. | Small |
| reload_transaction_preflight.rs | reload_transaction_preflight.py | B | initial_failure preflight checks missing. | Small |
| reload_transaction_publish.rs | reload_transaction_publish.py | B | publish_or_rollback logic missing. | Small |
| reload_transaction_records.rs | reload_transaction_records.py | B | graph_signature / record normalization helpers missing. | Small |
| reload_transaction_results.rs | reload_transaction_results.py | B | blocked_result / failed_result / published_result constructors missing. | Small |
| reload_transaction_service.rs | reload_transaction_service.py | B | publish_additive_reload_transaction orchestration missing. | Medium |
| reload_transaction_signature.rs | reload_transaction_signature.py | B | lease/lifecycle signature handoff functions missing. | Medium |
| reload_transaction_signature_rollback.rs | reload_transaction_signature_rollback.py | B | rollback_signatures logic missing. | Small |
| runtime_attach.rs | services/runtime_attach.py | B | binding_source_for_attach / resolve_session_fields utilities missing. | Medium |
| runtime_recovery_policy.rs | services/runtime_recovery_policy.py | B | should_attempt_background_recovery predicates missing. | Small |
| runtime_runtime/files.rs | runtime_runtime/files.py | A | run_dir / state_file_path / log_path already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| runtime_runtime/logs.rs | runtime_runtime/logs.py | A | write_log already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| runtime_runtime/state.rs | runtime_runtime/state.py | A | get_daemon_work_dir already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| runtime_runtime/support.rs | runtime_runtime/support.py | A | random_token / normalize_connect_host already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| services/runtime_attach.rs | services/runtime_attach.py | B | binding_source_for_attach / resolve_session_fields utilities missing. | Medium |
| services/runtime_recovery_policy.rs | services/runtime_recovery_policy.py | B | should_attempt_background_recovery predicates missing. | Small |
| services/runtime_runtime/attach.rs | services/runtime_runtime/attach.py | B | attach_runtime stub is minimal; missing resolve/upsert/mount_attempt logic vs Python. | Medium |
| services/runtime_runtime/attach_models.rs | services/runtime_runtime/attach_models.py | B | AttachRuntimeValues dataclass not implemented elsewhere. | Small |
| services/runtime_runtime/attach_records.rs | services/runtime_runtime/attach_records.py | B | new_runtime / updated_runtime record builders not implemented elsewhere. | Small |
| services/runtime_runtime/attach_values.rs | services/runtime_runtime/attach_values.py | B | resolve_attach_runtime_values + helpers; ~270 LOC missing. | Large |
| services/runtime_runtime/common.rs | services/runtime_runtime/common.py | B | ACTIVE_RUNTIME_STATES set + fallback_workspace_path helper missing. | Small |
| services/runtime_runtime/refresh.rs | services/runtime_runtime/refresh.py | B | refresh_provider_binding; `services/runtime.rs` has only a simplified stub. | Medium |
| services/runtime_runtime/restore.rs | services/runtime_runtime/restore.py | B | re-export shim not implemented. | Small |
| services/runtime_runtime/restore_runtime/execution.rs | services/runtime_runtime/restore_runtime/execution.py | B | restore_runtime restore-from-store flow not implemented elsewhere. | Small |
| services/runtime_runtime/restore_runtime/helpers.rs | services/runtime_runtime/restore_runtime/helpers.py | B | restore_attachment_kwargs / touch helpers not implemented elsewhere. | Small |
| services/runtime_runtime/restore_runtime/readiness.rs | services/runtime_runtime/restore_runtime/readiness.py | B | ensure_runtime_ready orchestrator not implemented elsewhere. | Small |
| state.rs | runtime_runtime/state.py | A | get_daemon_work_dir already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |
| support.rs | runtime_runtime/support.py | A | random_token / normalize_connect_host already implemented in `rust/crates/ccb-daemon/src/runtime.rs`. | - |


## app/keeper/client/socket (batch07_app_keeper_client_socket)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| app_runtime/bootstrap.rs | app_runtime/bootstrap.py | A | Implemented in `rust/crates/ccb-daemon/src/app.rs` (`CcbdApp::new` / `CcbdApp::with_backend`). | - |
| app_runtime/handlers.rs | app_runtime/handlers.py | A | Implemented in `rust/crates/ccb-daemon/src/handlers/mod.rs` (`build_registry`). | - |
| app_runtime/lifecycle.rs | app_runtime/lifecycle.py | A | Core lifecycle split across `app.rs` (`start`, `heartbeat`, `shutdown`, `request_shutdown`, `stop_all`, report writes) and `socket_server/server.rs` + `main.rs` (serve loop). | - |
| app_runtime/policy.rs | app_runtime/policy.py | B | `persist_start_policy` exists in `app.rs`; `recovery_start_options`, `mount_agent_from_policy`, `remount_project_from_policy` are missing. | Small |
| app_runtime/request_guard.rs | app_runtime/request_guard.py | B | `rejection_for_request` and `lifecycle_is_stopping` are not implemented; Rust socket server has no lifecycle request guard. | Small |
| app_runtime/service_graph.rs | app_runtime/service_graph.py | A | Replaced by direct `CcbdApp` field construction in `app.rs`; no separate service-graph dataclass. | - |
| app_state.rs | keeper_runtime/app_state.py | C | No keeper process in the Rust daemon; daemon runs directly via `main.rs`. | - |
| bootstrap.rs | app_runtime/bootstrap.py | A | Same as `app_runtime/bootstrap.rs`: `CcbdApp::new` in `app.rs`. | - |
| client_runtime/resolution.rs | client_runtime/resolution.py | B | Re-exports `resolve_work_dir` / `resolve_work_dir_with_registry`; neither helper exists in Rust. | Small |
| client_runtime/resolution_runtime/explicit.rs | client_runtime/resolution_runtime/explicit.py | B | `resolve_work_dir` and session-path validation helpers are missing. | Small |
| client_runtime/resolution_runtime/registry.rs | client_runtime/resolution_runtime/registry.py | B | `resolve_work_dir_with_registry` is missing (Rust only has the underlying `find_project_session_file` in `ccb-provider-sessions`). | Small |
| endpoints.rs | socket_client_runtime/endpoints.py | B | `bind_endpoint`, payload builders, and `client_endpoints` map are missing; CLI builds payloads per-command in `commands.rs`. | Medium |
| errors.rs | socket_client_runtime/errors.py | B | `CcbdClientError` exception type is not defined in Rust. | Small |
| explicit.rs | client_runtime/resolution_runtime/explicit.py | B | Same as `client_runtime/resolution_runtime/explicit.rs`: `resolve_work_dir` missing. | Small |
| keeper_runtime/app_state.rs | keeper_runtime/app_state.py | C | No keeper process in the Rust daemon. | - |
| keeper_runtime/loop_.rs | keeper_runtime/loop.py | C | No keeper process in the Rust daemon. | - |
| keeper_runtime/records.rs | keeper_runtime/records.py | C | No keeper process in the Rust daemon. | - |
| keeper_runtime/state.rs | keeper_runtime/state.py | C | No keeper process in the Rust daemon. | - |
| keeper_runtime/stores.rs | keeper_runtime/stores.py | C | No keeper process in the Rust daemon. | - |
| keeper_runtime/support.rs | keeper_runtime/support.py | C | No keeper process in the Rust daemon. | - |
| policy.rs | app_runtime/policy.py | B | Same as `app_runtime/policy.rs`: only `persist_start_policy` is present in `app.rs`. | Small |
| request_guard.rs | app_runtime/request_guard.py | B | Same as `app_runtime/request_guard.rs`: lifecycle request guard missing. | Small |
| resolution.rs | client_runtime/resolution.py | B | Same as `client_runtime/resolution.rs`: resolve helpers missing. | Small |
| service_graph.rs | app_runtime/service_graph.py | A | Same as `app_runtime/service_graph.rs`: replaced by `CcbdApp` construction in `app.rs`. | - |
| socket_client.rs | socket_client.py | A | Core Unix-socket client implemented in `rust/crates/ccb-cli/src/services/mod.rs` (`UnixDaemonClient`); endpoint binding is not generic. | - |
| socket_client_runtime/endpoints.rs | socket_client_runtime/endpoints.py | B | Same as `endpoints.rs`: endpoint map and bind helper missing. | Medium |
| socket_client_runtime/errors.rs | socket_client_runtime/errors.py | B | Same as `errors.rs`: `CcbdClientError` missing. | Small |
| socket_client_runtime/transport.rs | socket_client_runtime/transport.py | A | Transport logic inlined in `UnixDaemonClient::call` in `rust/crates/ccb-cli/src/services/mod.rs`. | - |
| socket_server_runtime/lifecycle.rs | socket_server_runtime/lifecycle.py | A | Implemented in `rust/crates/ccb-daemon/src/socket_server/server.rs` (`SocketServer::listen`, `SocketServer::shutdown`). | - |
| socket_server_runtime/loop_.rs | socket_server_runtime/loop.py | A | Implemented in `SocketServer::listen`; Rust uses a simpler single-threaded accept loop rather than Python's queued worker/maintenance threads. | - |
| socket_server_runtime/protocol.rs | socket_server_runtime/protocol.py | A | Implemented in `rust/crates/ccb-daemon/src/socket_server/protocol.rs` (`handle_request`). | - |
| socket_server_runtime/server.rs | socket_server_runtime/server.py | A | Implemented in `rust/crates/ccb-daemon/src/socket_server/server.rs` (`SocketServer`). | - |
| stores.rs | keeper_runtime/stores.py | C | No keeper process in the Rust daemon. | - |
| transport.rs | socket_client_runtime/transport.py | A | Same as `socket_client_runtime/transport.rs`: inlined in `UnixDaemonClient::call`. | - |


## handlers & views (batch08_handlers_views)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| activity.rs | project_view/activity.py | B | Stub. Python implements `resolve_agent_activity` and pane-text heuristics (active/idle/pending/failed). Not implemented elsewhere in the Rust workspace. | Large |
| handler.rs | handlers/ping_runtime/handler.py | B | Stub. Rust has a basic `handlers/ping.rs`, but the full `build_ping_handler` factory, agent/ccbd payload dispatch, and metric timing are missing. | Medium |
| handlers/ping_runtime/handler.rs | handlers/ping_runtime/handler.py | B | Same as `handler.rs` above: full ping handler factory is missing. | Medium |
| handlers/ping_runtime/payloads.rs | handlers/ping_runtime/payloads.py | B | Stub. `build_agent_payload` and `build_ccbd_payload` (with diagnostics/metrics/identity merging) are not implemented elsewhere. | Medium |
| handlers/ping_runtime/summaries.rs | handlers/ping_runtime/summaries.py | B | Stub. Summary loaders for restore/namespace/event/start-policy stores are missing. | Small |
| handlers/project_reload_cache.rs | handlers/project_reload_cache.py | B | Stub. Project-view cache invalidation helper (`invalidate_project_view_cache`) is not wired in Rust; `project_view/service.rs` has no `invalidate_cache` method. | Small |
| handlers/project_reload_metrics.rs | handlers/project_reload_metrics.py | B | Stub. `metrics_fields` and diagnostic/error text extraction for reload results are missing. | Small |
| handlers/project_reload_payload.rs | handlers/project_reload_payload.py | B | Stub. `apply_reload_payload` transform (status → mutation flags, diagnostics, errors) is not applied in `handlers/project_reload.rs`. | Small |
| payloads.rs | handlers/ping_runtime/payloads.py | B | Same as `handlers/ping_runtime/payloads.rs` above. | Medium |
| project_focus/tmux.rs | project_focus/tmux.py | B | Stub. Low-level tmux primitives exist in `ccb-terminal`, but the project-focus helpers (`backend_for_namespace`, `select_window`, `select_pane`, `find_agent_pane`, `refresh_sidebar_panes`) are missing in `ccb-daemon`. | Medium |
| project_reload_cache.rs | handlers/project_reload_cache.py | B | Same as `handlers/project_reload_cache.rs` above. | Small |
| project_reload_metrics.rs | handlers/project_reload_metrics.py | B | Same as `handlers/project_reload_metrics.rs` above. | Small |
| project_reload_payload.rs | handlers/project_reload_payload.py | B | Same as `handlers/project_reload_payload.rs` above. | Small |
| project_view/activity.rs | project_view/activity.py | B | Same as root `activity.rs` above. | Large |
| project_view/provider_activity.rs | project_view/provider_activity.py | B | Stub. Underlying read/write lives in `rust/crates/ccb-provider-hooks/src/activity.rs`, but the daemon wrapper (`provider_activity_evidence`, `record_provider_activity_failure`) that resolves runtime dirs and runtime fields is missing. | Small |
| project_view/sequence.rs | project_view/sequence.py | B | Stub. `ProjectViewSequenceCache` (stable digest + sequence number) is not implemented elsewhere. | Small |
| project_view/state.rs | project_view/state.py | B | Stub. Persistent `ProjectViewStateStore` for dismissed comms is missing. Rust has a different `ProjectViewState` response struct in `project_view/service.rs`, not the store. | Small |
| provider_activity.rs | project_view/provider_activity.py | B | Same as `project_view/provider_activity.rs` above; underlying provider-hook activity I/O is in `ccb-provider-hooks/src/activity.rs`. | Small |
| sequence.rs | project_view/sequence.py | B | Same as `project_view/sequence.rs` above. | Small |
| service.rs | project_focus/service.py | B | Stub. **Note:** the Rust stub header says `lib/fault_injection/service.py`, but the batch maps it to `lib/ccbd/project_focus/service.py`. Either way, real `ProjectFocusService` (window/agent focus with tmux selection and cache refresh) is not implemented; `project_focus/service.rs` only stores names and returns mock JSON. | Medium |
| summaries.rs | handlers/ping_runtime/summaries.py | B | Same as `handlers/ping_runtime/summaries.rs` above. | Small |
| tmux.rs | project_focus/tmux.py | B | Same as `project_focus/tmux.rs` above. | Medium |


## models & supervisor & api (batch09_models_supervisor_api)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| api_models_runtime/common.rs | api_models_runtime/common.py | `API_VERSION`, `SCHEMA_VERSION`, `JobStatus`, `DeliveryScope`, `TargetKind` | A. Implemented in `rust/crates/ccb-daemon/src/models/api_models/common.rs` (also adds `MountState`, `LeaseHealth`, `CcbdModelError`). | N/A |
| api_models_runtime/messages.rs | api_models_runtime/messages.py | `MessageEnvelope` | A. Implemented in `models/api_models/messages.rs` and consumed by dispatcher/socket protocol. | N/A |
| api_models_runtime/receipts.rs | api_models_runtime/receipts.py | `AcceptedJobReceipt`, `SubmitReceipt`, `CancelReceipt` | A. Implemented in `models/api_models/receipts.rs` and used by `services/dispatcher.rs`. | N/A |
| api_models_runtime/records.rs | api_models_runtime/records.py | `JobRecord`, `SubmissionRecord`, `JobEvent` | A. Implemented in `models/api_models/records.rs`; related types also live in `ccb-mailbox`. | N/A |
| api_models_runtime/rpc.rs | api_models_runtime/rpc.py | `RpcRequest`, `RpcResponse` | A. Implemented in `models/api_models/rpc.rs` and consumed by `socket_server/protocol.rs`. | N/A |
| models_runtime/common.rs | models_runtime/common.py | `API_VERSION`, `SCHEMA_VERSION`, `CcbdModelError` | A. Implemented in `models/api_models/common.rs`. | N/A |
| models_runtime/lifecycle.rs | models_runtime/lifecycle.py | re-exports of lifecycle types | A. Re-export layer only; canonical types in `models/lifecycle.rs`. | N/A |
| models_runtime/lifecycle_runtime/cleanup.rs | models_runtime/lifecycle_runtime/cleanup.py | `CcbdTmuxCleanupSummary`, `cleanup_summaries_from_objects` | A. Implemented (simplified) in `models/lifecycle.rs`; used by shutdown report serialization. | N/A |
| models_runtime/lifecycle_runtime/common.rs | models_runtime/lifecycle_runtime/common.py | `clean_text`, `clean_tuple`, `coerce_int`, `to_runtime_state` | C. Python deserialization helpers are architecturally replaced by serde derive + inline defaults in the Rust model structs. | N/A |
| models_runtime/lifecycle_runtime/shutdown.rs | models_runtime/lifecycle_runtime/shutdown.py | `CcbdShutdownReport`, `runtime_snapshots_summary`, `_validate_record` | A. Implemented in `models/lifecycle.rs` (`CcbdShutdownReport::to_record`, `summary_fields`, `runtime_snapshots_summary`). | N/A |
| models_runtime/lifecycle_runtime/snapshots.rs | models_runtime/lifecycle_runtime/snapshots.py | `CcbdRuntimeSnapshot` | A. Implemented (simplified) in `models/lifecycle.rs`. | N/A |
| models_runtime/lifecycle_runtime/startup.rs | models_runtime/lifecycle_runtime/startup.py | re-exports of `CcbdStartupAgentResult`, `CcbdStartupReport` | A. Canonical types in `models/lifecycle.rs`. | N/A |
| models_runtime/lifecycle_runtime/startup_agent.rs | models_runtime/lifecycle_runtime/startup_agent.py | `CcbdStartupAgentResult` | A. Implemented (simplified) in `models/lifecycle.rs`. | N/A |
| models_runtime/lifecycle_runtime/startup_report.rs | models_runtime/lifecycle_runtime/startup_report.py | `CcbdStartupReport` | A. Implemented (simplified) in `models/lifecycle.rs`. | N/A |
| models_runtime/mount.rs | models_runtime/mount.py | `MountState`, `LeaseHealth`, `CcbdLease`, `LeaseInspection` | A. Implemented in `models/mount.rs`. | N/A |
| models_runtime/restore.rs | models_runtime/restore.py | `CcbdRestoreEntry`, `CcbdRestoreReport` | A. Implemented in `models/restore.rs`. | N/A |
| namespace.rs | supervisor_runtime/namespace.py | `ensure_project_namespace` | A. Implemented in `supervisor_runtime/namespace.rs`; backing namespace service in `services/project_namespace.rs`. | N/A |
| reporting.rs | supervisor_runtime/reporting.py | `record_startup_report`, `record_shutdown_report` | A. Report construction/persistence is implemented inline in `app.rs` (`start`, `shutdown`, `stop_all`). | N/A |
| startup.rs | models_runtime/lifecycle_runtime/startup.py | re-exports of startup types | A. Canonical types in `models/lifecycle.rs`. | N/A |
| startup_agent.rs | models_runtime/lifecycle_runtime/startup_agent.py | `CcbdStartupAgentResult` | A. Implemented in `models/lifecycle.rs`. | N/A |
| startup_report.rs | models_runtime/lifecycle_runtime/startup_report.py | `CcbdStartupReport` | A. Implemented in `models/lifecycle.rs`. | N/A |
| state_bundle.rs | supervisor_runtime/state_bundle.py | `SupervisorRuntimeState`, `SupervisorRuntimeStateMixin` | C. Python dependency-bundle/mixin pattern is architecturally replaced by the `CcbdApp` struct and service graph in Rust. | N/A |
| supervisor_runtime/lifecycle.rs | supervisor_runtime/lifecycle.py | `start_supervisor`, `stop_all_supervisor`, `_sync_lifecycle_namespace_epoch`, `_uses_explicit_windows_topology` | A. Orchestration implemented in `app.rs` (`start`, `stop_all`, `shutdown`) and the start/stop flow services (`start_flow/service.rs`, `stop_flow/service.rs`). | N/A |
| supervisor_runtime/namespace.rs | supervisor_runtime/namespace.py | `ensure_project_namespace`, `ProjectNamespace` trait | A. Real code in this file already functionally matches the Python implementation; concrete namespace logic is in `services/project_namespace.rs`. | N/A |
| supervisor_runtime/reporting.rs | supervisor_runtime/reporting.py | `record_startup_report`, `record_shutdown_report` | A. Implemented inline in `app.rs` start/shutdown flows. | N/A |
| supervisor_runtime/state_bundle.rs | supervisor_runtime/state_bundle.py | `SupervisorRuntimeState`, `SupervisorRuntimeStateMixin` | C. Architecturally replaced by `CcbdApp` and the service graph. | N/A |


## remaining (batch10_remaining)

| rust_file | py_file | class | notes | complexity |
|---|---|---|---|---|
| api_models.rs | api_models.py | A | Python re-export wrapper; actual API models already live in `rust/crates/ccb-daemon/src/models/api_models/` and `api_models_runtime/` (common, messages, records, receipts, rpc). | - |
| daemon_process.rs | daemon_process.py | A | Spawn/wait behavior for the ccbd process is implemented in `rust/crates/ccb-cli/src/services/daemon_runtime/processes.rs` and `facade.rs`. | - |
| health_runtime_state.rs | services/health_runtime_state.py | B | `HealthMonitorRuntimeState` state bag and mixin are missing; the health-monitor runtime submodules are still stubs. | Small |
| keeper.rs | keeper.py | B | `ProjectKeeper` orchestration class is missing. Only low-level keeper helpers exist in `ccb-cli/src/services/daemon_runtime/keeper.rs`. | Large |
| keeper_main.rs | keeper_main.py | B | CLI entrypoint for `ProjectKeeper` is not implemented. | Small |
| lifecycle_report_store.rs | lifecycle_report_store.py | B | Report structs exist in `rust/crates/ccb-daemon/src/models/lifecycle.rs`, but the `CcbdStartupReportStore`/`CcbdShutdownReportStore` persistence wrappers are missing. | Small |
| metrics.rs | metrics.py | B | `ControlPlaneMetrics` struct and `/proc/self` process snapshot helpers are not implemented. | Medium |
| project_inspection.rs | services/project_inspection.py | B | `ProjectDaemonInspection` builder and fallback phase/desired-state logic are not implemented. | Medium |
| project_namespace_pane.rs | services/project_namespace_pane.py | B | `ProjectNamespacePaneRecord`, `inspect_project_namespace_pane`, and tmux pane matching are not implemented. | Medium |
| project_namespace_state.rs | services/project_namespace_state.py | B | Python re-export wrapper; underlying `ProjectNamespaceState`/`Event`/`Store` types are not implemented (see runtime stubs below). | Small |
| provider_runtime_facts.rs | services/provider_runtime_facts.py | A | Core session fact extraction is implemented in `rust/crates/ccb-provider-core/src/session_binding.rs`; the resulting values are consumed directly as `AgentBinding`/`RuntimeAttachParams` in `start_runtime/agent_runtime.rs`. | - |
| restore_report_store.rs | restore_report_store.py | B | `CcbdRestoreReport` model exists in `rust/crates/ccb-daemon/src/models/restore.rs`, but the `CcbdRestoreReportStore` wrapper is missing. | Small |
| services/health_runtime.rs | services/health_runtime.py | A | Stub is a re-export shim; the real implementation is in `rust/crates/ccb-daemon/src/services/health.rs` (`HealthMonitor`, `ProviderPaneAssessment`). | - |
| services/health_runtime_state.rs | services/health_runtime_state.py | B | Same as `health_runtime_state.rs` above: runtime state bag and mixin are missing. | Small |
| services/project_inspection.rs | services/project_inspection.py | B | Same as `project_inspection.rs` above. | Medium |
| services/project_namespace_pane.rs | services/project_namespace_pane.py | B | Same as `project_namespace_pane.rs` above. | Medium |
| services/project_namespace_state.rs | services/project_namespace_state.py | B | Same as `project_namespace_state.rs` above. | Small |
| services/project_namespace_state_runtime/common.rs | services/project_namespace_state_runtime/common.py | B | Constants (`NAMESPACE_STATE_RECORD_TYPE`, etc.), `clean_text`, and schema/record-type validators are missing. | Small |
| services/project_namespace_state_runtime/models.rs | services/project_namespace_state_runtime/models.py | B | `ProjectNamespaceState` and `ProjectNamespaceEvent` dataclasses with validation, `to_record`/`from_record`, and `summary_fields` are missing. | Medium |
| services/project_namespace_state_runtime/stores.rs | services/project_namespace_state_runtime/stores.py | B | `ProjectNamespaceStateStore`, `ProjectNamespaceEventStore`, and `next_namespace_epoch` are missing. | Small |
| services/provider_runtime_facts.rs | services/provider_runtime_facts.py | A | Same as `provider_runtime_facts.rs` above. | - |
| start_preparation.rs | start_preparation.py | B | Stub contains a placeholder `prepare_start_agents`; the full Python orchestration (workspace planner/materializer/validator, agent spec/restore stores, binding resolution, provider workspace prep) is missing. Primitives exist in `ccb-workspace`/`ccb-agents`. | Large |
| startup_policy.rs | startup_policy.py | A | Timeout constants and env-var parsing are implemented in `rust/crates/ccb-cli/src/services/daemon_runtime/policy.rs`. | - |
| supervisor.rs | supervisor.py | B | `RuntimeSupervisor` class and `start_supervisor`/`stop_all_supervisor` orchestration are missing; start/stop flow services exist but are incomplete stubs. | Large |
| system.rs | system.py | B | General system helpers (`utc_now`, `parse_utc_timestamp`, `process_exists`, `read_boot_id`, `unix_socket_connectable`) are not implemented as a shared module. | Small |


