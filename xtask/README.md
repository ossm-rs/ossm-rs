# OSSM-RS Xtask

Build system to build the firmware under `ossm-rs`.

Similar to a makefile, but in rust.

This crate handles:

- Using the correct toolchain for each board
- Enabling the correct features for each board
- Generating the final `.elf` and `.bin` for each board
