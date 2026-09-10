use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

const MIN_STEPS: f64 = 2.0;
const MAX_STEPS: f64 = 22.0;

pub struct Pull;

impl Pattern for Pull {
    const NAME: &'static str = "Pull";
    const DESCRIPTION: &'static str = "Pull stutters it's way out. Sensation controls the number of steps.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            let num_steps = (ctx.scale_sensation(MIN_STEPS, MAX_STEPS) as usize).max(1);

            for step in (0..=num_steps).rev() {
                ctx.motion().position(step as f64 / num_steps as f64).send().await?;
            }
        }
    }
}
