.global syscall_dispatcher

syscall_dispatcher:
    // 1. Swap GS. 
    // Before this, GS points to user data (or nothing).
    // After this, GS points to KERNEL_SCRATCH (stored in KernelGsBase MSR).
    swapgs

    // 2. Save the User Stack Pointer.
    // We move RSP into [GS:8] (offset 8 in KernelScratch struct)
    mov [gs:8], rsp

    // 3. Load the Kernel Stack Pointer.
    // We move [GS:0] (offset 0 in KernelScratch struct) into RSP
    mov rsp, [gs:0]

    // 4. Save User Registers (Context).
    // The C calling convention for x86_64 uses: RDI, RSI, RDX, RCX, R8, R9.
    // Note: The syscall instruction destroys RCX (stores RIP) and R11 (stores RFLAGS).
    // The syscall arguments usually come in: RDI, RSI, RDX, R10, R8, R9.
    // We push registers to preserve them.
    
    push r11  // Saved RFLAGS
    push rcx  // Saved RIP (Return Instruction Pointer)
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    // 5. Handle Arguments.
    // Syscall convention uses R10 for the 4th arg, but C functions expect RCX.
    // We copy R10 to RCX so our Rust function sees the args correctly.
    mov rcx, r10

    // 6. Call the Rust handler.
    // The return value will be in RAX.
    call syscall_rust_handler

    // 7. Restore Registers.
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop rcx  // Restore RIP into RCX
    pop r11  // Restore RFLAGS into R11

    // 8. Restore User Stack.
    mov rsp, [gs:8]

    // 9. Swap GS back to user mode.
    swapgs

    // 10. Return to Userspace.
    // This loads RIP from RCX and RFLAGS from R11.
    sysretq
