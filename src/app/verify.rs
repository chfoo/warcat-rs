use std::process::ExitCode;

use super::arg::VerifyCommand;

const VERIFY_FAILED_EXIT_CODE: u8 = 8;

pub fn verify(args: &VerifyCommand) -> anyhow::Result<ExitCode> {
    todo!()
}
