use fvm_ipld_encoding::ipld_block::IpldBlock;

use crate::error::ExitCode;

/// The outcome of a `Send`, covering its ExitCode and optional return data
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Response {
    pub exit_code: ExitCode,
    pub return_data: Option<IpldBlock>,
}
