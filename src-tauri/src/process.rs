use std::{
    io::{self, Read},
    process::{Child, Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Spawn a subprocess without flashing a console window on Windows.
pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn output(cmd: &mut Command) -> io::Result<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = cmd.spawn()?;
    wait_with_output(child)
}

pub fn wait_with_output(mut child: Child) -> io::Result<Output> {
    let timeout = command_timeout();
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    read_pipe(child.stdout.take(), PipeKind::Stdout, tx.clone());
    read_pipe(child.stderr.take(), PipeKind::Stderr, tx);
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;

    loop {
        while let Ok((kind, result)) = rx.try_recv() {
            match kind {
                PipeKind::Stdout => stdout = Some(result),
                PipeKind::Stderr => stderr = Some(result),
            }
        }

        if status.is_none() {
            status = child.try_wait()?;
        }

        if let (Some(status), Some(stdout), Some(stderr)) =
            (status, stdout.as_ref(), stderr.as_ref())
        {
            return Ok(Output {
                status,
                stdout: clone_reader_result(stdout)?,
                stderr: clone_reader_result(stderr)?,
            });
        }

        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("command timed out after {}s", timeout.as_secs()),
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Debug, Clone, Copy)]
enum PipeKind {
    Stdout,
    Stderr,
}

fn read_pipe<T>(pipe: Option<T>, kind: PipeKind, tx: mpsc::Sender<(PipeKind, io::Result<Vec<u8>>)>)
where
    T: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        let result = match pipe {
            Some(mut pipe) => pipe.read_to_end(&mut output).map(|_| output),
            None => Ok(output),
        };
        let _ = tx.send((kind, result));
    });
}

fn clone_reader_result(result: &io::Result<Vec<u8>>) -> io::Result<Vec<u8>> {
    result
        .as_ref()
        .map(Clone::clone)
        .map_err(|e| io::Error::new(e.kind(), e.to_string()))
}

fn command_timeout() -> Duration {
    std::env::var("COPY_DIFF_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(120))
}
