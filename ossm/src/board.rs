use core::fmt::Debug;

/// The hardware abstraction for motor control.
///
/// A `Board` is a **position follower** — it receives a position in
/// millimetres every tick and makes the motor go there as fast as it can.
/// All trajectory planning (acceleration limits, jerk limits, smooth stops)
/// is handled by the [`MotionController`](crate::MotionController) using ruckig.
/// The board never decides its own path.
///
/// # Safety model
///
/// This design is deliberate: the OSSM enforces safe motion profiles at the
/// controller level via ruckig. The motor's internal trajectory planner (if it
/// has one) is configured for maximum tracking speed so it acts as a pure
/// position servo. Two trajectory planners in series would produce
/// unpredictable compounding behaviour — so we don't allow it.
///
/// # Units
///
/// - Positions in **millimetres** (mm)
/// - Torque as a **fraction** (0.0–1.0) of the motor's maximum output
///
/// The board converts mm to whatever the motor needs internally (steps,
/// register values, pulse intervals).
///
/// # Homing
///
/// `home()` is the one operation where the board takes full control. Different
/// boards home via different mechanisms:
/// - Modbus command (motor firmware handles it)
/// - Current sensing + ADC (board crawls, detects stall)
/// - Limit switches (GPIO)
///
/// The motion controller blocks during homing and does not send position
/// commands. Emergency stop during homing is handled at the board level.
#[allow(async_fn_in_trait)]
pub trait Board {
    type Error: Debug;

    /// Enable the motor driver. Must be called before any motion commands.
    ///
    /// The board should configure the motor for maximum tracking performance
    /// (max internal speed, max internal acceleration) so it follows position
    /// commands from the controller with minimal lag.
    async fn enable(&mut self) -> Result<(), Self::Error>;

    /// Disable the motor driver.
    async fn disable(&mut self) -> Result<(), Self::Error>;

    /// Run the full homing sequence and establish the coordinate origin.
    ///
    /// Returns when the motor is at a known position and ready for position
    /// commands. The board is responsible for:
    /// 1. Performing the homing motion (however the hardware supports it)
    /// 2. Zeroing its internal position reference
    /// 3. Configuring the motor for maximum tracking performance afterward
    ///
    /// The motion controller will move the motor to `min_position_mm` after
    /// homing completes — the board does not need to handle backoff.
    async fn home(&mut self) -> Result<(), Self::Error>;

    /// Command the motor to a position immediately.
    ///
    /// Called every tick (typically every 10ms) with the next point on the
    /// ruckig trajectory curve. The board converts mm to motor units and
    /// sends the command.
    ///
    /// The motor should be configured for maximum tracking speed so it
    /// reaches each commanded position before the next tick. If the motor
    /// falls behind, position error accumulates — this is a configuration
    /// problem (motor internal speed/accel too low), not a normal condition.
    async fn set_position(&mut self, position_mm: f64) -> Result<(), Self::Error>;

    /// Set the torque limit as a fraction of the motor's maximum output (0.0–1.0).
    ///
    /// What "torque" means is motor-specific:
    /// - For the 57AIM, this scales the StandstillMaxOutput register
    /// - For a stepper, this might control the current limit
    ///
    /// The board translates the fraction to the appropriate motor-specific value.
    async fn set_torque(&mut self, fraction: f64) -> Result<(), Self::Error>;

    /// Current position in millimetres from the home position.
    ///
    /// Used by the controller for telemetry and verification. For Modbus
    /// boards this may require a register read. For STEP/DIR boards this
    /// is computed from the step count.
    async fn position_mm(&mut self) -> Result<f64, Self::Error>;

    /// Periodic housekeeping, called every tick.
    ///
    /// Called every tick *before* `set_position()`. Use this for:
    /// - Polling fault/alarm registers
    /// - Updating cached telemetry
    /// - Any periodic maintenance the hardware needs
    ///
    /// For many boards this is a no-op. Returns an error if a critical fault
    /// is detected — the motion controller will transition to a safe state.
    async fn tick(&mut self) -> Result<(), Self::Error>;
}
