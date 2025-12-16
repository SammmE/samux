use crate::framebuffer::WRITER;
use crate::task::keyboard::ScancodeStream;
use crate::{print, println};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use futures_util::stream::StreamExt;
use pc_keyboard::{DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1, layouts};

pub async fn runshell() {
    let PROMPT: &str = "ferros> ";

    let mut scancode_stream = ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    loop {
        print!("{}", PROMPT);

        let mut input_buffer: String = String::new();

        while let Some(scancode) = scancode_stream.next().await {
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            match character {
                                '\n' => {
                                    println!();
                                    break;
                                }
                                '\x08' => {
                                    // Handle backspace
                                    if !input_buffer.is_empty() {
                                        input_buffer.pop();
                                        print!("\x08 \x08"); // Move back, print space, move back again
                                    }
                                }
                                _ => {
                                    input_buffer.push(character);
                                    print!("{}", character);
                                }
                            }
                        }
                        DecodedKey::RawKey(KeyCode::Backspace) => {
                            // Handle backspace
                            if !input_buffer.is_empty() {
                                input_buffer.pop();
                                print!("\x08 \x08"); // Move back, print space, move back again
                            }
                        }
                        DecodedKey::RawKey(_) => {}
                    }
                }
            }
        }

        let mut parts = input_buffer.split_whitespace();
        if let Some(command) = parts.next() {
            let args: Vec<&str> = parts.collect();
            let output = execute_command(command, &args);
            for line in output {
                println!("{}", line);
            }
        }
    }
}

fn execute_command(command: &str, args: &[&str]) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();

    match command {
        "help" => {
            output.push("Available commands:".to_string());
            output.push("  help - Show this help message".to_string());
            output.push("  echo [text] - Echo the provided text".to_string());
            output.push("  clear - Clear the screen".to_string());
            output.push("  reboot - Reboot the system".to_string());
        }
        "echo" => {
            let echoed = args.join(" ");
            output.push(echoed);
        }
        "clear" => {
            if let Some(writer) = WRITER.lock().as_mut() {
                writer.clear();
            }
        }
        "reboot" => {
            // Since we don't have ACPI shutdown yet, we'll force a QEMU exit
            // via our panic handler logic or just direct exit if exposed.
            // For now, let's just panic to trigger the shutdown hook.
            panic!("Reboot command issued!");
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }

    output
}
