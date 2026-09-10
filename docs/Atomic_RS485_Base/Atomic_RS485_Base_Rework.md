# Atomic RS485 Base Rework

## Why Is This Necessary

This board has a hack to save an extra pin for enabling the RS485 driver output.

The driver is enabled only on 1 on the TX pin.
On 0 the transceiver outputs are in high impedance mode and the pull up/down resistors are used on A/B to drive the bus.

This makes the signal completely unusable at 115200 bps
and almost completely unusable at 19200 bps (motor responds, but too glitchy to work).

## Schematic

![Schematic](schematic.webp)

## Rework

### 1. Bend the DI pin to disconnect it from ground

![Bent DI pin](bent_pin.webp)

### 2. Remove the pull up/down 4.7 kΩ resistors

![Removed pull up/down resistors](removed_pull_up_down.webp)

### 3. Remove the transistor and the 4.7 kΩ resistor

![Removed transistor and resistor](removed_transistor_resistor.webp)

### 4. Solder 2 wires

- From DI to where the base of the transistor was connected (right after the 1K resistor going to TX)
- From DE and _RE to pin labeled 23 on the board

![Finished Board](finished_board.webp)

### 5 Add a 100-150Ω termination resistor between A and B

![Termination Resistor](termination_resistor.webp)
