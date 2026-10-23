use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::protocol::{Request, Response};
use crate::EngineError;

const STDERR_LINES: usize = 40;

/// The engine process. PoB takes 0.5 to 1 GB once a build is loaded, so it runs on its own and
/// can be dropped when idle, and an error or hang in PoB can't take the app down with it.
pub struct Engine {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<std::io::Result<String>>,
    stderr: Arc<Mutex<VecDeque<String>>>,
    next_id: u64,
    /// What the engine reported when it started: the PoB version it loaded.
    pub ready: Value,
}

impl Engine {
    /// Starts `exe` on the PoB folder and waits until PoB has loaded its data.
    pub fn start(exe: &Path, pob_root: &Path, startup: Duration) -> Result<Self, EngineError> {
        let mut child = Command::new(exe)
            .arg("--pob")
            .arg(pob_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(EngineError::Spawn)?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr_pipe = child.stderr.take().expect("piped stderr");

        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(VecDeque::new()));
        let tail = stderr.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr_pipe).lines().map_while(Result::ok) {
                let mut tail = tail.lock().unwrap();
                if tail.len() == STDERR_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });

        let mut engine = Self {
            child,
            stdin,
            lines,
            stderr,
            next_id: 1,
            ready: Value::Null,
        };
        engine.ready = engine.read(0, "startup", startup)?;
        Ok(engine)
    }

    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, EngineError> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            id,
            method: method.to_string(),
            params,
        };
        let mut line = serde_json::to_vec(&req).map_err(EngineError::Protocol)?;
        line.push(b'\n');
        if self
            .stdin
            .write_all(&line)
            .and_then(|_| self.stdin.flush())
            .is_err()
        {
            return Err(self.exited());
        }
        self.read(id, method, timeout)
    }

    fn read(&mut self, id: u64, method: &str, timeout: Duration) -> Result<Value, EngineError> {
        let line = match self.lines.recv_timeout(timeout) {
            Ok(Ok(line)) => line,
            Ok(Err(e)) => return Err(EngineError::Io(e)),
            Err(RecvTimeoutError::Disconnected) => return Err(self.exited()),
            Err(RecvTimeoutError::Timeout) => {
                // The engine is stuck in PoB. Its answer would arrive out of order, so it goes.
                let _ = self.child.kill();
                return Err(EngineError::Timeout(method.to_string()));
            }
        };
        let resp: Response = serde_json::from_str(&line).map_err(EngineError::Protocol)?;
        if resp.id != id {
            return Err(EngineError::Pob(format!(
                "answer {} to request {id}",
                resp.id
            )));
        }
        match (resp.result, resp.error) {
            (_, Some(e)) => Err(EngineError::Pob(e)),
            (Some(v), None) => Ok(v),
            (None, None) => Ok(Value::Null),
        }
    }

    fn exited(&mut self) -> EngineError {
        let _ = self.child.wait();
        // The stderr thread may still be draining the last lines.
        std::thread::sleep(Duration::from_millis(50));
        let tail = self.stderr.lock().unwrap();
        EngineError::Exited(tail.iter().cloned().collect::<Vec<_>>().join("\n"))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
