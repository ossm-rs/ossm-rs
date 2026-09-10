use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

const MIN_STEPS: f64 = 2.0;
const MAX_STEPS: f64 = 22.0;

pub struct Hammerjack;

impl Pattern for Hammerjack {
    const NAME: &'static str = "Hammerjack";
    const DESCRIPTION: &'static str = "Jackhammer, but backwards. Sensation controls the number of steps.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            let num_steps = (ctx.scale_sensation(MIN_STEPS, MAX_STEPS) as usize).max(1);

            ctx.motion().position(1.0).jerk(0.0).send().await?;
            for step in (1..=num_steps-2).rev() {
                ctx.motion().position(step as f64 / num_steps as f64).jerk(1.0).send().await?;
                ctx.motion().position((step as f64 + 1.0)/ num_steps as f64).jerk(0.25).send().await?;
            }
            ctx.motion().position(0.0).jerk(1.0).send().await?;
        }
    }
}
