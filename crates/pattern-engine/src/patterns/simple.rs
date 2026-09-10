use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct Simple;

impl Pattern for Simple {
    const NAME: &'static str = "Simple Stroke";
    const DESCRIPTION: &'static str = "Simple in and out. Sensation does nothing.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            ctx.motion().position(1.0).send().await?;
            ctx.motion().position(0.0).send().await?;
        }
    }
}
