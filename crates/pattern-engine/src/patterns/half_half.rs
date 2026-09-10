use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct HalfHalf;

impl Pattern for HalfHalf {
    const NAME: &'static str = "Half'n'Half";
    const DESCRIPTION: &'static str = "Alternate between full and half strokes. Sensation controls speed ratio.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        let mut half = false;

        loop {
            let sensation = ctx.sensation();
            let factor = sensation.abs().clamp(0.0,1.0) * 0.5;

            let (out_jerk, in_jerk) = if sensation > 0.0 {
                (0.5 - factor, 0.5 + factor)
            } else if sensation < 0.0 {
                (0.5 + factor, 0.5 - factor)
            } else {
                (0.5, 0.5)
            };

            let depth = if half { 0.5 } else { 1.0 };
            half = !half;

            ctx.motion().position(depth).jerk(in_jerk).send().await?;
            ctx.motion().position(0.0).jerk(out_jerk).send().await?;
        }
    }
}
