use std::{cell::RefCell, process::ExitCode, rc::Rc};

use crate::{
    app::common::{ReaderEvent, ReaderPipeline},
    dataseq::SeqWriter,
    verify::{Check, Verifier, VerifyStatus},
};

use super::arg::VerifyCommand;

const VERIFY_FAILED_EXIT_CODE: u8 = 8;

pub fn verify(args: &VerifyCommand) -> anyhow::Result<ExitCode> {
    let output_path = &args.output;
    let output = super::common::open_output(output_path)?;
    let seq_format = args.format.into();

    let mut writer = SeqWriter::new(output, seq_format);
    let mut problem_count = 0u64;
    let mut verifier = if let Some(path) = &args.database {
        Verifier::open(path)?
    } else {
        Verifier::new()
    };

    for exclude in &args.exclude_check {
        verifier.checks_mut().remove(&Check::from(*exclude));
    }

    let verifier = Rc::new(RefCell::new(verifier));

    for input_path in &args.input {
        let span = tracing::info_span!("verify", path = ?input_path);
        let _span_guard = span.enter();

        let input = super::common::open_input(input_path)?;

        tracing::info!("opened file");

        let compression_format = args.compression.try_into_native(input_path)?;
        let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();

        let mut reader = ReaderPipeline::new(
            |event| match event {
                ReaderEvent::Header {
                    header,
                    record_boundary_position: _,
                } => {
                    let mut verifier = verifier.borrow_mut();

                    for problem in verifier.problems() {
                        problem_count += 1;
                        writer.put(problem)?;
                    }
                    verifier.problems_mut().clear();
                    verifier.begin_record(&header)?;

                    Ok(())
                }
                ReaderEvent::Block { data } => {
                    let mut verifier = verifier.borrow_mut();

                    if data.is_empty() {
                        verifier.end_record();
                    } else {
                        verifier.block_data(data);
                    }

                    Ok(())
                }
            },
            input,
            compression_format,
            file_len,
        )?;
        reader.run()?;

        let mut verifier = verifier.borrow_mut();

        if reader.has_record_at_time_compression_fault {
            verifier.add_not_record_at_time_compression();
        }

        loop {
            let action = verifier.verify_end()?;

            for problem in verifier.problems() {
                problem_count += 1;
                writer.put(problem)?;
            }
            verifier.problems_mut().clear();

            match action {
                VerifyStatus::HasMore => {}
                VerifyStatus::Done => break,
            }
        }

        tracing::info!("closed file");
    }

    let exit_code = if problem_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(VERIFY_FAILED_EXIT_CODE)
    };

    Ok(exit_code)
}
