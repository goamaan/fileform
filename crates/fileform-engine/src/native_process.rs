// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn capture(
    reader: impl Read + Send + 'static,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            overflow.store(true, Ordering::Release);
        }
        Ok(bytes)
    })
}
fn capture_file(
    mut reader: impl Read + Send + 'static,
    file: std::fs::File,
    maximum: u64,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let result = (|| {
            let mut writer = std::io::BufWriter::with_capacity(64 * 1024, file);
            let mut buffer = [0u8; 64 * 1024];
            let mut total = 0u64;
            loop {
                let count = match reader.read(&mut buffer) {
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    other => other?,
                };
                if count == 0 {
                    break;
                }
                let next = total.checked_add(count as u64);
                if next.is_none_or(|v| v > maximum) {
                    overflow.store(true, Ordering::Release);
                    writer.flush()?;
                    return Ok(Vec::new());
                }
                writer.write_all(&buffer[..count])?;
                total = next.expect("checked byte count");
            }
            writer.flush()?;
            Ok(Vec::new())
        })();
        if result.is_err() {
            overflow.store(true, Ordering::Release);
        }
        result
    })
}
/// Private adapter for bundled tools which do not spawn child processes.
/// Process-tree containment is required before extending this to arbitrary tools.
pub(crate) fn run(
    command: Command,
    cancellation: &Cancellation,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>> {
    run_with_output_limit(command, cancellation, timeout, limit, None)
}
fn check_output(output: Option<(&std::path::Path, u64)>) -> Result<()> {
    if let Some((path, maximum)) = output {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > maximum {
            return Err(fail(
                "limit",
                "The native output exceeded its size limit. No new output was saved.",
            ));
        }
    }
    Ok(())
}
pub(crate) fn run_with_output_limit(
    command: Command,
    cancellation: &Cancellation,
    timeout: Duration,
    limit: usize,
    output: Option<(&std::path::Path, u64)>,
) -> Result<Vec<u8>> {
    run_internal(command, cancellation, timeout, limit, output, None)
}
/// Keep large binary replies in an owned staging file, not an in-memory Vec.
pub(crate) fn run_to_file(
    command: Command,
    cancellation: &Cancellation,
    timeout: Duration,
    temporary: &mut tempfile::NamedTempFile,
    maximum: u64,
) -> Result<u64> {
    cancellation.check()?;
    let file = temporary.as_file_mut();
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    run_internal(
        command,
        cancellation,
        timeout,
        64 * 1024,
        None,
        Some((file.try_clone()?, maximum)),
    )?;
    let bytes = file.metadata()?.len();
    if bytes > maximum {
        return Err(fail("limit", "Native output exceeded its size limit."));
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(bytes)
}
fn run_internal(
    mut command: Command,
    cancellation: &Cancellation,
    timeout: Duration,
    limit: usize,
    output: Option<(&std::path::Path, u64)>,
    stdout_file: Option<(std::fs::File, u64)>,
) -> Result<Vec<u8>> {
    cancellation.check()?;
    check_output(output)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = Reap(
        command
            .spawn()
            .map_err(|_| fail("engine_unavailable", "The native tool could not start."))?,
    );
    let overflow = Arc::new(AtomicBool::new(false));
    let reader = child.0.stdout.take().expect("piped stdout");
    let stdout = match stdout_file {
        Some((file, maximum)) => capture_file(reader, file, maximum, overflow.clone()),
        None => capture(reader, limit, overflow.clone()),
    };
    let stderr = capture(
        child.0.stderr.take().expect("piped stderr"),
        limit,
        overflow.clone(),
    );
    let started = Instant::now();
    let status = loop {
        if let Err(error) = check_output(output) {
            break Err(error);
        }
        if cancellation.is_cancelled() {
            break Err(fail("cancelled", "Native processing cancelled."));
        }
        if overflow.load(Ordering::Acquire) {
            break Err(fail("limit", "The native tool returned too much data."));
        }
        if started.elapsed() >= timeout {
            break Err(fail("timeout", "Native processing took too long."));
        }
        match child.0.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error.into()),
        }
    };
    if status.is_err() {
        let _ = child.0.kill();
        let _ = child.0.wait();
    }
    let out = stdout
        .join()
        .map_err(|_| fail("engine_failed", "Native output could not be read."))??;
    let err = stderr
        .join()
        .map_err(|_| fail("engine_failed", "Native output could not be read."))??;
    cancellation.check()?;
    if overflow.load(Ordering::Acquire) {
        return Err(fail("limit", "The native tool returned too much data."));
    }
    check_output(output)?;
    if !status?.success() {
        if std::env::var_os("FILEFORM_NATIVE_DIAGNOSTICS").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            let text = String::from_utf8_lossy(&err[..err.len().min(4096)]);
            let safe: String = text
                .chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                .collect();
            eprintln!("Native tool diagnostic: {safe}");
        }
        return Err(fail(
            "unsupported",
            "The native operation failed. The input may be damaged or unsupported.",
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "subprocess fixture"]
    fn child_fixture() {
        use std::io::Write;
        match std::env::var("FILEFORM_TEST_CHILD").as_deref() {
            Ok("binary") => {
                std::io::stdout()
                    .write_all(b"binary-start\0\xff\r\nbinary-end")
                    .unwrap();
            }
            Ok("flood") => {
                for _ in 0..1000 {
                    let _ = std::io::stdout().write_all(&[b'x'; 4096]);
                }
            }
            Ok("file") => {
                std::fs::write(
                    std::env::var_os("FILEFORM_TEST_OUTPUT").unwrap(),
                    vec![0; 8192],
                )
                .unwrap();
            }
            Ok("wait") => thread::sleep(Duration::from_secs(10)),
            Ok("fail") => std::process::exit(7),
            _ => println!("fixture complete"),
        }
    }
    fn command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "native_process::tests::child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("FILEFORM_TEST_CHILD", mode);
        command
    }
    #[test]
    fn captures_success_and_rejects_failure_and_overflow() {
        assert!(String::from_utf8(
            run(
                command("ok"),
                &Cancellation::default(),
                Duration::from_secs(5),
                4096
            )
            .unwrap()
        )
        .unwrap()
        .contains("fixture complete"));
        assert_eq!(
            run(
                command("fail"),
                &Cancellation::default(),
                Duration::from_secs(5),
                4096
            )
            .unwrap_err()
            .code,
            "unsupported"
        );
        assert_eq!(
            run(
                command("flood"),
                &Cancellation::default(),
                Duration::from_secs(5),
                4096
            )
            .unwrap_err()
            .code,
            "limit"
        );
    }
    #[test]
    fn binary_capture_is_file_backed_and_strictly_bounded() {
        let mut temporary = tempfile::NamedTempFile::new().unwrap();
        let count = run_to_file(
            command("binary"),
            &Cancellation::default(),
            Duration::from_secs(5),
            &mut temporary,
            4096,
        )
        .unwrap();
        let bytes = std::fs::read(temporary.path()).unwrap();
        assert_eq!(count, bytes.len() as u64);
        assert!(bytes
            .windows(b"binary-start\0\xff\r\nbinary-end".len())
            .any(|v| v == b"binary-start\0\xff\r\nbinary-end"));
        let error = run_to_file(
            command("flood"),
            &Cancellation::default(),
            Duration::from_secs(5),
            &mut temporary,
            512,
        )
        .unwrap_err();
        assert_eq!(error.code, "limit");
        assert!(temporary.as_file().metadata().unwrap().len() <= 512);
        assert!(run_to_file(
            command("fail"),
            &Cancellation::default(),
            Duration::from_secs(5),
            &mut temporary,
            4096
        )
        .is_err());
        assert_eq!(
            run_to_file(
                command("wait"),
                &Cancellation::default(),
                Duration::from_millis(100),
                &mut temporary,
                4096
            )
            .unwrap_err()
            .code,
            "timeout"
        );
        let signal = Cancellation::default();
        let copy = signal.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            copy.cancel();
        });
        assert_eq!(
            run_to_file(
                command("wait"),
                &signal,
                Duration::from_secs(5),
                &mut temporary,
                4096
            )
            .unwrap_err()
            .code,
            "cancelled"
        );
        trigger.join().unwrap();
    }
    #[test]
    fn oversized_file_is_rejected_even_when_child_succeeds() {
        let output = tempfile::NamedTempFile::new().unwrap();
        let mut command = command("file");
        command.env("FILEFORM_TEST_OUTPUT", output.path());
        let result = run_with_output_limit(
            command,
            &Cancellation::default(),
            Duration::from_secs(5),
            4096,
            Some((output.path(), 1024)),
        );
        assert_eq!(result.unwrap_err().code, "limit");
    }
    #[test]
    fn timeout_and_cancel_reap_waiting_child() {
        let start = Instant::now();
        assert_eq!(
            run(
                command("wait"),
                &Cancellation::default(),
                Duration::from_millis(100),
                4096
            )
            .unwrap_err()
            .code,
            "timeout"
        );
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        let cancel = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal.cancel();
        });
        assert_eq!(
            run(command("wait"), &cancellation, Duration::from_secs(5), 4096)
                .unwrap_err()
                .code,
            "cancelled"
        );
        cancel.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(4));
    }
}
