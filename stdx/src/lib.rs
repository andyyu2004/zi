#![feature(array_windows)]
pub mod bomb;
pub mod bound;
pub mod iter;
pub mod merge;
pub mod path;
pub mod range;
pub mod slice;
pub mod sync;

#[macro_export]
macro_rules! dbg {
    ($e:expr) => {{
        let e = $e;
        tracing::error!("{}:{}: {} = {:#?}", file!(), line!(), stringify!($e), e);
        e
    }};
}
