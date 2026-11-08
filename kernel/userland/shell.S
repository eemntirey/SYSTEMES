.section .text
.global _start
.type _start,@function
.set LINEBUF, 0x8000007000
_start:
    xor %r12d, %r12d
    movabs $LINEBUF, %r13
    lea 256(%r13), %r14
prompt:
    xor %r12d, %r12d
    lea prompt_text(%rip), %rsi
    mov $2, %edx
    call write_buf
read_char:
    mov $2, %eax
    mov $1, %edi
    mov %r14, %rsi
    mov $1, %edx
    syscall
    test %rax, %rax
    jle read_char
    movzbl (%r14), %eax
    cmp $10, %al
    je submit_line
    cmp $13, %al
    je submit_line
    cmp $8, %al
    je erase_char
    cmp $127, %al
    je erase_char
    cmp $32, %al
    jb read_char
    cmp $127, %r12d
    jae read_char
    movb %al, (%r13,%r12,1)
    inc %r12d
    mov $1, %eax
    mov $1, %edi
    mov %r14, %rsi
    mov $1, %edx
    syscall
    jmp read_char
erase_char:
    test %r12d, %r12d
    jz read_char
    dec %r12d
    lea erase_text(%rip), %rsi
    mov $3, %edx
    call write_buf
    jmp read_char
submit_line:
    lea newline_text(%rip), %rsi
    mov $2, %edx
    call write_buf
    movb $0, (%r13,%r12,1)
    test %r12d, %r12d
    jz prompt
    cmp $4, %r12d
    jne check_uptime
    cmpl $0x74697865, (%r13)
    je command_exit
    cmpl $0x706c6568, (%r13)
    je command_help
check_uptime:
    cmp $6, %r12d
    jne check_ls
    cmpl $0x69747075, (%r13)
    jne check_ls
    cmpw $0x656d, 4(%r13)
    je command_uptime
check_ls:
    cmp $2, %r12d
    jb check_cat
    cmpw $0x736c, (%r13)
    jne check_cat
    cmp $2, %r12d
    je command_ls
    cmpb $0x20, 2(%r13)
    jne check_cat
    jmp command_ls
check_cat:
    cmp $3, %r12d
    jne check_cat_args
    cmpw $0x6163, (%r13)
    jne check_cat_args
    cmpb $0x74, 2(%r13)
    je cat_usage
check_cat_args:
    cmp $4, %r12d
    jb check_echo
    cmpl $0x20746163, (%r13)
    jne check_echo
    mov %r12d, %esi
    sub $4, %esi
    test %esi, %esi
    jz cat_usage
    lea 4(%r13), %rdi
    mov $6, %eax
    syscall
    cmp $-2, %rax
    je cat_not_found
    cmp $-19, %rax
    je cat_no_filesystem
    test %rax, %rax
    js cat_usage
    mov %rax, %r15
cat_read:
    mov $2, %eax
    mov %r15, %rdi
    lea 512(%r13), %rsi
    mov $256, %edx
    syscall
    test %rax, %rax
    jle cat_close
    mov %rax, %rdx
    mov $1, %eax
    mov $1, %edi
    lea 512(%r13), %rsi
    syscall
    jmp cat_read
cat_close:
    mov $7, %eax
    mov %r15, %rdi
    syscall
    jmp prompt
cat_not_found:
    lea cat_not_found_text(%rip), %rsi
    mov $(cat_not_found_end-cat_not_found_text), %edx
    call write_buf
    jmp prompt
cat_no_filesystem:
    lea cat_no_filesystem_text(%rip), %rsi
    mov $(cat_no_filesystem_end-cat_no_filesystem_text), %edx
    call write_buf
    jmp prompt
cat_usage:
    lea cat_usage_text(%rip), %rsi
    mov $(cat_usage_end-cat_usage_text), %edx
    call write_buf
    jmp prompt
command_ls:
    cmp $2, %r12d
    je ls_root_path
    cmp $4, %r12d
    jb ls_usage
    lea 3(%r13), %rdi
    mov %r12d, %esi
    sub $3, %esi
    jmp ls_open
