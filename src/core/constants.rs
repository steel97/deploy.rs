pub static VERSION: &str = env!("CARGO_PKG_VERSION");
pub static SUDO_PREPEND: &str = "sudo ";
pub static CHUNK_UPLOAD_RETRIES: u32 = 5;
pub static CHUNK_UPLOAD_BUFFER: usize = 40_960;
