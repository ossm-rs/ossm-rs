use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

const PROJECT_NAME: &str = "ossm-rs";
const BOARDS: [&str; 8] = [
    "waveshare",
    "seeed_xiao_s3",
    "atom_s3",
    "ossm_v3",
    "custom_s3",
    "custom_c6",
    "ossm_alt_v2",
    "ossm_alt_v3",
];
const BINARIES_OUTPUT_DIR: &str = "release_binaries";

type DynError = Box<dyn std::error::Error>;

struct Board {
    name: String,
    mcu: Mcu,
    flash_mb: u8,
}

impl FromStr for Board {
    type Err = DynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, mcu) = match s {
            x @ "waveshare"
            | x @ "seeed_xiao_s3"
            | x @ "atom_s3"
            | x @ "ossm_v3"
            | x @ "ossm_alt_v3"
            | x @ "custom_s3" => (x, Mcu::Esp32S3),
            x @ "custom_c6" | x @ "ossm_alt_v2" => (x, Mcu::Esp32C6),
            x => Err(format!("Invalid board: {}", x))?,
        };

        let flash_mb = match s {
            "waveshare" | "ossm_v3" => 16,
            "seeed_xiao_s3" | "atom_s3" | "custom_s3" | "ossm_alt_v3" => 8,
            "custom_c6" | "ossm_alt_v2" => 4,
            x => Err(format!("Invalid board: {}", x))?,
        };

        Ok(Board {
            name: name.to_string(),
            mcu,
            flash_mb,
        })
    }
}

struct Toolchain {
    channel: String,
    components: Option<Vec<String>>,
    targets: Option<Vec<String>>,
}

enum Mcu {
    Esp32S3,
    Esp32C6,
}

impl Mcu {
    fn target_triple(&self) -> &str {
        match self {
            Mcu::Esp32S3 => "xtensa-esp32s3-none-elf",
            Mcu::Esp32C6 => "riscv32imac-unknown-none-elf",
        }
    }

    fn chip(&self) -> &str {
        match self {
            Mcu::Esp32S3 => "esp32s3",
            Mcu::Esp32C6 => "esp32c6",
        }
    }

    fn toolchain(&self) -> Toolchain {
        match self {
            Mcu::Esp32S3 => Toolchain {
                channel: "esp".to_string(),
                components: None,
                targets: None,
            },
            Mcu::Esp32C6 => Toolchain {
                channel: "stable".to_string(),
                components: Some(vec!["rust-src".to_string()]),
                targets: Some(vec!["riscv32imac-unknown-none-elf".to_string()]),
            },
        }
    }
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("run") => {
            let board_arg = env::args().nth(2);
            if let Some(board) = board_arg {
                let board = Board::from_str(&board)?;
                run_cargo_cmd("run", &board)?
            } else {
                Err("Board not gived")?
            }
        }
        Some("build-all") => build_all()?,
        Some("clean") => clean()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "
Available Tasks:
run: builds and runs the firmware
build-all: builds all the firmware binaries
clean: remove all the built files
"
    )
}

fn run_cargo_cmd(cmd: &str, board: &Board) -> Result<(), DynError> {
    let feature = format!("board_{}", board.name);

    println!("Starting the build for {}", board.name);
    println!("Building in {}", project_root().to_str().unwrap());

    let toolchain = board.mcu.toolchain().channel;

    let mut command = Command::new("cargo");
    let command = command
        .current_dir(project_root())
        .arg(format!("+{}", toolchain))
        .arg(cmd)
        .arg("--release")
        .args(["--target", board.mcu.target_triple()])
        .args(["--features", &feature]);

    let status = command.status()?;

    if !status.success() {
        Err("Failed to build ossm-rs")?;
    }

    Ok(())
}

fn build_all() -> Result<(), DynError> {
    let output_dir = project_root().join(BINARIES_OUTPUT_DIR);
    let elf_dir = output_dir.join("elf");
    let bin_dir = output_dir.join("bin");

    if !output_dir.exists() {
        fs::create_dir(&output_dir)?;
    }
    if !elf_dir.exists() {
        fs::create_dir(&elf_dir)?;
    }
    if !bin_dir.exists() {
        fs::create_dir(&bin_dir)?;
    }

    for board_str in BOARDS {
        let board = Board::from_str(board_str)?;
        let target_triple = board.mcu.target_triple();

        let build_out_file = project_root()
            .join("target")
            .join(target_triple)
            .join("release")
            .join(PROJECT_NAME);

        println!("Build out: {}", build_out_file.to_str().unwrap());

        run_cargo_cmd("build", &board)?;

        let elf_path = elf_dir.join(board_str).with_extension("elf");
        let bin_path = bin_dir.join(board_str).with_extension("bin");

        fs::copy(&build_out_file, &elf_path)?;

        let mut command = Command::new("espflash");
        let command = command
            .current_dir(project_root())
            .arg("save-image")
            .arg("--merge")
            .args(["--chip", board.mcu.chip()])
            .args(["--flash-size", &format!("{}mb", board.flash_mb)])
            .arg(
                elf_path
                    .to_str()
                    .expect("Could not convert elf path to string"),
            )
            .arg(
                bin_path
                    .to_str()
                    .expect("Could not convert bin path to string"),
            );

        let status = command.status()?;

        if !status.success() {
            Err("Failed to convert elf to bin")?;
        }
    }
    Ok(())
}

fn clean() -> Result<(), DynError> {
    let output_dir = project_root().join(BINARIES_OUTPUT_DIR);

    if let Err(err) = fs::remove_dir_all(output_dir) {
        if err.kind() != std::io::ErrorKind::NotFound {
            Err(err)?
        }
    }

    let status = Command::new("cargo")
        .current_dir(project_root())
        .arg("clean")
        .status()?;

    if !status.success() {
        Err("Failed to clean")?;
    }

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .join(PROJECT_NAME)
        .to_path_buf()
}
