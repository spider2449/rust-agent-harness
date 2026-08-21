use std::{
    env, fs,
    io::{self, Read, Write},
    process::{Command, ExitCode, Stdio},
    thread,
    time::Duration,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("fixture error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<u8, String> {
    let mut args = env::args().skip(1);
    let operation = args.next().ok_or("missing fixture operation")?;
    match operation.as_str() {
        "echo" => {
            let text = args.next().ok_or("missing echo text")?;
            reject_extra(args)?;
            print!("{text}");
            Ok(0)
        }
        "audited-echo" => {
            let audit_file = args.next().ok_or("missing audit file")?;
            let count_file = args.next().ok_or("missing count file")?;
            let text = args.next().ok_or("missing echo text")?;
            reject_extra(args)?;
            let count = fs::read_to_string(&count_file)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
                + 1;
            fs::write(&count_file, count.to_string()).map_err(|error| error.to_string())?;
            let mut byte = [0_u8; 1];
            let stdin_bytes = io::stdin()
                .read(&mut byte)
                .map_err(|error| error.to_string())?;
            let mut environment = env::vars_os()
                .map(|(name, value)| {
                    format!("{}={}", name.to_string_lossy(), value.to_string_lossy())
                })
                .collect::<Vec<_>>();
            environment.sort();
            let executable =
                fs::canonicalize(env::current_exe().map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let cwd = fs::canonicalize(env::current_dir().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
            let audit = format!(
                "execution_count={count}\npid={}\nexecutable={}\ncwd={}\ninput={text}\nstdin_bytes={stdin_bytes}\nprocess_in_job={}\nenvironment={}\n",
                std::process::id(),
                executable.display(),
                cwd.display(),
                process_in_job()?,
                environment.join("|")
            );
            fs::write(audit_file, audit).map_err(|error| error.to_string())?;
            print!("{text}");
            Ok(0)
        }
        "streams" => {
            let stdout_bytes = parse_usize(args.next(), "stdout byte count")?;
            let stderr_bytes = parse_usize(args.next(), "stderr byte count")?;
            reject_extra(args)?;
            let stdout = thread::spawn(move || write_bytes(io::stdout(), b'O', stdout_bytes));
            let stderr = thread::spawn(move || write_bytes(io::stderr(), b'E', stderr_bytes));
            stdout
                .join()
                .map_err(|_| "stdout writer panicked".to_owned())??;
            stderr
                .join()
                .map_err(|_| "stderr writer panicked".to_owned())??;
            Ok(0)
        }
        "delay" => {
            let delay = parse_u64(args.next(), "delay")?;
            reject_extra(args)?;
            thread::sleep(Duration::from_millis(delay));
            println!("delay-complete");
            Ok(0)
        }
        "exit" => {
            let code = parse_u8(args.next(), "exit code")?;
            reject_extra(args)?;
            Ok(code)
        }
        "audit-env" => {
            reject_extra(args)?;
            let mut environment = env::vars_os()
                .map(|(name, value)| {
                    format!("{}={}", name.to_string_lossy(), value.to_string_lossy())
                })
                .collect::<Vec<_>>();
            environment.sort();
            print!("{}", environment.join("\n"));
            Ok(0)
        }
        "cwd" => {
            reject_extra(args)?;
            print!(
                "{}",
                env::current_dir()
                    .map_err(|error| error.to_string())?
                    .display()
            );
            Ok(0)
        }
        "stdin-eof" => {
            reject_extra(args)?;
            let mut byte = [0_u8; 1];
            let count = io::stdin()
                .read(&mut byte)
                .map_err(|error| error.to_string())?;
            print!("{count}");
            Ok(0)
        }
        "pid-delay" => {
            let pid_file = args.next().ok_or("missing pid file")?;
            let delay = parse_u64(args.next(), "delay")?;
            reject_extra(args)?;
            fs::write(pid_file, std::process::id().to_string())
                .map_err(|error| error.to_string())?;
            thread::sleep(Duration::from_millis(delay));
            Ok(0)
        }
        "count-delay" => {
            let count_file = args.next().ok_or("missing count file")?;
            let delay = parse_u64(args.next(), "delay")?;
            reject_extra(args)?;
            let count = fs::read_to_string(&count_file)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
                + 1;
            fs::write(count_file, count.to_string()).map_err(|error| error.to_string())?;
            thread::sleep(Duration::from_millis(delay));
            Ok(0)
        }
        "spawn-child" => {
            let marker = args.next().ok_or("missing marker file")?;
            let child_pid_file = args.next().ok_or("missing child pid file")?;
            let delay = args.next().ok_or("missing delay")?;
            reject_extra(args)?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let child = Command::new(executable)
                .args(["delayed-write", &marker, &child_pid_file, &delay])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| error.to_string())?;
            println!("{}", child.id());
            thread::sleep(Duration::from_secs(30));
            Ok(0)
        }
        "delayed-write" => {
            let marker = args.next().ok_or("missing marker file")?;
            let pid_file = args.next().ok_or("missing pid file")?;
            let delay = parse_u64(args.next(), "delay")?;
            reject_extra(args)?;
            fs::write(pid_file, std::process::id().to_string())
                .map_err(|error| error.to_string())?;
            thread::sleep(Duration::from_millis(delay));
            fs::write(marker, "child-complete").map_err(|error| error.to_string())?;
            Ok(0)
        }
        _ => Err("unknown fixture operation".to_owned()),
    }
}

fn reject_extra(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if args.next().is_some() {
        Err("unexpected fixture argument".to_owned())
    } else {
        Ok(())
    }
}

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_u64(value: Option<String>, label: &str) -> Result<u64, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_u8(value: Option<String>, label: &str) -> Result<u8, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn write_bytes(mut writer: impl Write, byte: u8, count: usize) -> Result<(), String> {
    let chunk = [byte; 8 * 1024];
    let mut remaining = count;
    while remaining > 0 {
        let length = remaining.min(chunk.len());
        writer
            .write_all(&chunk[..length])
            .map_err(|error| error.to_string())?;
        remaining -= length;
    }
    writer.flush().map_err(|error| error.to_string())
}

#[cfg(windows)]
fn process_in_job() -> Result<bool, String> {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn IsProcessInJob(
            process_handle: *mut c_void,
            job_handle: *mut c_void,
            result: *mut i32,
        ) -> i32;
    }

    let mut result = 0_i32;
    let succeeded =
        unsafe { IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &raw mut result) };
    if succeeded == 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(result != 0)
    }
}

#[cfg(not(windows))]
fn process_in_job() -> Result<bool, String> {
    Ok(false)
}
