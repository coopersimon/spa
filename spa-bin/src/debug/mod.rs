
use spa::DebugInterface;

pub fn debug_mode(mut debug_interface: DebugInterface) {
    println!("Debug mode.");
    println!("Enter 'h' for help.");

    let mut breaks = std::collections::BTreeSet::new();
    let mut stack_trace = Vec::new();
    loop {
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => if input.starts_with("b:") {
                // Add breakpoint
                match u32::from_str_radix(&input[2..].trim(), 16) {
                    Ok(num) => {
                        println!("Inserted breakpoint at ${:08X}", num);
                        breaks.insert(num);
                    },
                    Err(e) => println!("Invalid breakpoint: {}", e),
                }
            } else if input.starts_with("c:") {
                // Remove breakpoint
                match u32::from_str_radix(&input[2..].trim(), 16) {
                    Ok(num) => {
                        println!("Cleared breakpoint at ${:08X}", num);
                        breaks.remove(&num);
                    },
                    Err(e) => println!("Invalid breakpoint: {}", e),
                }
            } else if input.starts_with("c") {
                // Remove all breakpoints
                println!("Cleared all breakpoints");
                breaks.clear();
            } else if input.starts_with("r") {
                // Run
                loop {
                    let state = debug_interface.get_state();
                    let loc = state.regs[15];
                    if breaks.contains(&loc) {
                        println!("Break at ${:08X}", loc);
                        break;
                    } else {
                        step_and_trace(&mut debug_interface, &mut stack_trace, false);
                    }
                }
            } else if input.starts_with("s:") {
                // Step x times
                match usize::from_str_radix(&input[2..].trim(), 10) {
                    Ok(num) => {
                        for _ in 0..num {
                            step_and_trace(&mut debug_interface, &mut stack_trace, true);
                        }
                    },
                    Err(e) => println!("Invalid number of steps: {}", e),
                }
            } else if input.starts_with("s") {
                // Step
                step_and_trace(&mut debug_interface, &mut stack_trace, true);
            } else if input.starts_with("p:") {
                // Print cpu or mem state
                print(&input[2..].trim(), &mut debug_interface);
            } else if input.starts_with("p") {
                // Print state
                print_all(&mut debug_interface);
            } else if input.starts_with("t") {
                let trace = stack_trace.iter()
                    .map(|n| format!("${:08X}", n))
                    .collect::<Vec<_>>()
                    .join("\n");
                println!("{}", trace);
            } else if input.starts_with("h") {
                // Help
                help();
            } else if input.starts_with("q") {
                break;
            },
            Err(e) => println!("Input error: {}", e),
        }
    }
}

fn print(s: &str, debug_interface: &mut DebugInterface) {
    if let Some(reg) = s.strip_prefix("r") {
        match usize::from_str_radix(reg, 10) {
            Ok(num) => println!("r{}: ${:08X}", num, debug_interface.get_state().regs[num]),
            Err(e) => println!("Invalid p tag: {}", e),
        }
    } else if let Some(bytes) = s.strip_prefix("b") {
        // Memory range
        if let Some(x) = bytes.find('-') {
            match u32::from_str_radix(&bytes[..x], 16) {
                Ok(start) => match u32::from_str_radix(&s[(x+1)..], 16) {
                    Ok(end) => {
                        println!("${:08X} - ${:08X}:", start, end);
                        let mems = (start..end).map(|n| format!("{:02X}", debug_interface.get_byte(n)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", mems);
                    },
                    Err(e) => println!("Invalid p tag: {}", e),
                },
                Err(e) => println!("Invalid p tag: {}", e),
            }
        } else {    // Single location
            match u32::from_str_radix(bytes, 16) {
                Ok(num) => println!("${:08X}: ${:02X}", num, debug_interface.get_byte(num)),
                Err(e) => println!("Invalid p tag: {}", e),
            }
        }
    } else if let Some(words) = s.strip_prefix("w") {
        // Memory range
        if let Some(x) = words.find('-') {
            match u32::from_str_radix(&words[..x], 16) {
                Ok(start) => match u32::from_str_radix(&s[(x+1)..], 16) {
                    Ok(end) => {
                        println!("${:08X} - ${:08X}:", start, end);
                        let mems = (start..end).map(|n| format!("{:08X}", debug_interface.get_word(n)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", mems);
                    },
                    Err(e) => println!("Invalid p tag: {}", e),
                },
                Err(e) => println!("Invalid p tag: {}", e),
            }
        } else {    // Single location
            match u32::from_str_radix(words, 16) {
                Ok(num) => println!("${:08X}: ${:08X}", num, debug_interface.get_word(num)),
                Err(e) => println!("Invalid p tag: {}", e),
            }
        }
    } else {
        match s {
            "cpsr" => println!("cpsr: ${:032b}", debug_interface.get_state().flags),
            _ => println!("unrecognised printable")
        }
    }
}

fn print_all(debug_interface: &mut DebugInterface) {
    let state = debug_interface.get_state();
    println!(" 0: {:08X} {:08X} {:08X} {:08X}", state.regs[0], state.regs[1], state.regs[2], state.regs[3]);
    println!(" 4: {:08X} {:08X} {:08X} {:08X}", state.regs[4], state.regs[5], state.regs[6], state.regs[7]);
    println!(" 8: {:08X} {:08X} {:08X} {:08X}", state.regs[8], state.regs[9], state.regs[10], state.regs[11]);
    println!("12: {:08X} {:08X} {:08X} {:08X}", state.regs[12], state.regs[13], state.regs[14], state.regs[15]);
    println!("flags: {:032b}", state.flags);
}

fn help() {
    println!("b:x: New breakpoint at memory location x (hex).");
    println!("c:x: Clear breakpoint at memory location x (hex).");
    println!("r: Keep running until a breakpoint is hit.");
    println!("s: Step a single instruction, and see the current instruction pipeline.");
    println!("s:x: Step multiple instructions (base 10).");
    println!("t: Print the stack trace (all the call locations).");
    println!("p: Print the current state of the CPU.");
    println!("p:rx: Print the register x.");
    println!("p:bx: Print the byte found at address x.");
    println!("p:bx-y: Print the memory in the range x -> y.");
    println!("q: Quit execution.");
}

// Step the CPU, and add the PC to the stack trace if it calls.
fn step_and_trace(debug_interface: &mut DebugInterface, _stack_trace: &mut Vec<u32>, print: bool) {
    let state = debug_interface.get_state();
    let instr = state.pipeline;
    if let Some(_executing) = &instr[2] {
        
    }
    
    if print {
        println!("${:08X} [{}]", state.regs[15], instr.iter().map(|i| if let Some(instr) = i {
            format!("{}", instr)
        } else {
            "_".to_string()
        }).collect::<Vec<_>>().join(" => "));
    }

    debug_interface.step();
}
