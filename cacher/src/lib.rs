pub use asyncfileio::Msg;
use asyncfileio::{AsyncFileReader, BufPool, Buffer, Paths};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::sync::Arc;

pub struct Decoder<T, U, F>
where
    U: 'static + Send,
    F: FnMut(U, &mut BufPool) -> T,
{
    f: F,
    pool: BufPool,
    _p: PhantomData<T>,
    _pp: PhantomData<U>,
}
impl<T, U, F> Decoder<T, U, F>
where
    U: 'static + Send,
    F: FnMut(U, &mut BufPool) -> T,
{
    pub fn new(f: F) -> Self {
        Self {
            f,
            pool: BufPool::default(),
            _p: PhantomData,
            _pp: PhantomData,
        }
    }
    pub fn decode(&mut self, buf: U) -> T {
        (self.f)(buf, &mut self.pool)
    }
}
pub struct Cacher<T, U: 'static + Send, F: FnMut(U, &mut BufPool) -> T> {
    reader: AsyncFileReader<U>,
    map: Vec<Option<Weak<T>>>,
    que: VecDeque<Rc<T>>,
    decoder: Decoder<T, U, F>,
    pub que_max: usize,
    _paths: Arc<Paths>,
}

impl<T, U: 'static + Send, F: FnMut(U, &mut BufPool) -> T> Cacher<T, U, F> {
    /// f: ブロッキングスレッドで実行する
    /// g: イベントループで実行する
    pub fn new(que_max: usize, paths: Arc<Paths>, f: fn(Buffer) -> U, g: F) -> Self {
        let len = paths.len();
        Self {
            reader: AsyncFileReader::spawn(Arc::clone(&paths), f),
            map: vec![None; len],
            que: VecDeque::new(),
            decoder: Decoder::new(g),
            que_max,
            _paths: paths,
        }
    }
    /// メッセージの送信
    pub fn query(&self, msg: Msg) {
        let _ = self.reader.tx.blocking_send(msg);
    }

    /// キャッシュになければ非同期スレッドからの受信をを試みる
    pub fn get(&mut self, key: usize) -> Option<Rc<T>> {
        if let Some(Some(buf)) = self.map[key].as_ref().map(|v| v.upgrade()) {
            return Some(buf);
        }
        loop {
            let (k, vv) = match self.reader.rx.try_recv() {
                Ok(Ok(x)) => x,
                Err(asyncfileio::TryRecvError::Empty) => {
                    return None;
                }
                Ok(Err(e)) => {
                    dbg!(e);
                    return None;
                }
                _ => panic!("disconnected async file reader"),
            };
            let v = self.decoder.decode(vv);
            let is_key = k == key;
            let buf = self.insert(k, v);
            if is_key {
                return Some(buf);
            }
        }
    }
    fn insert(&mut self, k: usize, v: T) -> Rc<T> {
        let value = Rc::new(v);
        self.map.insert(k, Some(Rc::downgrade(&value)));
        self.que.push_back(Rc::clone(&value));
        while self.que.len() > self.que_max {
            self.que.pop_front();
        }
        value
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
