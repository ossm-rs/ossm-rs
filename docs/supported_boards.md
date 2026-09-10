# Supported Boards (tested to work)

## Recommended Boards

### WaveShare ESP32-S3-RS485-CAN

board_name: `waveshare`

![WaveShare ESP32-S3-RS485-CAN](images/ESP32-S3-RS485-CAN.webp)

### OSSM Alt PCB v3

board_name: `ossm_alt_v3`

A capabable board with 28V USB-PD.

The project can be found [here](https://github.com/jollydodo/OSSM-ALT-Edition)

![OSSM Alt PCB v3](images/OSSM-Alt-PCB-V3.webp)

## Alright Boards

### Seeed Studio XIAO ESP32-S3

board_name: `seeed_xiao_s3`

You will need:
- Seeed Studio XIAO ESP32-S3
- RS485 Breakout Board for Seeed Studio XIAO

Notice how the 5V switch is in the OUT position and the 120R switch is in the ON position

![Seeed Studio XIAO ESP32-S3](images/Seeed-Xiao-S3.webp)

### OSSM Reference PCB v3

board_name: `ossm_v3`

The not yet released board from [here](https://github.com/KinkyMakers/OSSM-hardware/tree/xpi/PCB-v3/Hardware/PCB%20Files/OSSM%20Reference%20PCB%20V3)

![OSSM Reference PCB v3](images/OSSM-Reference-PCB-v3.webp)

### OSSM Alt PCB v2 (Deprecated)

board_name: `ossm_alt_v2`

A cheap and compact C6 board with 28V USB-PD (Version 2 was never widely released. Please make sure you look at the correct board for the pin out (the polarities have changed!).

The project can be found [here](https://github.com/jollydodo/OSSM-ALT-Edition)

![OSSM Alt PCB v2](images/OSSM-Alt-PCB-V2.webp)

## Not Recommended Boards

### M5 Atom S3

board_name: `atom_s3`

#### ⚠️ This board does not work out of the box and requires soldering to make it work!

Instructions [here](Atomic_RS485_Base/Atomic_RS485_Base_Rework.md)

⚠️ This board cannot be powered directly from the motor power supply if you use above 24V!

This "board" consists of a few components from the M5Stack ecosystem:
- AtomS3-Lite
- Atomic RS485 Base
- Grove Cable 10 cm

![M5 Atom S3](images/Atom-S3.webp)

## Custom Boards

### Custom S3 Board

board_name: `custom_s3`

Set pins in `main.rs` manually

### Custom C6 Board

board_name: `custom_c6`

Set pins in `main.rs` manually
