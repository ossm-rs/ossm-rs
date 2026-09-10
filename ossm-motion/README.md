# OSSM-RS Motion

This crate contains the core OSSM-RS motion control code

## Architecture Overview

### config
- All the user-confirurable parameters

### motion_control
- Computes the paths for comamnds like: "go to x mm with a velocity of y mm/s"
- Sets the position at which the motor should be at
- Verifies that all the machine constraints like min/max position/velocity are met, either by saturating the bounds or by panicking when exceeded

#### motor
- The `Motor` trait to be implemented by crates that want to use `MotionControl`

#### timer
- The `Timer` trait to be implemented by crates that want to use `MotionControl`

### motion
- Handles pattern execution
- Computs the pattern move which is then commanded to `motion_control`
- Handles pause behaviour with options to retract or stop

#### motion_state
- Global atomic state that is used by `motion` to then be passed on to the current pattern
- Crates can set this directly using some sort of user input to control the pattern

### pattern
- Defines the `Pattern` trait as well as the corresponding `PatternInput` and `PatternMove`
- Has a list of all patterns and a `PatternExecutor` (which also implements the `Pattern` trait), but can be used to set the current pattern and get moves from it

### utils
- Small utility functions

## How To

Using for custom development:

- Implement the `Motor` and `Timer` traits
- Create an instance of `MotionControl` and call the `update_handler()` function every `MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS` ms (main control loop)
- Run the forever running async `run_motion()` task in a thread (runs the pattern executor)
- Call the functions in `motion_state` in % or in mm to set the desired values for the pattern
