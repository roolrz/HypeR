// SPDX-FileCopyrightText: The Rust Project Developers
// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::env::{CommandEnv, CommandEnvs, CommandResolvedEnvs};
pub use crate::ffi::OsString as EnvKey;
use crate::ffi::{OsStr, OsString};
use crate::num::NonZero;
use crate::path::Path;
use crate::process::StdioPipes;
use crate::sys::fs::File;
use crate::sys::pal::{cvt, ffi, unsupported};
use crate::{fmt, io};

////////////////////////////////////////////////////////////////////////////////
// Command
////////////////////////////////////////////////////////////////////////////////

pub struct Command {
    program: OsString,
    args: Vec<OsString>,
    env: CommandEnv,

    cwd: Option<OsString>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
}

#[derive(Debug)]
pub enum Stdio {
    Inherit,
    Null,
    MakePipe,
    ParentStdout,
    ParentStderr,
    #[allow(dead_code)] // This variant exists only for the Debug impl
    InheritFile(File),
    Pipe(ChildPipe),
}

impl Command {
    pub fn new(program: &OsStr) -> Command {
        Command {
            program: program.to_owned(),
            args: vec![program.to_owned()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn arg(&mut self, arg: &OsStr) {
        self.args.push(arg.to_owned());
    }

    pub fn env_mut(&mut self) -> &mut CommandEnv {
        &mut self.env
    }

    pub fn cwd(&mut self, dir: &OsStr) {
        self.cwd = Some(dir.to_owned());
    }

    pub fn stdin(&mut self, stdin: Stdio) {
        self.stdin = Some(stdin);
    }

    pub fn stdout(&mut self, stdout: Stdio) {
        self.stdout = Some(stdout);
    }

    pub fn stderr(&mut self, stderr: Stdio) {
        self.stderr = Some(stderr);
    }

    pub fn get_program(&self) -> &OsStr {
        &self.program
    }

    pub fn get_args(&self) -> CommandArgs<'_> {
        let mut iter = self.args.iter();
        iter.next();
        CommandArgs { iter }
    }

    pub fn get_envs(&self) -> CommandEnvs<'_> {
        self.env.iter()
    }

    pub fn get_env_clear(&self) -> bool {
        self.env.does_clear()
    }

    pub fn get_resolved_envs(&self) -> CommandResolvedEnvs {
        CommandResolvedEnvs::new(self.env.capture())
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.cwd.as_ref().map(|cs| Path::new(cs))
    }

    pub fn spawn(
        &mut self,
        default: Stdio,
        needs_stdin: bool,
    ) -> io::Result<(Process, StdioPipes)> {
        let environment = self.env.capture();
        let program = valid_text(&self.program)?;
        if self.cwd.as_ref().is_some_and(|cwd| cwd.is_empty()) {
            return Err(io::ErrorKind::NotFound.into());
        }
        let cwd = self
            .cwd
            .as_deref()
            .map(valid_text)
            .transpose()?
            .unwrap_or("");
        let candidates: Vec<crate::path::PathBuf> = if program.contains('/') {
            vec![program.into()]
        } else {
            let path = environment
                .get(OsStr::new("PATH"))
                .map(|value| valid_text(value))
                .transpose()?
                .unwrap_or("/bin");
            path.split(':')
                .map(|base| Path::new(if base.is_empty() { "." } else { base }).join(program))
                .collect()
        };
        let mut opened = None;
        let mut error = io::Error::from(io::ErrorKind::NotFound);
        for candidate in candidates {
            let candidate = if candidate.is_relative() && !cwd.is_empty() {
                Path::new(cwd).join(candidate)
            } else {
                candidate
            };
            let bytes = valid_text(candidate.as_os_str())?.as_bytes();
            let mut raw = 0;
            match cvt(unsafe {
                ffi::__hyper_std_process_begin(
                    bytes.as_ptr(),
                    bytes.len(),
                    cwd.as_ptr(),
                    cwd.len(),
                    &mut raw,
                )
            }) {
                Ok(()) => {
                    opened = Some(Builder(raw));
                    break;
                }
                Err(next) if next.kind() == io::ErrorKind::NotFound => error = next,
                Err(next) => return Err(next),
            }
        }
        let mut builder = opened.ok_or(error)?;
        for argument in &self.args {
            let bytes = valid_text(argument)?.as_bytes();
            cvt(unsafe {
                ffi::__hyper_std_process_argument(builder.0, bytes.as_ptr(), bytes.len(), 0)
            })?;
        }
        for (key, value) in environment {
            let key = valid_text(&key)?;
            if key.is_empty() || key.contains('=') {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            let value = valid_text(&value)?;
            let entry = format!("{key}={value}");
            cvt(unsafe {
                ffi::__hyper_std_process_argument(builder.0, entry.as_ptr(), entry.len(), 1)
            })?;
        }
        let stdin_default = if needs_stdin { &default } else { &Stdio::Null };
        let stdin = prepare_stdio(builder.0, 0, self.stdin.as_ref().unwrap_or(stdin_default))?;
        let stdout = prepare_stdio(builder.0, 1, self.stdout.as_ref().unwrap_or(&default))?;
        let stderr = prepare_stdio(builder.0, 2, self.stderr.as_ref().unwrap_or(&default))?;
        let mut process = 0;
        cvt(unsafe { ffi::__hyper_std_process_start(builder.0, &mut process) })?;
        builder.0 = 0;
        Ok((
            Process {
                handle: process,
                status: None,
            },
            StdioPipes {
                stdin,
                stdout,
                stderr,
            },
        ))
    }
}

fn valid_text(value: &OsStr) -> io::Result<&str> {
    let value = value.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    if value.as_bytes().contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(value)
}

struct Builder(u64);
impl Drop for Builder {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { ffi::__hyper_std_process_abort(self.0) };
        }
    }
}

