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
| 5 | `SYS_UPTIME` | — | Unsigned 64-bit APIC timer ticks since boot, returned in `RAX` (100 ticks/second) |
| 6 | `SYS_OPEN` | `RDI`: user pointer to absolute UTF-8 path, `RSI`: path byte length (1–255) | File descriptor `>= 3`, or negative errno |
| 7 | `SYS_CLOSE` | `RDI`: file descriptor `>= 3` | `0`, or negative errno |

`SYS_UPTIME` reads the same monotonic Local APIC timer counter used by the scheduler; it does not read wall-clock time or dereference user memory. Convert to whole seconds with floor division (`ticks / 100`). The unsigned counter wraps modulo `2^64` after `u64::MAX`; callers comparing samples across wrap-around must use wrapping arithmetic.

`SYS_READ` keeps descriptor `1` as the blocking serial console for compatibility with the shell. Descriptors `>= 3` refer to files opened by `SYS_OPEN`; reads advance the per-open-file offset and return `0` at EOF. Descriptors `0` and `2` are currently invalid. `SYS_OPEN` copies the path into a bounded kernel buffer, requires valid UTF-8 and an absolute path, and currently accepts only a single root-directory component (for example `/hello.txt`). Relative paths, `.` and `..`, embedded NUL, and nested components are rejected with `EINVAL`; `RSI > 255` returns `ENAMETOOLONG`. The mounted SYSFS image is copied into a kernel-owned allocation and intentionally retained for the kernel lifetime, so the VFS remains available after the virtio-blk device setup returns. The image is capped at 64 KiB; a missing or invalid disk leaves the filesystem unavailable without failing boot. The active descriptor table belongs to the single Ring 3 process and all its handles are closed by `SYS_EXIT`.

Errors are returned in `RAX` as negative errno values:

| Value | Name | Meaning |
|---:|---|---|
| `-5` | `EIO` | Serial input/output failure |
| `-9` | `EBADF` | Unsupported file descriptor |
| `-14` | `EFAULT` | Invalid, unmapped, or permission-incompatible user buffer |
| `-19` | `ENODEV` | No mounted root filesystem |
| `-20` | `ENOTDIR` | Path resolves to a directory where a file is required |
| `-22` | `EINVAL` | Invalid path or syscall argument |
| `-24` | `EMFILE` | Process has reached the 16-open-file limit |
| `-36` | `ENAMETOOLONG` | Path exceeds the supported length |
| `-38` | `ENOSYS` | Unknown syscall number |

`SYS_EXIT` returns through the saved ring-0 continuation, not `SYSRET`. The
kernel restores its address space and releases the exiting process's user
pages and page-table frames before continuing to idle. `SYS_YIELD` is
cooperative and does not promise a switch when there are no other runnable
kernel tasks. `SYS_UPTIME` is monotonic modulo its unsigned 64-bit wrap-around. `SYS_EXIT` closes all descriptors owned by the exiting process before resuming the kernel continuation.
