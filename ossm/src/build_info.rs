/// Fixed-size container for build metadata, placed in a dedicated ELF section
/// so it can be stripped when hashing firmware for meaningful-change detection.
#[repr(C)]
pub struct BuildMeta {
    data: [u8; 256],
}

impl BuildMeta {
    /// Build from null-separated fields: `device\0commit\0release\0build_time\0`
    pub const fn new(raw: &[u8]) -> Self {
        let mut data = [0u8; 256];
        let mut i = 0;
        while i < raw.len() && i < 256 {
            data[i] = raw[i];
            i += 1;
        }
        Self { data }
    }

    fn field(&self, index: usize) -> &str {
        let mut start = 0;
        let mut count = 0;
        let mut i = 0;
        while i < self.data.len() {
            if self.data[i] == 0 {
                if count == index {
                    return match core::str::from_utf8(&self.data[start..i]) {
                        Ok(s) => s,
                        Err(_) => "?",
                    };
                }
                count += 1;
                start = i + 1;
            }
            i += 1;
        }
        "?"
    }

    pub fn device(&self) -> &str {
        self.field(0)
    }
    pub fn commit_hash(&self) -> &str {
        self.field(1)
    }
    pub fn release_id(&self) -> &str {
        self.field(2)
    }
    pub fn build_time(&self) -> &str {
        self.field(3)
    }
}

/// Logs build metadata from a dedicated ELF section (`.ossm.build_info`).
///
/// The calling crate's `build.rs` must write `ossm_build_info.bin` to `OUT_DIR`
/// as null-separated fields: `device\0commit\0release_id\0build_time\0`.
#[macro_export]
macro_rules! build_info {
    () => {{
        #[unsafe(link_section = ".ossm.build_info")]
        #[used]
        static OSSM_BUILD_META: $crate::BuildMeta = $crate::BuildMeta::new(
            include_bytes!(concat!(env!("OUT_DIR"), "/ossm_build_info.bin")),
        );

        log::info!(
            "{} {} (release: {}, built: {})",
            OSSM_BUILD_META.device(),
            OSSM_BUILD_META.commit_hash(),
            OSSM_BUILD_META.release_id(),
            OSSM_BUILD_META.build_time(),
        );
    }};
}
