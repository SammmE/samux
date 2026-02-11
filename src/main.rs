use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::env;
use std::process::{Command, exit};

fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // parse mode from CLI
    let args: Vec<String> = env::args().collect();
    let prog = &args[0];

    let user_disk = "user_disk.img";

    // check that user disk exists
    if !std::path::Path::new(user_disk).exists() {
        eprintln!(
            "User disk image '{}' not found. Please create it before running the VM.",
            user_disk
        );
        exit(1);
    }

    // copy user disk to old_user_disk.img. If it already exists, do old_user_disk_[1-inf].img
    let mut backup_index = 0;
    let backup_disk = loop {
        let backup_name = if backup_index == 0 {
            "old_user_disk.img".to_string()
        } else {
            format!("old_user_disk_{}.img", backup_index)
        };
        if !std::path::Path::new(&backup_name).exists() {
            break backup_name;
        }
        backup_index += 1;
    };
    std::fs::copy(user_disk, &backup_disk).expect("failed to backup user disk image");
    println!("Backed up '{}' to '{}'", user_disk, backup_disk);

    // choose whether to start the UEFI or BIOS image
    let uefi = match args.get(1).map(|s| s.to_lowercase()) {
        Some(ref s) if s == "uefi" => true,
        Some(ref s) if s == "bios" => false,
        Some(ref s) if s == "-h" || s == "--help" => {
            println!("Usage: {prog} [uefi|bios]");
            println!("  uefi  - boot using OVMF (UEFI)");
            println!("  bios  - boot using legacy BIOS");
            exit(0);
        }
        _ => {
            eprintln!("Usage: {prog} [uefi|bios]");
            exit(1);
        }
    };

    let mut cmd = Command::new("qemu-system-x86_64");
    // print serial output to the shell
    cmd.arg("-serial").arg("mon:stdio");
    // enable the guest to exit qemu
    cmd.arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    cmd.arg("-drive").arg(format!(
        "file={},format=raw,if=ide,index=1,media=disk",
        user_disk
    ));

    if uefi {
        let prebuilt =
            Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to update prebuilt");

        let code = prebuilt.get_file(Arch::X64, FileType::Code);
        let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

        cmd.arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=0,file={},readonly=on",
            code.display()
        ));
        // copy vars and enable rw instead of snapshot if you want to store data (e.g. enroll secure boot keys)
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={},snapshot=on",
            vars.display()
        ));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }

    // restore the old user disk, and store the current one in user_disk_new[0-inf].img
    let mut new_index = 0;
    let new_disk = loop {
        let new_name = if new_index == 0 {
            "user_disk_new.img".to_string()
        } else {
            format!("new_user_disk{}.img", new_index)
        };
        if !std::path::Path::new(&new_name).exists() {
            break new_name;
        }
        new_index += 1;
    };
    std::fs::rename(user_disk, &new_disk).expect("failed to store new user disk image");
    std::fs::copy(&backup_disk, user_disk).expect("failed to restore old user disk image");
    println!("Stored new user disk image as '{}'", new_disk);

    let mut child = cmd.spawn().expect("failed to start qemu-system-x86_64");
    let status = child.wait().expect("failed to wait on qemu");
    match status.code().unwrap_or(1) {
        0x10 => 0, // success
        0x11 => 1, // failure
        _ => 2,    // unknown fault
    };
}
