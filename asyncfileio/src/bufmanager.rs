use std::sync::mpsc;

/// ヒープ領域のメモリのプール
/// 返ってきたメモリはチャネルのキューに貯められる
#[derive(Debug)]
pub struct BufPool {
    tx: mpsc::SyncSender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
    default_capacity: usize,
}

impl Default for BufPool {
    fn default() -> Self {
        let (tx, rx) = mpsc::sync_channel(Self::BUFFER_SIZE);
        Self {
            tx,
            rx,
            default_capacity: 0,
        }
    }
}

impl BufPool {
    pub const BUFFER_SIZE: usize = 1 << 2;
    pub fn get_buffer(&mut self) -> Buffer {
        use mpsc::TryRecvError::*;
        let buf = match self.rx.try_recv() {
            Ok(recved) => {
                let cap = recved.capacity();
                if cap > self.default_capacity {
                    self.default_capacity = cap;
                }
                recved
            }
            Err(Empty) => Vec::with_capacity(self.default_capacity),
            _ => panic!("Sender must has self"),
        };
        Buffer::new(buf, self.tx.clone())
    }
    /// 外で確保したメモリをライフサイクルに組み込む
    pub fn add_buffer(&self, buf: Vec<u8>) -> Buffer {
        Buffer::new(buf, self.tx.clone())
    }
}

pub struct Buffer {
    buf: Vec<u8>,
    tx: mpsc::SyncSender<Vec<u8>>,
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("buf", &"buffer")
            .field("tx", &self.tx)
            .finish()
    }
}

impl Buffer {
    pub fn new(buf: Vec<u8>, tx: mpsc::SyncSender<Vec<u8>>) -> Self {
        Self { buf, tx }
    }
}

impl AsRef<Vec<u8>> for Buffer {
    fn as_ref(&self) -> &Vec<u8> {
        self.buf.as_ref()
    }
}

impl AsMut<Vec<u8>> for Buffer {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        self.buf.as_mut()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let mut buf = Vec::new(); //NOTE: no allocation
        std::mem::swap(&mut self.buf, &mut buf);
        buf.clear();
        let _ = self.tx.try_send(buf);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
    #[test]
    fn test_pool() {
        use super::*;
        let mut pool = BufPool::default();
        {
            let mut x = pool.get_buffer();
            let xx = x.as_mut();
            xx.push(1);
            xx.push(2);
        }
        let y = pool.get_buffer();
        let v = y.as_ref();
        dbg!(v.capacity());
        assert_eq!(v.len(), 0);
        assert_ne!(v.capacity(), 0);
    }
}