ls_root_path:
    lea root_path(%rip), %rdi
    mov $1, %esi
ls_open:
    mov $6, %eax
    syscall
    cmp $-19, %rax
    je ls_no_filesystem
    test %rax, %rax
    js ls_error
    mov %rax, %r15
ls_read:
    mov $8, %eax
    mov %r15, %rdi
    lea 512(%r13), %rsi
    mov $256, %edx
    syscall
    test %rax, %rax
    js ls_read_error
    test %rax, %rax
    jz ls_close
    mov %rax, %rdx
    mov $1, %eax
    mov $1, %edi
    lea 512(%r13), %rsi
    syscall
    jmp ls_read
ls_read_error:
    mov $7, %eax
    mov %r15, %rdi
    syscall
    jmp ls_error
ls_close:
    mov $7, %eax
    mov %r15, %rdi
    syscall
    jmp prompt
ls_no_filesystem:
    lea ls_no_filesystem_text(%rip), %rsi
    mov $19, %edx
    call write_buf
    jmp prompt
ls_usage:
    lea ls_usage_text(%rip), %rsi
    mov $11, %edx
    call write_buf
    jmp prompt
ls_error:
    lea ls_error_text(%rip), %rsi
    mov $11, %edx
    call write_buf
    jmp prompt
check_echo:
    cmp $5, %r12d
    jb command_unknown
    cmpl $0x6f686365, (%r13)
    jne command_unknown
    cmpb $0x20, 4(%r13)
    jne command_unknown
    mov %r12d, %edx
    sub $5, %edx
    jle prompt
    mov $1, %eax
    mov $1, %edi
    lea 5(%r13), %rsi
    syscall
    lea newline_text(%rip), %rsi
    mov $2, %edx
    call write_buf
    jmp prompt
command_uptime:
    mov $5, %eax
    syscall
    xor %edx, %edx
    mov $100, %ecx
    div %rcx
    lea 255(%r13), %rsi
    mov $10, %ecx
itoa_loop:
    xor %edx, %edx
    div %rcx
    add $'0', %dl
    dec %rsi
    movb %dl, (%rsi)
    test %rax, %rax
    jne itoa_loop
    lea 255(%r13), %rdx
    sub %rsi, %rdx
    movb $'s', (%rsi,%rdx)
    inc %rdx
    movb $13, (%rsi,%rdx)
    inc %rdx
    movb $10, (%rsi,%rdx)
    inc %rdx
    mov $1, %eax
    mov $1, %edi
    syscall
    jmp prompt
command_help:
    lea help_text(%rip), %rsi
    mov $(help_text_end-help_text), %edx
    call write_buf
    jmp prompt
command_unknown:
    lea unknown_text(%rip), %rsi
    mov $(unknown_text_end-unknown_text), %edx
    call write_buf
    jmp prompt
command_exit:
    mov $3, %eax
    xor %edi, %edi
    syscall
    ud2
.type write_buf,@function
write_buf:
    mov $1, %eax
    mov $1, %edi
    syscall
    ret
prompt_text: .ascii "> "
newline_text: .ascii "\r\n"
erase_text: .ascii "\b \b"
help_text: .ascii "Commands: help, echo <text>, uptime, cat <path>, ls [path], exit\r\n"
help_text_end:
unknown_text: .ascii "Unknown command. Type help.\r\n"
unknown_text_end:
cat_not_found_text: .ascii "cat: not found\r\n"
cat_not_found_end:
cat_no_filesystem_text: .ascii "cat: no filesystem\r\n"
cat_no_filesystem_end:
cat_usage_text: .ascii "cat: usage\r\n"
cat_usage_end:
root_path: .asciz "/"
ls_no_filesystem_text: .ascii "ls: no filesystem\r\n"
ls_usage_text: .ascii "ls: usage\r\n"
ls_error_text: .ascii "ls: error\r\n"
.size _start, .-_start
