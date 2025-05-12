use nix::unistd::Pid;

#[repr(C, align(8))]
#[derive(Debug, Default, Copy, Clone)]
pub struct Clone3Args {
    pub flags: u64, /* Flags bit mask. See libc::CLONE_* constants */
    pub pidfd: u64, /* Where to store PID file descriptor (int *) */
    pub child_tid: u64, /* Where to store child TID, in child's memory (pid_t *) */
    pub parent_tid: u64, /* Where to store child TID, in parent's memory (pid_t *) */
    pub exit_signal: u64, /* Signal to deliver to parent on child termination */
    pub stack: u64,      /* Pointer to lowest byte of stack */
    pub stack_size: u64, /* Size of stack */
    pub tls: u64,        /* Location of new TLS */
    pub set_tid: u64,    /* Pointer to a pid_t array (since Linux 5.5) */
    pub set_tid_size: u64, /* Number of elements in set_tid (since Linux 5.5) */
    pub cgroup: u64, /* File descriptor for target cgroup of child (since Linux 5.7) */
}

#[allow(dead_code)]
pub const CLONE_VM: u64 = libc::CLONE_VM as u64; /* Set if VM shared between processes */
#[allow(dead_code)]
pub const CLONE_FS: u64 = libc::CLONE_FS as u64; /* Set if fs info shared between processes */
#[allow(dead_code)]
pub const CLONE_FILES: u64 = libc::CLONE_FILES as u64; /* Set if open files shared between processes */
#[allow(dead_code)]
pub const CLONE_SIGHAND: u64 = libc::CLONE_SIGHAND as u64; /* Set if signal handlers shared */
#[allow(dead_code)]
pub const CLONE_PIDFD: u64 = libc::CLONE_PIDFD as u64; /* Set if a pidfd should be placed in parent */
#[allow(dead_code)]
pub const CLONE_PTRACE: u64 = libc::CLONE_PTRACE as u64; /* Set if we want to let tracing continue on the child too */
#[allow(dead_code)]
pub const CLONE_VFORK: u64 = libc::CLONE_VFORK as u64; /* Set if the parent wants the child to wake it up on mm_release */
#[allow(dead_code)]
pub const CLONE_PARENT: u64 = libc::CLONE_PARENT as u64; /* Set if we want to have the same parent as the cloner */
#[allow(dead_code)]
pub const CLONE_THREAD: u64 = libc::CLONE_THREAD as u64; /* Same thread group? */
#[allow(dead_code)]
pub const CLONE_NEWNS: u64 = libc::CLONE_NEWNS as u64; /* New mount namespace group */
#[allow(dead_code)]
pub const CLONE_SYSVSEM: u64 = libc::CLONE_SYSVSEM as u64; /* Share system V SEM_UNDO semantics */
#[allow(dead_code)]
pub const CLONE_SETTLS: u64 = libc::CLONE_SETTLS as u64; /* Create a new TLS for the child */
#[allow(dead_code)]
pub const CLONE_PARENT_SETTID: u64 = libc::CLONE_PARENT_SETTID as u64; /* Set TID in parent */
#[allow(dead_code)]
pub const CLONE_CHILD_CLEARTID: u64 = libc::CLONE_CHILD_CLEARTID as u64; /* Clear TID in child */
#[allow(dead_code)]
pub const CLONE_DETACHED: u64 = libc::CLONE_DETACHED as u64; /* Unused, ignored */
#[allow(dead_code)]
pub const CLONE_UNTRACED: u64 = libc::CLONE_UNTRACED as u64; /* Set if the tracing process can't force CLONE_PTRACE on this clone */
#[allow(dead_code)]
pub const CLONE_CHILD_SETTID: u64 = libc::CLONE_CHILD_SETTID as u64; /* Set TID in child */
#[allow(dead_code)]
pub const CLONE_NEWCGROUP: u64 = libc::CLONE_NEWCGROUP as u64; /* New cgroup namespace */
#[allow(dead_code)]
pub const CLONE_NEWUTS: u64 = libc::CLONE_NEWUTS as u64; /* New timesharing namespace */
#[allow(dead_code)]
pub const CLONE_NEWIPC: u64 = libc::CLONE_NEWIPC as u64; /* New ipc namespace */
#[allow(dead_code)]
pub const CLONE_NEWUSER: u64 = libc::CLONE_NEWUSER as u64; /* New user namespace */
#[allow(dead_code)]
pub const CLONE_NEWPID: u64 = libc::CLONE_NEWPID as u64; /* New pid namespace */
#[allow(dead_code)]
pub const CLONE_NEWNET: u64 = libc::CLONE_NEWNET as u64; /* New network namespace */
#[allow(dead_code)]
pub const CLONE_IO: u64 = libc::CLONE_IO as u64; /* Clone I/O context */

pub fn clone3(args: &Clone3Args) -> Result<Pid, std::io::Error> {
    let result = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            args,
            core::mem::size_of::<Clone3Args>(),
        )
    };
    if result < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(Pid::from_raw(result as i32))
    }
}
