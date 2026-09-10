use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

pub struct RoboStroke;

impl Pattern for RoboStroke {
    const NAME: &'static str = "Robo Stroke";
    const DESCRIPTION: &'static str = "Sensation controls the acceleration from smooth to robotic";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        loop {
            let jerk = ctx.scale_sensation(0.0, 1.0);
            ctx.motion().position(1.0).jerk(jerk).send().await?;
            let jerk = ctx.scale_sensation(0.0, 1.0);
            ctx.motion().position(0.0).jerk(jerk).send().await?;
        }
    }
}
