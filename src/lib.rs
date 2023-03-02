#![feature(try_trait_v2)]

use core::fmt;
use core::ops::{ControlFlow, Try};

pub trait Context<T, Src> {
    #[track_caller]
    fn context<W: With<Src, Dst>, Dst>(self, ctx: W) -> Result<T, Dst>;
    #[track_caller]
    fn with_context<W: With<Src, Dst>, Dst>(self, ctx: impl FnMut() -> W) -> Result<T, Dst>;
}

pub trait With<Src, Dst> {
    #[track_caller]
    fn bind(self, residual: Src) -> Dst;
}

impl<T: Try> Context<T::Output, T::Residual> for T {
    fn context<W: With<T::Residual, Dst>, Dst>(self, ctx: W) -> Result<T::Output, Dst> {
        match self.branch() {
            ControlFlow::Continue(v) => Ok(v),
            ControlFlow::Break(cause) => Err(ctx.bind(cause)),
        }
    }

    fn with_context<W: With<T::Residual, Dst>, Dst>(
        self,
        mut ctx: impl FnMut() -> W,
    ) -> Result<T::Output, Dst> {
        match self.branch() {
            ControlFlow::Continue(v) => Ok(v),
            ControlFlow::Break(cause) => Err(ctx().bind(cause)),
        }
    }
}

pub use error_derive::Error;

pub struct Help {
    msg: String,
}

impl Help {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

impl fmt::Display for Help {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.msg, f)
    }
}
