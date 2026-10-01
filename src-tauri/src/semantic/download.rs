use sha2::{Digest, Sha256};
use std::{
    fs::File,
    future::{poll_fn, Future},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
    time::Duration,
};

pub(super) const CANCEL_POLL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(super) enum ModelError {
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("语义索引已取消"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}
impl From<String> for ModelError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}
impl From<std::io::Error> for ModelError {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}
impl From<reqwest::Error> for ModelError {
    fn from(error: reqwest::Error) -> Self {
        // Redirects can contain temporary signed URLs; do not expose them.
        Self::Failed(format!(
            "公开模型下载失败，请检查网络后重试：{}",
            error.without_url()
        ))
    }
}

pub(super) type ModelResult<T> = Result<T, ModelError>;

pub(super) struct Control<'a> {
    pub cancel: Option<&'a AtomicBool>,
    pub report: &'a dyn Fn(&str),
}
impl Control<'_> {
    pub fn check(&self) -> ModelResult<()> {
        if self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            Err(ModelError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn phase(&self, message: &str) -> ModelResult<()> {
        self.check()?;
        (self.report)(message);
        self.check()
    }
}

pub(super) fn client() -> ModelResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .referer(false)
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        // Preserve default environment proxy routing. This client never
        // reads HF tokens or sets origin authentication/cookies.
        .build()?)
}