fn prepare_stdio(builder: u64, stream: u32, option: &Stdio) -> io::Result<Option<ChildPipe>> {
    match option {
        Stdio::MakePipe | Stdio::Null => {
            let mut raw = 0;
            cvt(unsafe { ffi::__hyper_std_process_pipe(builder, stream, &mut raw) })?;
            let pipe = ChildPipe::from_owned(raw);
            if matches!(option, Stdio::MakePipe) {
                return Ok(Some(pipe));
            }
            if stream == 0 {
                drop(pipe);
            } else {
                // A null output is a userspace sink. Keeping a reader alive
                // preserves successful writes without a kernel null device.
                crate::thread::Builder::new().spawn(move || {
                    let mut buffer = [0; 4096];
                    while let Ok(count) = pipe.read(&mut buffer) {
                        if count == 0 {
                            break;
                        }
                    }
                })?;
            }
        }
        Stdio::Inherit | Stdio::ParentStdout | Stdio::ParentStderr => {
            let parent = match option {
                Stdio::ParentStdout => 1,
                Stdio::ParentStderr => 2,
                _ => stream,
            };
            cvt(unsafe { ffi::__hyper_std_process_inherit(builder, stream, 0, parent) })?;
        }
        Stdio::Pipe(pipe) => {
            let handle = pipe.handle_for_inheritance()?;
            cvt(unsafe { ffi::__hyper_std_process_inherit(builder, stream, handle, stream) })?;
        }
        Stdio::InheritFile(_) => return unsupported(),
    }
    Ok(None)
}

pub fn output(command: &mut Command) -> io::Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let (mut process, mut pipes) = command.spawn(Stdio::MakePipe, false)?;
    drop(pipes.stdin.take());
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    match (pipes.stdout.take(), pipes.stderr.take()) {
        (Some(out), Some(err)) => read_output(out, &mut stdout, err, &mut stderr)?,
        (Some(out), None) => {
            out.read_to_end(&mut stdout)?;
        }
        (None, Some(err)) => {
            err.read_to_end(&mut stderr)?;
        }
        (None, None) => {}
    }
    Ok((process.wait()?, stdout, stderr))
}

