# SYSTEMES syscall ABI

The x86_64 `SYSCALL` ABI uses `RAX` for the syscall number and return value.
Arguments are passed in `RDI`, `RSI`, `RDX`, `R10`, `R8`, and `R9`, in that
order. `SYSCALL` clobbers `RCX` and `R11`; user flags are sanitized before
return. Pointer arguments must refer to mapped user memory.

| Number | Name | Arguments | Result |
|---:|---|---|---|
| 0 | `SYS_GET_API_VERSION` | — | API version (`1`) |
| 1 | `SYS_WRITE` | `RDI`: fd (`1` serial), `RSI`: buffer, `RDX`: byte count | Bytes written |
| 2 | `SYS_READ` | `RDI`: fd (`1` serial), `RSI`: buffer, `RDX`: maximum bytes | Bytes read; stops at newline |
| 3 | `SYS_EXIT` | `RDI`: process status | Does not return to user mode; resumes the kernel continuation |
| 4 | `SYS_YIELD` | — | `0`; yields to the kernel scheduler when it has runnable tasks |

Errors are returned in `RAX` as negative errno values:

| Value | Name | Meaning |
|---:|---|---|
| `-5` | `EIO` | Serial input/output failure |
| `-9` | `EBADF` | Unsupported file descriptor |
| `-14` | `EFAULT` | Invalid, unmapped, or permission-incompatible user buffer |
| `-38` | `ENOSYS` | Unknown syscall number |

`SYS_EXIT` returns through the saved ring-0 continuation, not `SYSRET`. The
kernel restores its address space and releases the exiting process's user
pages and page-table frames before continuing to idle. `SYS_YIELD` is
cooperative and does not promise a switch when there are no other runnable
kernel tasks.
