use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct Knot;

impl Pattern for Knot {
    const NAME: &'static str = "Knot";
    const DESCRIPTION: &'static str = "Sensation controls the position of the knot transition.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            let position = ctx.scale_sensation(0.1, 0.9);
            ctx.motion().position(position).jerk(0.5).send().await?;
            ctx.motion().position(1.0).jerk(0.0).speed(0.7).send().await?;
            ctx.motion().position(0.0).jerk(0.0).speed(0.7).send().await?;
        }
    }
}