impl From<ChildPipe> for Stdio {
    fn from(pipe: ChildPipe) -> Self {
        Self::Pipe(pipe)
    }
}

impl From<io::Stdout> for Stdio {
    fn from(_: io::Stdout) -> Stdio {
        Stdio::ParentStdout
    }
}

impl From<io::Stderr> for Stdio {
    fn from(_: io::Stderr) -> Stdio {
        Stdio::ParentStderr
    }
}

impl From<File> for Stdio {
    fn from(file: File) -> Stdio {
        Stdio::InheritFile(file)
    }
}

impl fmt::Debug for Command {
    // show all attributes
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let mut debug_command = f.debug_struct("Command");
            debug_command
                .field("program", &self.program)
                .field("args", &self.args);
            if !self.env.is_unchanged() {
                debug_command.field("env", &self.env);
            }

            if self.cwd.is_some() {
                debug_command.field("cwd", &self.cwd);
            }

            if self.stdin.is_some() {
                debug_command.field("stdin", &self.stdin);
            }
            if self.stdout.is_some() {
                debug_command.field("stdout", &self.stdout);
            }
            if self.stderr.is_some() {
                debug_command.field("stderr", &self.stderr);
            }

            debug_command.finish()
        } else {
            if let Some(ref cwd) = self.cwd {
                write!(f, "cd {cwd:?} && ")?;
            }
            if self.env.does_clear() {
                write!(f, "env -i ")?;
                // Altered env vars will be printed next, that should exactly work as expected.
            } else {
                // Removed env vars need the command to be wrapped in `env`.
                let mut any_removed = false;
                for (key, value_opt) in self.get_envs() {
                    if value_opt.is_none() {
                        if !any_removed {
                            write!(f, "env ")?;
                            any_removed = true;
                        }
                        write!(f, "-u {} ", key.to_string_lossy())?;
                    }
                }
            }
            // Altered env vars can just be added in front of the program.
            for (key, value_opt) in self.get_envs() {
                if let Some(value) = value_opt {
                    write!(f, "{}={value:?} ", key.to_string_lossy())?;
                }
            }
            if self.program != self.args[0] {
                write!(f, "[{:?}] ", self.program)?;
            }
            write!(f, "{:?}", self.args[0])?;

            for arg in &self.args[1..] {
                write!(f, " {:?}", arg)?;
            }
            Ok(())
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitStatus {
    reason: u32,
    status: i64,
}
impl Default for ExitStatus {
    fn default() -> Self {
        Self {
            reason: 3,
            status: 0,
        }
    }
}
impl ExitStatus {
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        if self.code() == Some(0) {
            Ok(())
        } else {
            Err(ExitStatusError(*self))
        }
    }
    pub fn code(&self) -> Option<i32> {
        if matches!(self.reason, 2 | 3 | 4) {
            i32::try_from(self.status).ok()
        } else {
            None
        }
    }
}
impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code() {
            Some(code) => write!(f, "exit status: {code}"),
            None => write!(
                f,
                "Native termination reason {} (detail {})",
                self.reason, self.status
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitStatusError(ExitStatus);
impl From<ExitStatusError> for ExitStatus {
    fn from(error: ExitStatusError) -> Self {
        error.0
    }
}
impl ExitStatusError {
    pub fn code(self) -> Option<NonZero<i32>> {
        self.0.code().and_then(NonZero::new)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitCode(u8);

impl ExitCode {
    pub const SUCCESS: ExitCode = ExitCode(0);
    pub const FAILURE: ExitCode = ExitCode(1);

    pub fn as_i32(&self) -> i32 {
        self.0 as i32
    }
}

impl From<u8> for ExitCode {
    fn from(code: u8) -> Self {
        Self(code)
    }
}

pub struct Process {
    handle: u64,
    status: Option<ExitStatus>,
}
impl Drop for Process {
    fn drop(&mut self) {
        unsafe { ffi::__hyper_std_fs_close(self.handle) };
    }
}
impl Process {
    pub fn id(&self) -> u32 {
        let mut id = 0;
        if cvt(unsafe { ffi::__hyper_std_process_id(self.handle, &mut id) }).is_err() {
            panic!("cannot inspect Native process identity");
        }
        match u32::try_from(id) {
            Ok(id) => id,
            Err(_) => panic!("Native KOID exceeds Rust's process ID range"),
        }
    }
    pub fn kill(&mut self) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_process_kill(self.handle) })
    }
    fn poll(&mut self, block: bool) -> io::Result<Option<ExitStatus>> {
        if self.status.is_some() {
            return Ok(self.status);
        }
        let (mut done, mut reason, mut status) = (0, 0, 0);
        cvt(unsafe {
            ffi::__hyper_std_process_wait(
                self.handle,
                block as u32,
                &mut done,
                &mut reason,
                &mut status,
            )
        })?;
        if done != 0 {
            self.status = Some(ExitStatus { reason, status });
        }
        Ok(self.status)
    }
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.poll(true)?.ok_or(io::ErrorKind::Other.into())
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.poll(false)
    }
}

