#![feature(closure_track_caller, stmt_expr_attributes, try_blocks)]

mod api;
mod keymap;
mod perf;
mod render;

use expect_test::expect;
use zi_test::{TestContext, new};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
