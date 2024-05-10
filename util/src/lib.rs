use std::mem::size_of;
use std::{error, fmt};

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SliceAsError {
    from: usize,
    to: usize,
}

impl fmt::Display for SliceAsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "memory arraignment is invalid {:?}", self)
    }
}

impl error::Error for SliceAsError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

pub trait SliceAs {
    unsafe fn slice_as<T>(&self) -> Result<&[T], SliceAsError>;
    unsafe fn slice_as_unchecked<T>(&self) -> &[T];
}

impl<F> SliceAs for [F] {
    ///注意: 数値型にのみ使用する
    unsafe fn slice_as<T>(&self) -> Result<&[T], SliceAsError> {
        let from = size_of::<F>();
        let to = size_of::<T>();
        if self.len() * from % to != 0 {
            return Err(SliceAsError { to, from });
        }
        Ok(self.slice_as_unchecked())
    }

    #[inline]
    unsafe fn slice_as_unchecked<T>(&self) -> &[T] {
        let data = self.as_ptr() as *const T;
        let len = self.len() * size_of::<F>() / size_of::<T>();
        std::slice::from_raw_parts(data, len)
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
    fn test_slice_as() {
        use super::*;
        let x = vec![1_u8; 4];
        let y: &[u32] = unsafe { x.slice_as() }.unwrap();
        dbg!(format!("{:x}", y[0]));
        assert_eq!(y.len(), 1);

        let x = vec![0x11u8, 0x44];
        let y: &[u16] = unsafe { x.slice_as() }.unwrap();
        dbg!(format!("{:x}", y[0]));
        assert_eq!(y.len(), 1);
    }
}
