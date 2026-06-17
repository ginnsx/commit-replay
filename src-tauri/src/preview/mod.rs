pub mod git_wc;
pub mod integration;
pub mod patch_apply;
pub mod target_wc;

mod service;

pub use integration::{build_integration_plan, strategy_map};
pub use service::{
    build_preview_plan, build_preview_plan_parallel, get_aggregated_files, target_kind_from_repo_type,
    MappingInput, PreviewContext,
};
pub use target_wc::TargetWcKind;
