pub mod ssh;
pub mod ssh_config;
pub mod simple_ssh;
pub mod russh_client;
pub mod file_transfer;
pub mod rzsz;
pub mod kitty_transfer;

pub use ssh::SshClient;
pub use ssh_config::import_ssh_config;
pub use simple_ssh::{connect_via_system_ssh, ssh_command_connect};
pub use russh_client::russh_connect;
pub use file_transfer::{
    upload_file, download_file, 
    upload_file_sftp, download_file_sftp,
    upload_file_kitty, download_file_kitty,
    upload_file_auto, download_file_auto
};
pub use rzsz::{is_rzsz_command, handle_rzsz};
pub use kitty_transfer::{is_kitty_available, upload_via_kitty, download_via_kitty}; 