async fn cancellable<F: Future>(future: F, control: &Control<'_>) -> ModelResult<F::Output> {
    control.check()?;
    let mut future = Box::pin(future);
    let mut wake = Box::pin(tokio::time::sleep(CANCEL_POLL));
    // Keep the same HTTP future pinned across timer ticks. On cancellation it
    // is dropped here, rather than abandoning a still-writing worker thread.
    poll_fn(|cx| {
        if let Err(error) = control.check() {
            return Poll::Ready(Err(error));
        }
        if let Poll::Ready(value) = future.as_mut().poll(cx) {
            return Poll::Ready(Ok(value));
        }
        if wake.as_mut().poll(cx).is_ready() {
            wake.as_mut()
                .reset(tokio::time::Instant::now() + CANCEL_POLL);
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    })
    .await
}

pub(super) async fn file(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    max_bytes: u64,
    exact_size: Option<u64>,
    control: &Control<'_>,
) -> ModelResult<()> {
    control.check()?;
    let mut response = cancellable(
        client
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send(),
        control,
    )
    .await??;
    if response.status() != reqwest::StatusCode::OK {
        return Err(ModelError::Failed(format!(
            "公开模型下载失败：HTTP {}，请稍后重试",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|size| size > max_bytes || exact_size.is_some_and(|expected| size != expected))
    {
        return Err(ModelError::Failed("公开模型响应大小不符合可信清单".into()));
    }
    control.check()?;
    let mut output = File::options()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut written = 0u64;
    while let Some(chunk) = cancellable(response.chunk(), control).await?? {
        written = written
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| ModelError::Failed("模型响应大小溢出".into()))?;
        if written > max_bytes {
            return Err(ModelError::Failed("公开模型响应超过安全大小上限".into()));
        }
        for part in chunk.chunks(1024 * 1024) {
            control.check()?;
            output.write_all(part)?;
        }
    }
    control.check()?;
    if written == 0 || exact_size.is_some_and(|expected| written != expected) {
        return Err(ModelError::Failed("公开模型下载不完整，已拒绝安装".into()));
    }
    output.sync_all()?;
    control.check()
}

pub(super) fn digest_reader(reader: &mut impl Read, control: &Control<'_>) -> ModelResult<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        control.check()?;
        let read = reader.read(&mut buffer)?;
        control.check()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn digest(path: &Path, control: &Control<'_>) -> ModelResult<String> {
    digest_reader(&mut File::open(path)?, control)
}

pub(super) fn read(path: &Path, max_bytes: u64, control: &Control<'_>) -> ModelResult<Vec<u8>> {
    control.check()?;
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > max_bytes {
        return Err(ModelError::Failed("模型文件超过安全大小上限".into()));
    }
    let capacity = usize::try_from(size)
        .map_err(|_| ModelError::Failed("模型文件大小超过当前平台上限".into()))?;
    // Match fs::read's exact initial capacity rather than doubling a 350 MB
    // model buffer while retaining cancellation boundaries between reads.
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        control.check()?;
        let size = file.read(&mut buffer)?;
        control.check()?;
        if size == 0 {
            break;
        }
        if bytes.len() as u64 + size as u64 > max_bytes {
            return Err(ModelError::Failed("模型文件超过安全大小上限".into()));
        }
        bytes.extend_from_slice(&buffer[..size]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, sync::Arc, thread, time::Instant};
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn destination() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "imagelore-download-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn stub(
        response: &'static [u8],
        stall: bool,
    ) -> (
        String,
        thread::JoinHandle<()>,
        std::sync::mpsc::Receiver<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let (send, received) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "loopback request was never received"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("loopback accept failed: {error}"),
                }
            };
            // Do not depend on a platform inheriting (or clearing) the
            // listener's nonblocking mode for an accepted socket.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0u8; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            let _ = send.send(());
            socket.write_all(response).unwrap();
            if stall {
                thread::sleep(Duration::from_millis(800));
            }
        });
        (format!("http://{address}/model"), worker, received)
    }
    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .read_timeout(Duration::from_millis(100))
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }
    #[test]
    fn streaming_rejects_http_errors_oversize_and_truncated_bodies() {
        for response in [
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nxxxx".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nx".as_slice(),
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nxxxx".as_slice(),
        ] {
            let (url, worker, _) = stub(response, false);
            let path = destination();
            let result = tauri::async_runtime::block_on(file(
                &test_client(),
                &url,
                &path,
                3,
                Some(3),
                &Control {
                    cancel: None,
                    report: &|_| {},
                },
            ));
            assert!(matches!(result, Err(ModelError::Failed(_))));
            worker.join().unwrap();
            let _ = std::fs::remove_file(path);
        }
    }
    #[test]
    fn streaming_accepts_a_complete_sized_body_and_total_deadline_does_not_reset_on_data() {
        let (url, worker, _) = stub(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc", false);
        let path = destination();
        tauri::async_runtime::block_on(file(
            &test_client(),
            &url,
            &path,
            3,
            Some(3),
            &Control {
                cancel: None,
                report: &|_| {},
            },
        ))
        .unwrap();
        worker.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        std::fs::remove_file(&path).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n")
                .unwrap();
            for _ in 0..10 {
                if socket.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .read_timeout(Duration::from_secs(30))
            .timeout(Duration::from_millis(120))
            .build()
            .unwrap();
        let path = destination();
        let start = Instant::now();
        let result = tauri::async_runtime::block_on(file(
            &client,
            &url,
            &path,
            10,
            Some(10),
            &Control {
                cancel: None,
                report: &|_| {},
            },
        ));
        assert!(matches!(result, Err(ModelError::Failed(_))));
        assert!(start.elapsed() < Duration::from_millis(700));
        worker.join().unwrap();
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn stalled_headers_and_body_have_finite_read_timeouts() {
        for response in [
            b"".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n".as_slice(),
        ] {
            let (url, worker, _) = stub(response, true);
            let path = destination();
            let start = Instant::now();
            let result = tauri::async_runtime::block_on(file(
                &test_client(),
                &url,
                &path,
                3,
                Some(3),
                &Control {
                    cancel: None,
                    report: &|_| {},
                },
            ));
            assert!(matches!(result, Err(ModelError::Failed(_))));
            assert!(start.elapsed() < Duration::from_millis(700));
            worker.join().unwrap();
            let _ = std::fs::remove_file(path);
        }
    }
    #[test]
    fn cancellation_wakes_a_pending_request_without_waiting_for_network_timeout() {
        for response in [
            b"".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n".as_slice(),
        ] {
            // Client construction can initialize TLS/certificate state on a
            // cold Windows runner. Finish it before starting the stub's
            // bounded request wait; cancellation timing starts below.
            let client = reqwest::Client::builder()
                .no_proxy()
                .read_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(900))
                .build()
                .unwrap();
            let (url, worker, received) = stub(response, true);
            let path = destination();
            let flag = Arc::new(AtomicBool::new(false));
            let setter = flag.clone();
            let cancel = thread::spawn(move || {
                received.recv_timeout(Duration::from_secs(2)).unwrap();
                thread::sleep(Duration::from_millis(30));
                setter.store(true, Ordering::Relaxed);
            });
            let start = Instant::now();
            let result = tauri::async_runtime::block_on(file(
                &client,
                &url,
                &path,
                3,
                Some(3),
                &Control {
                    cancel: Some(&flag),
                    report: &|_| {},
                },
            ));
            assert!(matches!(result, Err(ModelError::Cancelled)));
            assert!(start.elapsed() < Duration::from_millis(700));
            cancel.join().unwrap();
            worker.join().unwrap();
            let _ = std::fs::remove_file(path);
        }
    }
    #[test]
    fn hash_and_read_cancel_before_processing_the_next_chunk() {
        struct CancelReader<'a>(&'a AtomicBool, usize);
        impl Read for CancelReader<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.1 += 1;
                self.0.store(true, Ordering::Relaxed);
                buffer[0] = 1;
                Ok(1)
            }
        }
        let flag = AtomicBool::new(false);
        let mut reader = CancelReader(&flag, 0);
        assert!(matches!(
            digest_reader(
                &mut reader,
                &Control {
                    cancel: Some(&flag),
                    report: &|_| {}
                }
            ),
            Err(ModelError::Cancelled)
        ));
        assert_eq!(reader.1, 1);
        let path = destination();
        std::fs::write(&path, b"abc").unwrap();
        assert!(matches!(
            read(
                &path,
                3,
                &Control {
                    cancel: Some(&flag),
                    report: &|_| {}
                }
            ),
            Err(ModelError::Cancelled)
        ));
        std::fs::remove_file(path).unwrap();
    }
}
