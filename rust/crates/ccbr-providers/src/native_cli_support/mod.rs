pub mod adapter;
pub mod config;
pub mod observation;
pub mod prompt;

pub use adapter::{clear_tracked_processes, NativeCliExecutionAdapter};
pub use config::{
    CommandBuilder, EnvBuilder, NativeCliExecutionConfig, NativeCliExecutionRequest, OutputKind,
};
pub use observation::{
    observe_jsonl_output, observe_stdout_output, NativeCliObservation, NativeCliObserver,
};
pub use prompt::{clean_native_reply, wrap_native_prompt};
