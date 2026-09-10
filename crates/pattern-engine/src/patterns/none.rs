use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct NonePattern;

impl Pattern for NonePattern {
    const NAME: &'static str = "None";
    const DESCRIPTION: &'static str = "No pattern. Holds position.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            ctx.delay_ms(500).await;
        }
    }
}
