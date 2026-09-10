use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

const MIN_STEPS: f64 = 5.0;
const MAX_STEPS: f64 = 25.0;

pub struct Stutter;

impl Pattern for Stutter {
    const NAME: &'static str = "Stutter";
    const DESCRIPTION: &'static str = "Stutters in and out. Sensation adjusts number of steps and jerk. Full stroke remains relatively consistent with velocity. Vibration increases with sensation. ";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            let num_steps = (ctx.scale_sensation(MIN_STEPS, MAX_STEPS) as usize).max(1);
            let jerk = ctx.scale_sensation(0.0, 0.75) + 0.25;
            for step in 0..=num_steps {
                ctx.motion().position(step as f64 / num_steps as f64).jerk(jerk).send().await?;
            }
            let num_steps = (ctx.scale_sensation(MIN_STEPS, MAX_STEPS) as usize).max(1);
            let jerk = ctx.scale_sensation(0.0, 0.75) + 0.25;
            for step in (1..=num_steps-1).rev() {
                ctx.motion().position(step as f64 / num_steps as f64).jerk(jerk).send().await?;
            }
        }
    }
}