pub struct CommandArgs<'a> {
    iter: crate::slice::Iter<'a, OsString>,
}

impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;
    fn next(&mut self) -> Option<&'a OsStr> {
        self.iter.next().map(|os| &**os)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for CommandArgs<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}

impl<'a> fmt::Debug for CommandArgs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter.clone()).finish()
    }
}

pub type ChildPipe = crate::sys::pipe::Pipe;

pub fn read_output(
    out: ChildPipe,
    stdout: &mut Vec<u8>,
    err: ChildPipe,
    stderr: &mut Vec<u8>,
) -> io::Result<()> {
    // Earlier small reads may have consumed a whole Native message into the
    // adapter buffer, clearing kernel readiness while retaining a suffix.
    out.drain_buffered(stdout)?;
    err.drain_buffered(stderr)?;
    let (mut set, mut out_id, mut err_id) = (0, 0, 0);
    cvt(unsafe {
        ffi::__hyper_std_wait_pair(
            out.handle(),
            err.handle(),
            &mut set,
            &mut out_id,
            &mut err_id,
        )
    })?;
    struct Set(u64);
    impl Drop for Set {
        fn drop(&mut self) {
            unsafe { ffi::__hyper_std_fs_close(self.0) };
        }
    }
    let set = Set(set);
    // Consume an entire Native message on each notification. Smaller buffers
    // could leave a userspace suffix after the kernel readable level cleared.
    let mut bytes = vec![0; 65536];
    let (mut out_done, mut err_done) = (false, false);
    while !out_done || !err_done {
        let mut id = 0;
        cvt(unsafe { ffi::__hyper_std_wait_ready(set.0, &mut id) })?;
        let (pipe, output, done) = if id == out_id {
            (&out, &mut *stdout, &mut out_done)
        } else if id == err_id {
            (&err, &mut *stderr, &mut err_done)
        } else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        let count = match pipe.try_read(&mut bytes) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                cvt(unsafe { ffi::__hyper_std_wait_rearm(set.0, id) })?;
                continue;
            }
            Err(error) => return Err(error),
        };
        if count == 0 {
            *done = true;
            cvt(unsafe { ffi::__hyper_std_wait_remove(set.0, id) })?;
        } else {
            output.extend_from_slice(&bytes[..count]);
            cvt(unsafe { ffi::__hyper_std_wait_rearm(set.0, id) })?;
        }
    }
    Ok(())
}

pub fn getpid() -> u32 {
    match u32::try_from(unsafe { ffi::__hyper_std_current_process_id() }) {
        Ok(id) => id,
        Err(_) => panic!("Native KOID exceeds Rust process ID range"),
    }
}
