use embedded_hal_async::delay::DelayNs;

use crate::pattern::{Pattern, PatternCtx};

const MAX_STROKES: usize = 5;
const MIN_DELAY_MS: f64 = 100.0;
const MAX_DELAY_MS: f64 = 10_000.0;

pub struct StopNGo;

impl Pattern for StopNGo {
    const NAME: &'static str = "Stop'n'Go";
    const DESCRIPTION: &'static str = "Stops after a series of strokes. Sensation controls the delay.";

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
        let mut num_strokes: usize = 1;
        let mut counting_up = true;

        loop {
            for _ in 0..num_strokes {
                ctx.motion().position(1.0).send().await?;
                ctx.motion().position(0.0).send().await?;
            }

            let delay = ctx.scale_sensation(MIN_DELAY_MS, MAX_DELAY_MS) as u64;
            ctx.delay_ms(delay).await;

            if counting_up {
                if num_strokes >= MAX_STROKES {
                    counting_up = false;
                    num_strokes -= 1;
                } else {
                    num_strokes += 1;
                }
            } else if num_strokes <= 1 {
                counting_up = true;
                num_strokes += 1;
            } else {
                num_strokes -= 1;
            }
        }
    }
}
