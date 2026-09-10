use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct TeasingPounding;

impl Pattern for TeasingPounding {
    const NAME: &'static str = "Teasing Pounding";
    const DESCRIPTION: &'static str = "Alternating strokes. Sensation controls speed ratio of in and out strokes.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
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

            ctx.motion().position(1.0).jerk(in_jerk).send().await?;
            ctx.motion().position(0.0).jerk(out_jerk).send().await?;
        }
    }
}
