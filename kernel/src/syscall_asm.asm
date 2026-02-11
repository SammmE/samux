.intel_syntax noprefix
.global syscall_dispatcher

syscall_dispatcher:
    swapgs
    mov [gs:8], rsp
    mov rsp, [gs:0]

    push r11
    push rcx
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov rcx, r10
    call syscall_rust_handler

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop rcx
    pop r11

    mov rsp, [gs:8]
    swapgs
    sysretq
