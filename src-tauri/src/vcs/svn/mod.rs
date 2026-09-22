pub mod cli;
pub mod diff_parser;
pub mod info_parser;
pub mod log_parser;
pub mod ref_util;
pub mod summary_parser;

pub(crate) use cli::decode_svn_output;
pub use cli::SvnCredentials;
pub use cli::{
    detect_svn_branch, probe_svn_wc, run_svn, svn_cat_file, svn_cat_file_bytes,
    svn_diff_file_revision, svn_diff_revision, svn_diff_summary_xml, svn_log_revision_xml,
    svn_log_xml, svn_log_xml_paged,
};
pub use diff_parser::parse_unified_diff;
pub use info_parser::{branch_from_relative_url, parse_info_xml, SvnWcInfo};
pub use log_parser::parse_log_xml;
pub use ref_util::{format_svn_ref, parse_svn_revision};
pub use summary_parser::{encoded_child_url, parse_diff_summary_xml, SvnDiffSummaryEntry};
