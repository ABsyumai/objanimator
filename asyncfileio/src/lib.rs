use anyhow::{anyhow, Result};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
pub use tokio::sync::mpsc::error::TryRecvError;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    task,
    time::sleep,
};
use util::SliceAs;

mod bufmanager;
pub use bufmanager::{BufPool, Buffer};
#[derive(Debug)]
pub enum Msg {
    Reload(usize),
    Step(isize),
    Terminate,
}
const SLEEP: u64 = 1;
/// 非同期にファイルを読み込むスレッドを管理する
#[derive(Debug)]
pub struct AsyncFileReader<T: 'static + Send> {
    pub tx: mpsc::Sender<Msg>,
    pub rx: mpsc::Receiver<Responce<T>>,
}
pub type Paths = Vec<String>;
type Responce<T> = Result<(usize, T)>;
impl<T: 'static + Send> AsyncFileReader<T> {
    const BUFFER_SIZE: usize = 1 << 3;
    /// コンストラクタ
    /// スレッドを一つ立ち上げる
    pub fn spawn(paths: Arc<Paths>, decoder: fn(Buffer) -> T) -> Self {
        let (tx, rx_th) = mpsc::channel(Self::BUFFER_SIZE);
        let (tx_th, rx) = mpsc::channel(Self::BUFFER_SIZE);
        let _ = thread::spawn(move || Self::spawn_inner(paths, tx_th, rx_th, decoder));
        Self { tx, rx }
    }
    async fn read_file(path: Arc<Paths>, index: usize, m: Arc<Mutex<BufPool>>) -> Responce<Buffer> {
        let mut file = fs::File::open(&path[index]).await?;
        let mut buf = m.lock().unwrap().get_buffer();
        file.read_to_end(buf.as_mut()).await?;
        Ok((index, buf))
    }
    /// 読み込んだファイルの内容を順番に外部に送信する
    async fn pop_send(
        tx: &mpsc::Sender<Responce<T>>,
        handles: &Arc<Mutex<VecDeque<task::JoinHandle<Responce<T>>>>>,
    ) -> task::JoinHandle<()> {
        let tx = tx.clone();
        let handles = Arc::clone(handles);
        task::spawn(async move {
            loop {
                sleep(Duration::from_micros(SLEEP)).await;
                let poped = handles.lock().unwrap().pop_front();
                // dbg!("pop sender get lock");
                if let Some(handle) = poped {
                    let handle: task::JoinHandle<Responce<T>> = handle;
                    let x: Responce<T> = handle.await.unwrap();
                    let _ = tx.send(x).await;
                } else {
                }
            }
        })
    }

    #[tokio::main(flavor = "current_thread")]
    async fn spawn_inner(
        paths: Arc<Paths>,
        tx: mpsc::Sender<Responce<T>>,
        mut rx: mpsc::Receiver<Msg>,
        decoder: fn(Buffer) -> T,
    ) {
        let mut index = 0;
        let mut step = 1;
        let manager = Arc::new(Mutex::new(BufPool::default()));
        let handles = Arc::new(Mutex::new(VecDeque::new()));
        let _ = Self::pop_send(&tx, &handles).await;
        loop {
            sleep(Duration::from_micros(SLEEP)).await;
            {
                let mut handles = handles.lock().unwrap();
                if handles.len() > Self::BUFFER_SIZE {
                    continue;
                }
                let p = Arc::clone(&paths);
                let m = Arc::clone(&manager);
                let h = task::spawn(async move {
                    let x = Self::read_file(p, index, m).await;
                    match x {
                        Ok((i, buf)) => {
                            let res = task::spawn_blocking(move || decoder(buf)).await.unwrap();
                            Ok((i, res))
                        }
                        Err(e) => Err(e),
                    }
                });
                handles.push_back(h);
            }

            use mpsc::error::TryRecvError::*;
            use Msg::*;
            //外部からの制御
            match rx.try_recv() {
                Ok(Reload(p)) => index = p,
                Ok(Step(s)) => step = s,
                Ok(Terminate) | Err(Disconnected) => break,
                _ => {
                    if step.is_positive() {
                        index += step as usize;
                    } else {
                        if index == 0 {
                            index = paths.len()
                        }
                        index -= step.abs() as usize;
                    }
                    index %= paths.len();
                }
            } //match
        } //loop
        let _ = tx.send(Err(anyhow!("terminated"))).await;
    }
}

pub struct FileConverter {
    handle: thread::JoinHandle<Result<()>>,
}

type Vertex = Vec<f32>;
type Converter = fn(String, Buffer) -> Result<(PathBuf, Vertex)>;
impl FileConverter {
    /// コンストラクタ
    pub fn spawn(paths: Paths, f: Converter) -> Self {
        let handle = thread::spawn(move || Self::spawn_inner(paths, f));
        Self { handle }
    }
    /// 実行を完了するまでブロックする
    pub fn stop(self) -> Result<()> {
        self.handle.join().expect("failed to join")
    }
    #[tokio::main(flavor = "current_thread")]
    async fn spawn_inner(paths: Paths, f: Converter) -> Result<()> {
        let bufpool = Arc::new(Mutex::new(BufPool::default()));
        let h = paths.into_iter().map(|p| {
            let pool = Arc::clone(&bufpool);
            task::spawn(async move {
                let file = &mut fs::File::open(&p).await?;
                let mut buf = pool.lock().unwrap().get_buffer();
                file.read_to_end(buf.as_mut()).await?;
                let (dst, v) = f(p, buf)?;
                let file = &mut fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(dst)
                    .await?;
                file.write(unsafe { v.slice_as()? }).await?;
                Result::<()>::Ok(())
            })
        });
        for i in h {
            i.await??;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
