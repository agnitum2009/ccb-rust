//! Mirrors Python `lib/cli/management_runtime/versioning_runtime/`.

pub mod constants;
pub mod local;
pub mod remote;
pub mod tags;
pub mod transport;

pub use constants::{REMOTE_MAIN_COMMIT_API, REMOTE_TAGS_API, REPO_URL};
pub use local::{format_version_info, get_version_info};
pub use remote::get_remote_version_info;
pub use tags::get_available_versions;
