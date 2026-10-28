use core::ffi::c_void;
use core::ops::{Index, IndexMut};
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch::{self, Context};
use crate::deferred_slots::DeferredSlots;
use crate::futex::{self, FutexError, FutexWait};
use crate::lifecycle::{ThreadCompletion, ThreadLifecycle};
use crate::thread::{self, ThreadError, ThreadName};
use crate::tls::{
    TLS_DESTRUCTOR_ITERATIONS, TLS_KEY_CAPACITY, TlsDestructor, TlsError, TlsKey, TlsRegistry,
    TlsValues,
};

pub type LwpThread = u32;
pub type QueueHandle = u32;
pub type MutexHandle = u32;
pub type CondHandle = u32;
pub type SemaphoreHandle = u32;
pub type MessageQueueHandle = u32;
pub type Message = *mut c_void;
pub type ThreadEntry = Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>;
pub type DeferredCallback = Option<unsafe fn(*mut c_void)>;
pub type TimerCallback = DeferredCallback;
pub type KeyedTimerCallback = Option<unsafe extern "C" fn(u32, *mut c_void)>;
pub type DebugHook = Option<unsafe fn(DebugEvent)>;
pub type WorkspaceAlloc = Option<unsafe fn(usize) -> *mut u8>;
pub type WorkspaceFree = Option<unsafe fn(*mut u8, usize)>;
pub type ThreadCleanup = unsafe extern "C" fn();

pub const LWP_CLOSED: i32 = -1;
pub const LWP_SUCCESSFUL: i32 = 0;
pub const LWP_ALREADY_SUSPENDED: i32 = 1;
pub const LWP_NOT_SUSPENDED: i32 = 2;
pub const LWP_PRIO_IDLE: u32 = 0;
pub const LWP_PRIO_HIGHEST: u32 = 127;
pub const LWP_THREAD_NULL: LwpThread = u32::MAX;
pub const LWP_TQUEUE_NULL: QueueHandle = u32::MAX;
pub const LWP_MUTEX_NULL: MutexHandle = u32::MAX;
pub const LWP_COND_NULL: CondHandle = u32::MAX;
pub const LWP_SEM_NULL: SemaphoreHandle = u32::MAX;
pub const MQ_BOX_NULL: MessageQueueHandle = u32::MAX;
pub const MQ_ERROR_SUCCESSFUL: i32 = 0;
pub const MQ_ERROR_TOOMANY: i32 = -5;
pub const MQ_MSG_BLOCK: u32 = 0;
pub const MQ_MSG_NOBLOCK: u32 = 1;
pub const TICKS_PER_SECOND: u64 = 60_750_000;

const THREAD_PAGE_SLOTS: usize = 32;
const MAX_QUEUES: usize = 64;
const MAX_MUTEXES: usize = 64;
const MAX_CONDS: usize = 64;
const MAX_SEMAPHORES: usize = 64;
const MAX_DEFERRED: usize = 64;
const MAX_INTERRUPT_DEFERRED: usize = 32;
const MAX_DEBUG_EVENTS: usize = 256;
const FALLBACK_STACK_SIZE: usize = 32 * 1024;
// DevkitPPC newlib's PowerPC `struct _reent` layout. Keep these values in
// sync with the target `sys/reent.h` ABI; newlib accesses this storage through
// `__getreent`, so a short errno-only struct is not sufficient.
const REENT_SIZE: usize = 512;
const REENT_STREAMS_OFFSET: usize = 4;
const REENT_RAND_NEXT_OFFSET: usize = 0x98;
const REENT_RAND48_SEED_OFFSET: usize = 0xa0;
const REENT_FILE_SIZE: usize = 112;
const MAIN_THREAD: usize = 0;
const IDLE_THREAD: usize = 1;
const DEFERRED_THREAD: usize = 2;
const NO_THREAD: usize = usize::MAX;
const NO_WAITER: usize = usize::MAX;
const NO_DEADLINE: u64 = u64::MAX;
const ETIMEDOUT: i32 = -116;
const STACK_GUARD: u32 = 0x4c57_5047;

const EVENT_INIT: u16 = 1;
const EVENT_CREATE: u16 = 2;
const EVENT_SWITCH: u16 = 3;
const EVENT_BLOCK: u16 = 4;
const EVENT_WAKE: u16 = 5;
const EVENT_TIMEOUT: u16 = 6;
const EVENT_EXIT: u16 = 7;
const EVENT_DEFERRED: u16 = 8;
const EVENT_IPC_WAKE: u16 = 9;
const EVENT_STACK_GUARD: u16 = 10;
const EVENT_TLS_FAILURE: u16 = 11;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum WaitResult {
    Signaled = 0,
    TimedOut = ETIMEDOUT,
    Cancelled = LWP_CLOSED,
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct DebugCounters {
    pub context_switches: u64,
    pub threads_created: u64,
    pub threads_exited: u64,
    pub blocking_waits: u64,
    pub wakeups: u64,
    pub timeouts: u64,
    pub deferred_callbacks: u64,
    pub ipc_wakeups: u64,
    pub stack_guard_failures: u64,
    pub dropped_debug_events: u64,
    pub dropped_deferred_callbacks: u64,
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct DebugEvent {
    pub tick: u64,
    pub event: u16,
    pub cpu_context: u16,
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum ThreadState {
    Unused,
    Reserved,
    Ready,
    Running,
    Blocked,
    Suspended,
    Finished,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum WaitKind {
    None,
    Join,
    Queue,
    Mutex,
    Cond,
    Semaphore,
    MessageSend,
    MessageReceive,
    Sleep,
    External,
    Deferred,
    Futex,
}

#[derive(Copy, Clone)]
struct WaitQueue {
    head: usize,
}
impl WaitQueue {
    const fn empty() -> Self {
        Self { head: NO_WAITER }
    }

    fn is_empty(self) -> bool {
        self.head == NO_WAITER
    }
}

struct Thread {
    context: Context,
    context_mirror: *mut u8,
    entry: ThreadEntry,
    arg: *mut c_void,
    result: *mut c_void,
    state: ThreadState,
    wait_kind: WaitKind,
    base_priority: u8,
    effective_priority: u8,
    suspend_count: u16,
    wait_object: usize,
    wait_token: u32,
    wait_order: u64,
    wait_result: WaitResult,
    deadline: u64,
    join_waiters: WaitQueue,
    wait_next: usize,
    stack_guard: *mut u32,
    guard_reported: bool,
    pending_wake: bool,
    pending_token: u32,
    owned_stack: *mut u8,
    owned_stack_bytes: usize,
    stack_bytes: usize,
    reent: ReentStorage,
    external_reent: *mut u8,
    lifecycle: ThreadLifecycle,
    tls: TlsValues,
    tls_destructing: bool,
    cleanup_failed: bool,
    completion: ThreadCompletion,
    runtime_cleanup: Option<ThreadCleanup>,
    runtime_cleanup_ran: bool,
    cleanup_running: bool,
    tls_cleanup_result: Option<Result<(), TlsError>>,
    name: ThreadName,
}

impl Thread {
    const fn empty() -> Self {
        Self {
            context: Context::zeroed(),
            context_mirror: ptr::null_mut(),
            entry: None,
            arg: ptr::null_mut(),
            result: ptr::null_mut(),
            state: ThreadState::Unused,
            wait_kind: WaitKind::None,
            base_priority: 0,
            effective_priority: 0,
            suspend_count: 0,
            wait_object: 0,
            wait_token: 0,
            wait_order: 0,
            wait_result: WaitResult::Signaled,
            deadline: NO_DEADLINE,
            join_waiters: WaitQueue::empty(),
            wait_next: NO_WAITER,
            stack_guard: ptr::null_mut(),
            guard_reported: false,
            pending_wake: false,
            pending_token: 0,
            owned_stack: ptr::null_mut(),
            owned_stack_bytes: 0,
            stack_bytes: 0,
            reent: ReentStorage::empty(),
            external_reent: ptr::null_mut(),
            lifecycle: ThreadLifecycle::new(),
            tls: TlsValues::new(),
            tls_destructing: false,
            cleanup_failed: false,
            completion: ThreadCompletion::new(),
            runtime_cleanup: None,
            runtime_cleanup_ran: false,
            cleanup_running: false,
            tls_cleanup_result: None,
            name: ThreadName::new(),
        }
    }
}

struct ThreadPage {
    slots: [Thread; THREAD_PAGE_SLOTS],
    next: *mut ThreadPage,
}

impl ThreadPage {
    const fn empty() -> Self {
        Self {
            slots: [const { Thread::empty() }; THREAD_PAGE_SLOTS],
            next: ptr::null_mut(),
        }
    }
}

struct ThreadTable {
    base: [Thread; THREAD_PAGE_SLOTS],
    pages: *mut ThreadPage,
    capacity: usize,
}

impl ThreadTable {
    const fn new() -> Self {
        Self {
            base: [const { Thread::empty() }; THREAD_PAGE_SLOTS],
            pages: ptr::null_mut(),
            capacity: THREAD_PAGE_SLOTS,
        }
    }

    unsafe fn slot_ptr(&self, index: usize) -> *mut Thread {
        if index < THREAD_PAGE_SLOTS {
            return unsafe { self.base.as_ptr().add(index).cast_mut() };
        }
        let mut page = self.pages;
        let mut remaining = index - THREAD_PAGE_SLOTS;
        while !page.is_null() {
            if remaining < THREAD_PAGE_SLOTS {
                return unsafe { (*page).slots.as_ptr().add(remaining).cast_mut() };
            }
            remaining -= THREAD_PAGE_SLOTS;
            page = unsafe { (*page).next };
        }
        ptr::null_mut()
    }

    unsafe fn append_page(&mut self, page: *mut ThreadPage) -> usize {
        let start = self.capacity;
        if self.pages.is_null() {
            self.pages = page;
        } else {
            let mut tail = self.pages;
            while unsafe { !(*tail).next.is_null() } {
                tail = unsafe { (*tail).next };
            }
            unsafe { (*tail).next = page };
        }
        self.capacity = self.capacity.saturating_add(THREAD_PAGE_SLOTS);
        start
    }
}

impl Index<usize> for ThreadTable {
    type Output = Thread;

    fn index(&self, index: usize) -> &Self::Output {
        let slot = unsafe { self.slot_ptr(index) };
        assert!(!slot.is_null(), "thread index out of range");
        unsafe { &*slot }
    }
}

impl IndexMut<usize> for ThreadTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let slot = unsafe { self.slot_ptr(index) };
        assert!(!slot.is_null(), "thread index out of range");
        unsafe { &mut *slot }
    }
}

trait ThreadAccess {
    fn len(&self) -> usize;
    fn thread(&self, index: usize) -> &Thread;
    fn thread_mut(&mut self, index: usize) -> &mut Thread;
}

impl ThreadAccess for ThreadTable {
    fn len(&self) -> usize {
        self.capacity
    }

    fn thread(&self, index: usize) -> &Thread {
        &self[index]
    }

    fn thread_mut(&mut self, index: usize) -> &mut Thread {
        &mut self[index]
    }
}

impl ThreadAccess for [Thread] {
    fn len(&self) -> usize {
        <[Thread]>::len(self)
    }

    fn thread(&self, index: usize) -> &Thread {
        &self[index]
    }

    fn thread_mut(&mut self, index: usize) -> &mut Thread {
        &mut self[index]
    }
}

impl<const N: usize> ThreadAccess for [Thread; N] {
    fn len(&self) -> usize {
        N
    }

    fn thread(&self, index: usize) -> &Thread {
        &self[index]
    }

    fn thread_mut(&mut self, index: usize) -> &mut Thread {
        &mut self[index]
    }
}

#[repr(C, align(8))]
#[derive(Copy, Clone)]
struct ReentStorage([u8; REENT_SIZE]);

impl ReentStorage {
    const fn empty() -> Self {
        Self([0; REENT_SIZE])
    }
}

const _: () = {
    assert!(core::mem::size_of::<ReentStorage>() == 512);
    assert!(core::mem::align_of::<ReentStorage>() >= 8);
    assert!(core::mem::size_of::<NewlibFiles>() == REENT_FILE_SIZE * 3);
    assert!(core::mem::align_of::<NewlibFiles>() >= 8);
    assert!(REENT_STREAMS_OFFSET + 3 * core::mem::size_of::<usize>() <= REENT_SIZE);
    assert!(REENT_RAND48_SEED_OFFSET + 7 * core::mem::size_of::<u16>() <= REENT_SIZE);
};

#[repr(C, align(8))]
struct NewlibFiles([u8; REENT_FILE_SIZE * 3]);

#[unsafe(no_mangle)]
static mut __sf: NewlibFiles = NewlibFiles([0; REENT_FILE_SIZE * 3]);

#[unsafe(no_mangle)]
static mut _impure_data: ReentStorage = ReentStorage::empty();

#[unsafe(no_mangle)]
pub static mut _impure_ptr: *mut c_void = ptr::addr_of_mut!(_impure_data).cast();

static mut FALLBACK_REENT_INITIALIZED: bool = false;

unsafe fn initialize_reent(storage: &mut ReentStorage) {
    unsafe { initialize_reent_with_streams(storage, ptr::addr_of_mut!(__sf).cast()) };
}

unsafe fn initialize_reent_with_streams(storage: &mut ReentStorage, streams: *mut u8) {
    storage.0.fill(0);
    let base = storage.0.as_mut_ptr();
    for stream in 0..3 {
        let stream_ptr = unsafe { streams.add(stream * REENT_FILE_SIZE) };
        unsafe {
            ptr::write_unaligned(
                base.add(REENT_STREAMS_OFFSET + stream * core::mem::size_of::<usize>())
                    .cast::<usize>(),
                stream_ptr as usize,
            );
        }
    }
    unsafe {
        ptr::write_unaligned(base.add(REENT_RAND_NEXT_OFFSET).cast::<u64>(), 1);
        let seed = base.add(REENT_RAND48_SEED_OFFSET).cast::<u16>();
        seed.write_unaligned(0x330e);
        seed.add(1).write_unaligned(0xabcd);
        seed.add(2).write_unaligned(0x1234);
        seed.add(3).write_unaligned(0xe66d);
        seed.add(4).write_unaligned(0xdeec);
        seed.add(5).write_unaligned(0x0005);
        seed.add(6).write_unaligned(0x000b);
    }
}

#[repr(C, align(32))]
struct Stack([u8; FALLBACK_STACK_SIZE]);

#[derive(Copy, Clone)]
struct Queue {
    used: bool,
    waiters: WaitQueue,
}
impl Queue {
    const fn empty() -> Self {
        Self {
            used: false,
            waiters: WaitQueue::empty(),
        }
    }
}

#[derive(Copy, Clone)]
struct MutexState {
    used: bool,
    recursive: bool,
    owner: usize,
    recursion: u32,
    waiters: WaitQueue,
}
impl MutexState {
    const fn empty() -> Self {
        Self {
            used: false,
            recursive: false,
            owner: NO_THREAD,
            recursion: 0,
            waiters: WaitQueue::empty(),
        }
    }
}

#[derive(Copy, Clone)]
struct CondState {
    used: bool,
    waiters: WaitQueue,
}
impl CondState {
    const fn empty() -> Self {
        Self {
            used: false,
            waiters: WaitQueue::empty(),
        }
    }
}

#[derive(Copy, Clone)]
struct SemaphoreState {
    used: bool,
    value: i32,
    waiters: WaitQueue,
}
impl SemaphoreState {
    const fn empty() -> Self {
        Self {
            used: false,
            value: 0,
            waiters: WaitQueue::empty(),
        }
    }
}

struct MessageQueueState {
    used: bool,
    capacity: usize,
    head: usize,
    len: usize,
    messages: *mut Message,
    send_waiters: WaitQueue,
    receive_waiters: WaitQueue,
    next: *mut MessageQueueState,
}
impl MessageQueueState {
    #[cfg(test)]
    const fn empty() -> Self {
        Self {
            used: false,
            capacity: 0,
            head: 0,
            len: 0,
            messages: ptr::null_mut(),
            send_waiters: WaitQueue::empty(),
            receive_waiters: WaitQueue::empty(),
            next: ptr::null_mut(),
        }
    }
}

#[derive(Copy, Clone)]
struct TimerState {
    key: usize,
    order: u64,
    callback: TimerCallback,
    keyed_callback: KeyedTimerCallback,
    arg: *mut c_void,
    deadline: u64,
    period: u64,
}
struct TimerNode {
    timer: TimerState,
    next: *mut TimerNode,
}

#[derive(Copy, Clone)]
struct DeferredItem {
    callback: DeferredCallback,
    arg: *mut c_void,
}
impl DeferredItem {
    const fn empty() -> Self {
        Self {
            callback: None,
            arg: ptr::null_mut(),
        }
    }
}

static mut INITIALIZED: bool = false;
static mut CURRENT: usize = MAIN_THREAD;
static mut NEXT_WAIT_ORDER: u64 = 1;
static mut NEXT_TIMER_ORDER: u64 = 1;
static mut LAST_SELECTED_BY_PRIORITY: [usize; 128] = [THREAD_PAGE_SLOTS - 1; 128];
static mut THREADS: ThreadTable = ThreadTable::new();
static mut STACKS: [Stack; THREAD_PAGE_SLOTS] =
    [const { Stack([0; FALLBACK_STACK_SIZE]) }; THREAD_PAGE_SLOTS];
static mut QUEUES: [Queue; MAX_QUEUES] = [Queue::empty(); MAX_QUEUES];
static mut MUTEXES: [MutexState; MAX_MUTEXES] = [MutexState::empty(); MAX_MUTEXES];
static mut CONDS: [CondState; MAX_CONDS] = [CondState::empty(); MAX_CONDS];
static mut SEMAPHORES: [SemaphoreState; MAX_SEMAPHORES] = [SemaphoreState::empty(); MAX_SEMAPHORES];
static mut MESSAGE_QUEUE_HEAD: *mut MessageQueueState = ptr::null_mut();
static mut TIMER_HEAD: *mut TimerNode = ptr::null_mut();
static mut TIMER_FREE_HEAD: *mut TimerNode = ptr::null_mut();
static mut WORKSPACE_ALLOC: WorkspaceAlloc = None;
static mut WORKSPACE_FREE: WorkspaceFree = None;
static mut DEFERRED: [DeferredItem; MAX_DEFERRED] = [DeferredItem::empty(); MAX_DEFERRED];
static mut DEFERRED_HEAD: usize = 0;
static mut DEFERRED_LEN: usize = 0;
static mut INTERRUPT_DEFERRED: [DeferredItem; MAX_INTERRUPT_DEFERRED] =
    [DeferredItem::empty(); MAX_INTERRUPT_DEFERRED];
static mut INTERRUPT_DEFERRED_HEAD: usize = 0;
static mut INTERRUPT_DEFERRED_SLOTS: DeferredSlots<MAX_INTERRUPT_DEFERRED> = DeferredSlots::new();
static mut DEBUG_EVENTS: [DebugEvent; MAX_DEBUG_EVENTS] = [DebugEvent {
    tick: 0,
    event: 0,
    cpu_context: 0,
    a: 0,
    b: 0,
    c: 0,
}; MAX_DEBUG_EVENTS];
// Callers must disable interrupts before they access this shared queue state.
unsafe fn interrupt_deferred_slots() -> &'static mut DeferredSlots<MAX_INTERRUPT_DEFERRED> {
    unsafe { &mut *core::ptr::addr_of_mut!(INTERRUPT_DEFERRED_SLOTS) }
}
static mut DEBUG_HEAD: usize = 0;
static mut DEBUG_LEN: usize = 0;
static mut DEBUG_COUNTERS: DebugCounters = DebugCounters {
    context_switches: 0,
    threads_created: 0,
    threads_exited: 0,
    blocking_waits: 0,
    wakeups: 0,
    timeouts: 0,
    deferred_callbacks: 0,
    ipc_wakeups: 0,
    stack_guard_failures: 0,
    dropped_debug_events: 0,
    dropped_deferred_callbacks: 0,
};
static mut DEBUG_HOOK: DebugHook = None;
static mut TLS_REGISTRY: TlsRegistry = TlsRegistry::new();

/// Completed cleanup operations and observable TLS cleanup failures.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ThreadCleanupCounters {
    pub detached_reaped: u32,
    pub tls_failures: u32,
}

static mut THREAD_CLEANUP_COUNTERS: ThreadCleanupCounters = ThreadCleanupCounters {
    detached_reaped: 0,
    tls_failures: 0,
};

#[inline]
fn thread_capacity() -> usize {
    unsafe { (*ptr::addr_of!(THREADS)).capacity }
}

#[inline]
fn thread_ptr(index: usize) -> *mut Thread {
    unsafe { (*ptr::addr_of!(THREADS)).slot_ptr(index) }
}

#[inline]
fn synchronization_handle(index: usize) -> u32 {
    index as u32 + 1
}

#[inline]
fn thread_handle(index: usize) -> u32 {
    !(thread_ptr(index) as usize as u32)
}

#[inline]
fn decode_priority(priority: u32) -> u8 {
    (priority & LWP_PRIO_HIGHEST) as u8
}
#[inline]
fn decode(handle: u32, max: usize, null: u32) -> Option<usize> {
    let index = handle.wrapping_sub(1) as usize;
    (handle != 0 && handle != null && index < max).then_some(index)
}
#[inline]
fn thread_index(handle_value: LwpThread) -> Option<usize> {
    if handle_value == 0 || handle_value == LWP_THREAD_NULL {
        return None;
    }
    let wanted = (!(handle_value) as usize) as *mut Thread;
    let capacity = thread_capacity();
    for index in 0..capacity {
        let slot = thread_ptr(index);
        if slot == wanted && unsafe { (*slot).state != ThreadState::Unused } {
            return Some(index);
        }
    }
    None
}

/// Installs the allocator used for dynamically sized LWP compatibility state.
///
/// This must be called before creating dynamic mailboxes, timers, or threads.
/// The system layer registers its dedicated LWP workspace allocator during
/// startup, mirroring libogc's `__lwp_wkspace_allocate` ownership model.
pub fn set_workspace_allocator(alloc: WorkspaceAlloc, free: WorkspaceFree) {
    let irq = arch::disable_interrupts();
    unsafe {
        WORKSPACE_ALLOC = alloc;
        WORKSPACE_FREE = free;
    }
    arch::restore_interrupts(irq);
}

unsafe fn workspace_alloc(size: usize) -> *mut u8 {
    unsafe {
        if size == 0 {
            return ptr::null_mut();
        }
        match WORKSPACE_ALLOC {
            Some(alloc) => alloc(size),
            None => ptr::null_mut(),
        }
    }
}

unsafe fn workspace_free(allocation: *mut u8, size: usize) {
    unsafe {
        if allocation.is_null() || size == 0 {
            return;
        }
        if let Some(free) = WORKSPACE_FREE {
            free(allocation, size);
        }
    }
}

/// Allocates dynamic compatibility storage from the shared LWP workspace.
///
/// # Safety
/// The returned allocation must be released with [`release_workspace`] using
/// the same `size`.
pub unsafe fn allocate_workspace(size: usize) -> *mut u8 {
    unsafe { workspace_alloc(size) }
}

/// Releases storage obtained from [`allocate_workspace`].
///
/// # Safety
/// `allocation` must have been returned by [`allocate_workspace`] for the same
/// `size` and must not already have been released.
pub unsafe fn release_workspace(allocation: *mut u8, size: usize) {
    unsafe { workspace_free(allocation, size) };
}

unsafe fn allocate_thread_page() -> *mut ThreadPage {
    let page = unsafe { workspace_alloc(core::mem::size_of::<ThreadPage>()) }.cast::<ThreadPage>();
    if !page.is_null() {
        unsafe { page.write(ThreadPage::empty()) };
    }
    page
}

unsafe fn free_thread_page(page: *mut ThreadPage) {
    unsafe { workspace_free(page.cast(), core::mem::size_of::<ThreadPage>()) };
}

unsafe fn reserve_thread_slot() -> Option<usize> {
    reap_detached_threads();
    let irq = arch::disable_interrupts();
    let capacity = thread_capacity();
    for index in 3..capacity {
        if unsafe { THREADS[index].state == ThreadState::Unused } {
            unsafe { THREADS[index].state = ThreadState::Reserved };
            arch::restore_interrupts(irq);
            return Some(index);
        }
    }
    arch::restore_interrupts(irq);

    let page = unsafe { allocate_thread_page() };
    if page.is_null() {
        return None;
    }

    let irq = arch::disable_interrupts();
    let capacity = thread_capacity();
    if let Some(index) =
        (3..capacity).find(|&index| unsafe { THREADS[index].state == ThreadState::Unused })
    {
        unsafe { THREADS[index].state = ThreadState::Reserved };
        arch::restore_interrupts(irq);
        unsafe { free_thread_page(page) };
        return Some(index);
    }

    let index = unsafe { (*ptr::addr_of_mut!(THREADS)).append_page(page) };
    unsafe { THREADS[index].state = ThreadState::Reserved };
    arch::restore_interrupts(irq);
    Some(index)
}

unsafe fn release_reserved_thread_slot(index: usize) {
    let irq = arch::disable_interrupts();
    if index < thread_capacity() && unsafe { THREADS[index].state == ThreadState::Reserved } {
        unsafe { THREADS[index] = Thread::empty() };
    }
    arch::restore_interrupts(irq);
}

unsafe fn allocate_owned_stack(size: usize) -> Option<(*mut u8, usize, *mut u8)> {
    let allocation_size = size.checked_add(31)?;
    let allocation = unsafe { workspace_alloc(allocation_size) };
    if allocation.is_null() {
        return None;
    }
    let aligned = ((allocation as usize + 31) & !31usize) as *mut u8;
    Some((allocation, allocation_size, aligned))
}

#[inline]
fn encode_message_queue_handle(state: *mut MessageQueueState) -> MessageQueueHandle {
    !(state as usize as u32)
}

unsafe fn message_queue_state(mq: MessageQueueHandle) -> Option<*mut MessageQueueState> {
    unsafe {
        if mq == 0 || mq == MQ_BOX_NULL {
            return None;
        }
        let wanted = (!(mq) as usize) as *mut MessageQueueState;
        let mut current = MESSAGE_QUEUE_HEAD;
        while !current.is_null() {
            if current == wanted && { (*current).used } {
                return Some(current);
            }
            current = (*current).next;
        }
        None
    }
}

unsafe fn recycle_timer_node(node: *mut TimerNode) {
    if node.is_null() {
        return;
    }
    unsafe {
        (*node).next = TIMER_FREE_HEAD;
        TIMER_FREE_HEAD = node;
    }
}

unsafe fn debug_locked(event: u16, a: u32, b: u32, c: u32) {
    unsafe {
        let item = { record_debug_locked(event, a, b, c) };
        if let Some(hook) = DEBUG_HOOK {
            hook(item);
        }
    }
}

unsafe fn record_debug_locked(event: u16, a: u32, b: u32, c: u32) -> DebugEvent {
    unsafe {
        let item = DebugEvent {
            tick: arch::read_timebase(),
            event,
            cpu_context: CURRENT as u16,
            a,
            b,
            c,
        };
        if DEBUG_LEN == MAX_DEBUG_EVENTS {
            DEBUG_HEAD = (DEBUG_HEAD + 1) % MAX_DEBUG_EVENTS;
            DEBUG_LEN -= 1;
            DEBUG_COUNTERS.dropped_debug_events =
                DEBUG_COUNTERS.dropped_debug_events.saturating_add(1);
        }
        let tail = (DEBUG_HEAD + DEBUG_LEN) % MAX_DEBUG_EVENTS;
        DEBUG_EVENTS[tail] = item;
        DEBUG_LEN += 1;
        item
    }
}

unsafe fn install_stack_guard(index: usize, stack_base: *mut u8) {
    unsafe {
        let guard = stack_base.cast::<u32>();
        ptr::write_volatile(guard, STACK_GUARD);
        THREADS[index].stack_guard = guard;
        THREADS[index].guard_reported = false;
    }
}

unsafe fn validate_stack_guard_locked(index: usize) {
    unsafe { validate_stack_guard(index, true) };
}

unsafe fn validate_stack_guard_from_interrupt_locked(index: usize) {
    unsafe { validate_stack_guard(index, false) };
}

unsafe fn validate_stack_guard(index: usize, invoke_hook: bool) {
    unsafe {
        let thread = &mut THREADS[index];
        if thread.stack_guard.is_null() || thread.guard_reported {
            return;
        }
        if ptr::read_volatile(thread.stack_guard) != STACK_GUARD {
            thread.guard_reported = true;
            DEBUG_COUNTERS.stack_guard_failures =
                DEBUG_COUNTERS.stack_guard_failures.saturating_add(1);
            let event = {
                record_debug_locked(
                    EVENT_STACK_GUARD,
                    thread_handle(index),
                    thread.stack_guard as usize as u32,
                    0,
                )
            };
            if invoke_hook && let Some(hook) = DEBUG_HOOK {
                hook(event);
            }
        }
    }
}

unsafe fn setup_context(index: usize, stack_top: *mut c_void) {
    unsafe {
        let mut sp = (stack_top as usize) & !7usize;
        sp = sp.saturating_sub(8);
        // C's fresh-thread frame uses UINT32_MAX as the terminal back-chain.
        ptr::write(sp as *mut u32, u32::MAX);
        let mut context = Context::zeroed();
        context.pc = thread_bootstrap as *const () as usize as u32;
        context.msr = arch::current_msr() | 0x0000_b032;
        context.lr = 0;
        context.gpr[1] = sp as u32;
        context.gpr[2] = arch::current_r2();
        context.gpr[13] = arch::current_r13();
        THREADS[index].context = context;
    }
}

unsafe fn initialize_locked() {
    unsafe {
        if INITIALIZED {
            return;
        }
        {
            initialize_reent(&mut *ptr::addr_of_mut!(_impure_data));
            FALLBACK_REENT_INITIALIZED = true;
        }
        THREADS[MAIN_THREAD] = Thread::empty();
        THREADS[MAIN_THREAD].state = ThreadState::Running;
        THREADS[MAIN_THREAD].base_priority = 64;
        THREADS[MAIN_THREAD].effective_priority = 64;
        initialize_reent(&mut THREADS[MAIN_THREAD].reent);

        THREADS[IDLE_THREAD] = Thread::empty();
        THREADS[IDLE_THREAD].state = ThreadState::Ready;
        THREADS[IDLE_THREAD].entry = Some(idle_entry);
        THREADS[IDLE_THREAD].base_priority = 0;
        THREADS[IDLE_THREAD].effective_priority = 0;
        initialize_reent(&mut THREADS[IDLE_THREAD].reent);
        let idle_base = STACKS[IDLE_THREAD].0.as_mut_ptr();
        install_stack_guard(IDLE_THREAD, idle_base);
        let idle_top = idle_base.add(FALLBACK_STACK_SIZE);
        setup_context(IDLE_THREAD, idle_top.cast());

        THREADS[DEFERRED_THREAD] = Thread::empty();
        THREADS[DEFERRED_THREAD].state = ThreadState::Blocked;
        THREADS[DEFERRED_THREAD].wait_kind = WaitKind::Deferred;
        THREADS[DEFERRED_THREAD].entry = Some(deferred_entry);
        THREADS[DEFERRED_THREAD].base_priority = 126;
        THREADS[DEFERRED_THREAD].effective_priority = 126;
        initialize_reent(&mut THREADS[DEFERRED_THREAD].reent);
        let deferred_base = STACKS[DEFERRED_THREAD].0.as_mut_ptr();
        install_stack_guard(DEFERRED_THREAD, deferred_base);
        let deferred_top = deferred_base.add(FALLBACK_STACK_SIZE);
        setup_context(DEFERRED_THREAD, deferred_top.cast());

        INITIALIZED = true;
        {
            _impure_ptr = ptr::addr_of_mut!(THREADS[MAIN_THREAD].reent.0).cast();
        }
        ogc_sys::set_decrementer_dispatch_callback(Some(decrementer_tick));
        ogc_sys::set_interrupt_return_callback(Some(interrupt_return));
        ogc_sys::install_decrementer_exception_vector();
        debug_locked(
            EVENT_INIT,
            thread_capacity() as u32,
            FALLBACK_STACK_SIZE as u32,
            0,
        );
    }
}

fn ensure_initialized() {
    let irq = arch::disable_interrupts();
    unsafe {
        initialize_locked();
    }
    arch::restore_interrupts(irq);
}

/// Initializes the static scheduler state before startup code uses alarms or
/// deferred callbacks. This is the Rust equivalent of the LWP runtime setup;
/// its queues and stacks live in Rust-owned static memory.
pub fn initialize() {
    ensure_initialized();
}

unsafe fn select_next_locked() -> usize {
    unsafe {
        let capacity = thread_capacity();
        let mut best_prio = 0u8;
        for index in 0..capacity {
            let thread = &THREADS[index];
            if matches!(thread.state, ThreadState::Ready | ThreadState::Running)
                && thread.effective_priority > best_prio
            {
                best_prio = thread.effective_priority;
            }
        }

        let start = (LAST_SELECTED_BY_PRIORITY[best_prio as usize] + 1) % capacity;
        let mut best = IDLE_THREAD;
        for offset in 0..capacity {
            let index = (start + offset) % capacity;
            let thread = &THREADS[index];
            if matches!(thread.state, ThreadState::Ready | ThreadState::Running)
                && thread.effective_priority == best_prio
            {
                best = index;
                break;
            }
        }
        LAST_SELECTED_BY_PRIORITY[best_prio as usize] = best;
        best
    }
}

unsafe fn rotate_priority_locked(priority: u8) {
    unsafe {
        let capacity = thread_capacity();
        let cursor = LAST_SELECTED_BY_PRIORITY[priority as usize] % capacity;
        for offset in 1..=capacity {
            let index = (cursor + offset) % capacity;
            let thread = &THREADS[index];
            if matches!(thread.state, ThreadState::Ready | ThreadState::Running)
                && thread.effective_priority == priority
            {
                LAST_SELECTED_BY_PRIORITY[priority as usize] = index;
                break;
            }
        }
    }
}

unsafe fn thread_reent_ptr(index: usize) -> *mut u8 {
    unsafe {
        let external = { THREADS[index].external_reent };
        if external.is_null() {
            core::ptr::addr_of_mut!(THREADS[index].reent).cast::<u8>()
        } else {
            external
        }
    }
}

unsafe fn copy_context_from_mirror(context: *mut Context, mirror: *const u8) {
    if mirror.is_null() {
        return;
    }
    unsafe {
        ptr::copy_nonoverlapping(
            mirror,
            context.cast::<u8>(),
            core::mem::size_of::<Context>(),
        )
    };
}

unsafe fn context_for_restore_locked(index: usize) -> *const Context {
    unsafe {
        let context = ptr::addr_of_mut!(THREADS[index].context);
        let mirror = THREADS[index].context_mirror;
        copy_context_from_mirror(context, mirror);
        context.cast_const()
    }
}

unsafe fn switch_locked(previous: usize, irq: u32) {
    unsafe {
        validate_stack_guard_locked(previous);
        let next = select_next_locked();
        if next == previous {
            THREADS[previous].state = ThreadState::Running;
            arch::restore_interrupts(irq);
            return;
        }
        THREADS[next].state = ThreadState::Running;
        CURRENT = next;
        _impure_ptr = thread_reent_ptr(next).cast();
        DEBUG_COUNTERS.context_switches = DEBUG_COUNTERS.context_switches.saturating_add(1);
        debug_locked(
            EVENT_SWITCH,
            thread_handle(previous),
            thread_handle(next),
            THREADS[next].effective_priority as u32,
        );
        let old = ptr::addr_of_mut!(THREADS[previous].context);
        let new = { context_for_restore_locked(next) };
        let mirror = THREADS[previous].context_mirror;
        arch::context_switch(old, new, mirror);
        arch::restore_interrupts(irq);
    }
}

unsafe fn wake_index_locked(index: usize, result: WaitResult, token: u32) {
    unsafe {
        let t = &mut THREADS[index];
        if !matches!(t.state, ThreadState::Blocked | ThreadState::Suspended) {
            return;
        }
        t.wait_result = result;
        t.wait_token = token;
        t.deadline = NO_DEADLINE;
        t.wait_kind = WaitKind::None;
        if t.suspend_count == 0 {
            t.state = ThreadState::Ready;
        } else {
            t.state = ThreadState::Suspended;
        }
        DEBUG_COUNTERS.wakeups = DEBUG_COUNTERS.wakeups.saturating_add(1);
        debug_locked(
            EVENT_WAKE,
            thread_handle(index),
            result as i32 as u32,
            token,
        );
        reschedule_decrementer_locked(arch::read_timebase());
    }
}

fn wait_queue_insert<T: ThreadAccess + ?Sized>(
    queue: &mut WaitQueue,
    threads: &mut T,
    index: usize,
) {
    threads.thread_mut(index).wait_next = queue.head;
    queue.head = index;
}

unsafe fn wait_queue_insert_locked(queue: *mut WaitQueue, index: usize) {
    let threads = unsafe { &mut *core::ptr::addr_of_mut!(THREADS) };
    unsafe { wait_queue_insert(&mut *queue, threads, index) };
}

fn wait_queue_remove<T: ThreadAccess + ?Sized>(
    queue: &mut WaitQueue,
    threads: &mut T,
    index: usize,
) -> bool {
    let mut previous = NO_WAITER;
    let mut current = queue.head;
    while current != NO_WAITER {
        let next = threads.thread(current).wait_next;
        if current == index {
            if previous == NO_WAITER {
                queue.head = next;
            } else {
                threads.thread_mut(previous).wait_next = next;
            }
            threads.thread_mut(current).wait_next = NO_WAITER;
            return true;
        }
        previous = current;
        current = next;
    }
    false
}

fn take_best_waiter_by<T: ThreadAccess + ?Sized>(
    queue: &mut WaitQueue,
    threads: &mut T,
) -> Option<usize> {
    let mut best = None;
    let mut current = queue.head;
    while current != NO_WAITER {
        let thread = threads.thread(current);
        best = futex::select(
            best,
            futex::Waiter {
                index: current,
                priority: thread.effective_priority,
                order: thread.wait_order,
            },
        );
        current = thread.wait_next;
    }
    let best = best?;
    assert!(
        wait_queue_remove(queue, threads, best.index),
        "selected LWP waiter is missing from its queue"
    );
    Some(best.index)
}

unsafe fn take_best_waiter_locked(waiters: &mut WaitQueue) -> Option<usize> {
    let threads = unsafe { &mut *core::ptr::addr_of_mut!(THREADS) };
    take_best_waiter_by(waiters, threads)
}

unsafe fn wake_one_locked(waiters: &mut WaitQueue, result: WaitResult, token: u32) {
    if let Some(index) = unsafe { take_best_waiter_locked(waiters) } {
        unsafe { wake_index_locked(index, result, token) };
    }
}

unsafe fn try_wake_one_locked(waiters: &mut WaitQueue, result: WaitResult, token: u32) -> bool {
    if let Some(index) = unsafe { take_best_waiter_locked(waiters) } {
        unsafe { wake_index_locked(index, result, token) };
        true
    } else {
        false
    }
}

unsafe fn wake_all_locked(waiters: &mut WaitQueue, result: WaitResult, token: u32) {
    while let Some(index) = unsafe { take_best_waiter_locked(waiters) } {
        unsafe { wake_index_locked(index, result, token) };
    }
}

fn handoff_mutex<T: ThreadAccess + ?Sized>(
    mutex: &mut MutexState,
    threads: &mut T,
) -> Option<usize> {
    let next_owner = take_best_waiter_by(&mut mutex.waiters, threads);
    if let Some(owner) = next_owner {
        mutex.owner = owner;
        mutex.recursion = 1;
    } else {
        mutex.owner = NO_THREAD;
        mutex.recursion = 0;
    }
    next_owner
}

unsafe fn block_current_locked(
    kind: WaitKind,
    object: usize,
    token: u32,
    deadline: u64,
    irq: u32,
) -> WaitResult {
    unsafe {
        let current = CURRENT;
        THREADS[current].state = ThreadState::Blocked;
        THREADS[current].wait_kind = kind;
        THREADS[current].wait_object = object;
        THREADS[current].wait_token = token;
        THREADS[current].wait_order = next_wait_order_locked();
        THREADS[current].wait_result = WaitResult::Signaled;
        THREADS[current].deadline = deadline;
        reschedule_decrementer_locked(arch::read_timebase());
        DEBUG_COUNTERS.blocking_waits = DEBUG_COUNTERS.blocking_waits.saturating_add(1);
        debug_locked(
            EVENT_BLOCK,
            thread_handle(current),
            kind as u32,
            object as u32,
        );
        switch_locked(current, irq);
        THREADS[current].wait_result
    }
}

fn next_wait_order(next_order: &mut u64) -> u64 {
    let order = *next_order;
    *next_order = next_order.wrapping_add(1);
    if *next_order == 0 {
        *next_order = 1;
    }
    order
}

unsafe fn next_wait_order_locked() -> u64 {
    let next_order = unsafe { &mut *core::ptr::addr_of_mut!(NEXT_WAIT_ORDER) };
    next_wait_order(next_order)
}

fn is_priority_waiter(thread: &Thread) -> bool {
    thread.state == ThreadState::Blocked
        && matches!(
            thread.wait_kind,
            WaitKind::Queue
                | WaitKind::Mutex
                | WaitKind::Cond
                | WaitKind::Semaphore
                | WaitKind::MessageSend
                | WaitKind::MessageReceive
        )
}

fn update_effective_priority(thread: &mut Thread, priority: u8, next_order: &mut u64) -> bool {
    if thread.effective_priority == priority {
        return false;
    }
    thread.effective_priority = priority;
    if is_priority_waiter(thread) {
        thread.wait_order = next_wait_order(next_order);
    }
    true
}

unsafe fn remove_waiter_locked(index: usize) {
    unsafe {
        let threads = { &mut *core::ptr::addr_of_mut!(THREADS) };
        match threads[index].wait_kind {
            WaitKind::Queue => {
                if threads[index].wait_object < MAX_QUEUES {
                    wait_queue_remove(
                        &mut QUEUES[threads[index].wait_object].waiters,
                        threads,
                        index,
                    );
                }
            }
            WaitKind::Mutex => {
                if threads[index].wait_object < MAX_MUTEXES {
                    wait_queue_remove(
                        &mut MUTEXES[threads[index].wait_object].waiters,
                        threads,
                        index,
                    );
                }
            }
            WaitKind::Cond => {
                if threads[index].wait_object < MAX_CONDS {
                    wait_queue_remove(
                        &mut CONDS[threads[index].wait_object].waiters,
                        threads,
                        index,
                    );
                }
            }
            WaitKind::Semaphore => {
                if threads[index].wait_object < MAX_SEMAPHORES {
                    wait_queue_remove(
                        &mut SEMAPHORES[threads[index].wait_object].waiters,
                        threads,
                        index,
                    );
                }
            }
            WaitKind::MessageSend => {
                if let Some(state) = { message_queue_state(threads[index].wait_object as u32) } {
                    wait_queue_remove(&mut (*state).send_waiters, threads, index);
                }
            }
            WaitKind::MessageReceive => {
                if let Some(state) = { message_queue_state(threads[index].wait_object as u32) } {
                    wait_queue_remove(&mut (*state).receive_waiters, threads, index);
                }
            }
            _ => {}
        }
    }
}

unsafe fn deferred_enqueue_locked(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    unsafe {
        if callback.is_none() {
            return LWP_CLOSED;
        }
        if DEFERRED_LEN == MAX_DEFERRED {
            DEBUG_COUNTERS.dropped_deferred_callbacks =
                DEBUG_COUNTERS.dropped_deferred_callbacks.saturating_add(1);
            return LWP_CLOSED;
        }
        let tail = (DEFERRED_HEAD + DEFERRED_LEN) % MAX_DEFERRED;
        DEFERRED[tail] = DeferredItem { callback, arg };
        DEFERRED_LEN += 1;
        if THREADS[DEFERRED_THREAD].state == ThreadState::Blocked {
            wake_index_locked(DEFERRED_THREAD, WaitResult::Signaled, 0);
        }
        LWP_SUCCESSFUL
    }
}

unsafe fn interrupt_deferred_enqueue_locked(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    unsafe {
        if callback.is_none() {
            return LWP_CLOSED;
        }
        let slots = interrupt_deferred_slots();
        let tail = (INTERRUPT_DEFERRED_HEAD + slots.len()) % MAX_INTERRUPT_DEFERRED;
        if !slots.enqueue_unreserved() {
            DEBUG_COUNTERS.dropped_deferred_callbacks =
                DEBUG_COUNTERS.dropped_deferred_callbacks.saturating_add(1);
            return LWP_CLOSED;
        }
        INTERRUPT_DEFERRED[tail] = DeferredItem { callback, arg };
        if THREADS[DEFERRED_THREAD].state == ThreadState::Blocked {
            wake_index_locked(DEFERRED_THREAD, WaitResult::Signaled, 0);
        }
        LWP_SUCCESSFUL
    }
}

unsafe fn interrupt_deferred_enqueue_reserved_locked(
    reservation: u32,
    callback: DeferredCallback,
    arg: *mut c_void,
) -> i32 {
    unsafe {
        if callback.is_none() {
            return LWP_CLOSED;
        }
        let slots = interrupt_deferred_slots();
        let tail = (INTERRUPT_DEFERRED_HEAD + slots.len()) % MAX_INTERRUPT_DEFERRED;
        if !slots.enqueue_reserved(reservation) {
            DEBUG_COUNTERS.dropped_deferred_callbacks =
                DEBUG_COUNTERS.dropped_deferred_callbacks.saturating_add(1);
            return LWP_CLOSED;
        }
        INTERRUPT_DEFERRED[tail] = DeferredItem { callback, arg };
        if THREADS[DEFERRED_THREAD].state == ThreadState::Blocked {
            wake_index_locked(DEFERRED_THREAD, WaitResult::Signaled, 0);
        }
        LWP_SUCCESSFUL
    }
}

unsafe fn process_timeouts_locked(now: u64, run_timer_callbacks: bool) {
    unsafe {
        let mut index = 0;
        let capacity = thread_capacity();
        while index < capacity {
            if THREADS[index].state == ThreadState::Blocked
                && THREADS[index].deadline != NO_DEADLINE
                && now.wrapping_sub(THREADS[index].deadline) < (1u64 << 63)
            {
                remove_waiter_locked(index);
                DEBUG_COUNTERS.timeouts = DEBUG_COUNTERS.timeouts.saturating_add(1);
                debug_locked(
                    EVENT_TIMEOUT,
                    thread_handle(index),
                    THREADS[index].wait_kind as u32,
                    0,
                );
                wake_index_locked(index, WaitResult::TimedOut, 0);
            }
            index += 1;
        }
        if run_timer_callbacks {
            loop {
                let now = arch::read_timebase();
                let mut earliest: *mut TimerNode = ptr::null_mut();
                let mut current = TIMER_HEAD;
                while !current.is_null() {
                    let timer = { &(*current).timer };
                    if timer_due(now, timer.deadline)
                        && (earliest.is_null()
                            || timer_precedes(
                                timer.deadline,
                                timer.order,
                                (*earliest).timer.deadline,
                                (*earliest).timer.order,
                            ))
                    {
                        earliest = current;
                    }
                    current = (*current).next;
                }
                if earliest.is_null() {
                    break;
                }

                let (callback, keyed_callback, key, arg, free_node) = {
                    let node = { &mut *earliest };
                    let timer = &mut node.timer;
                    let callback = timer.callback;
                    let keyed_callback = timer.keyed_callback;
                    let key = timer.key;
                    let arg = timer.arg;
                    if timer.period == 0 {
                        let mut link = core::ptr::addr_of_mut!(TIMER_HEAD);
                        while !(*link).is_null() {
                            if *link == earliest {
                                *link = (*earliest).next;
                                break;
                            }
                            link = { core::ptr::addr_of_mut!((**link).next) };
                        }
                        (callback, keyed_callback, key, arg, true)
                    } else {
                        timer.deadline = timer.deadline.wrapping_add(timer.period);
                        timer.order = next_timer_order_locked();
                        (callback, keyed_callback, key, arg, false)
                    }
                };
                if free_node {
                    recycle_timer_node(earliest);
                }
                if let Some(callback) = keyed_callback {
                    callback(key as u32, arg);
                } else if let Some(callback) = callback {
                    callback(arg);
                }
            }
        }
        reschedule_decrementer_locked(arch::read_timebase());
    }
}

unsafe fn reschedule_decrementer_locked(now: u64) {
    unsafe {
        let mut nearest: Option<u64> = None;
        let capacity = thread_capacity();
        for index in 0..capacity {
            let thread = &THREADS[index];
            if thread.state == ThreadState::Blocked && thread.deadline != NO_DEADLINE {
                let remaining = thread.deadline.wrapping_sub(now);
                if nearest.is_none_or(|current| (remaining.wrapping_sub(current) as i64) < 0) {
                    nearest = Some(remaining);
                }
            }
        }
        let mut timer_node = TIMER_HEAD;
        while !timer_node.is_null() {
            let timer = { &(*timer_node).timer };
            let remaining = timer.deadline.wrapping_sub(now);
            if nearest.is_none_or(|current| (remaining.wrapping_sub(current) as i64) < 0) {
                nearest = Some(remaining);
            }
            timer_node = (*timer_node).next;
        }
        let ticks = nearest.map_or(i32::MAX as u32, |remaining| {
            if remaining as i64 <= 0 {
                0
            } else {
                remaining.min(i32::MAX as u64) as u32
            }
        });
        arch::write_decrementer(ticks);
    }
}

unsafe extern "C" fn decrementer_tick() {
    unsafe {
        if !INITIALIZED {
            return;
        }
        process_timeouts_locked(arch::read_timebase(), true);
    }
}

unsafe extern "C" fn interrupt_return(frame: *const u32) -> *const u32 {
    unsafe {
        if frame.is_null() || !INITIALIZED {
            return frame;
        }

        let irq = arch::disable_interrupts();
        let current = CURRENT;
        let current_context = core::ptr::addr_of_mut!(THREADS[current].context);
        ptr::copy_nonoverlapping(frame.cast::<Context>(), current_context, 1);
        let mirror = THREADS[current].context_mirror;
        if !mirror.is_null() {
            ptr::copy_nonoverlapping(
                current_context.cast::<u8>(),
                mirror,
                core::mem::size_of::<Context>(),
            );
        }

        let should_reschedule = THREADS[current].state != ThreadState::Running || {
            has_higher_priority_ready_locked(THREADS[current].effective_priority)
        };

        let next = if should_reschedule {
            if THREADS[current].state == ThreadState::Running {
                THREADS[current].state = ThreadState::Ready;
            }
            select_next_locked()
        } else {
            current
        };

        if next != current {
            validate_stack_guard_from_interrupt_locked(current);
            THREADS[next].state = ThreadState::Running;
            CURRENT = next;
            _impure_ptr = thread_reent_ptr(next).cast();
            DEBUG_COUNTERS.context_switches = DEBUG_COUNTERS.context_switches.saturating_add(1);
            {
                record_debug_locked(
                    EVENT_SWITCH,
                    thread_handle(current),
                    thread_handle(next),
                    THREADS[next].effective_priority as u32,
                );
            }
        } else {
            THREADS[current].state = ThreadState::Running;
        }

        let restore = { context_for_restore_locked(next) };
        arch::restore_interrupts(irq);
        restore.cast::<u32>()
    }
}

unsafe fn has_higher_priority_ready_locked(current_priority: u8) -> bool {
    unsafe {
        let capacity = thread_capacity();
        (0..capacity).any(|index| {
            THREADS[index].state == ThreadState::Ready
                && THREADS[index].effective_priority > current_priority
        })
    }
}

#[cfg(test)]
unsafe fn has_higher_priority_ready(threads: *const Thread, current_priority: u8) -> bool {
    (0..THREAD_PAGE_SLOTS).any(|index| {
        let thread = unsafe { &*threads.add(index) };
        thread.state == ThreadState::Ready && thread.effective_priority > current_priority
    })
}

/// Recalculate inherited priority for a thread and each mutex owner above it.
///
/// C's `KThreadUpdateDynamicPrio` follows the blocked thread's mutex owner
/// after each effective-priority change. The bounded walk also prevents a
/// malformed ownership cycle from looping forever.
fn recompute_priority_chain<T: ThreadAccess + ?Sized>(
    threads: &mut T,
    mutexes: &[MutexState; MAX_MUTEXES],
    mut thread_index: usize,
    next_order: &mut u64,
) {
    // Following at most one owner edge per live thread is sufficient to break
    // malformed ownership cycles without encoding thread membership in a
    // fixed-width bit mask.
    for _ in 0..threads.len() {
        if thread_index >= threads.len() {
            break;
        }

        let mut priority = threads.thread(thread_index).base_priority;
        for mutex in mutexes {
            if mutex.used && mutex.owner == thread_index {
                let mut waiter = mutex.waiters.head;
                while waiter != NO_WAITER {
                    priority = priority.max(threads.thread(waiter).effective_priority);
                    waiter = threads.thread(waiter).wait_next;
                }
            }
        }
        if !update_effective_priority(threads.thread_mut(thread_index), priority, next_order) {
            break;
        }
        if threads.thread(thread_index).state != ThreadState::Blocked
            || threads.thread(thread_index).wait_kind != WaitKind::Mutex
        {
            break;
        }

        let mutex_index = threads.thread(thread_index).wait_object;
        if mutex_index >= MAX_MUTEXES {
            break;
        }
        let owner = mutexes[mutex_index].owner;
        if owner == NO_THREAD || owner == thread_index {
            break;
        }
        thread_index = owner;
    }
}

unsafe fn recompute_priority_chain_locked(thread_index: usize) {
    unsafe {
        recompute_priority_chain(
            &mut *core::ptr::addr_of_mut!(THREADS),
            &*core::ptr::addr_of!(MUTEXES),
            thread_index,
            &mut *core::ptr::addr_of_mut!(NEXT_WAIT_ORDER),
        );
    }
}

unsafe fn release_mutex_locked(index: usize, owner: usize) -> i32 {
    unsafe {
        let mutex = &mut MUTEXES[index];
        if !mutex.used || mutex.owner != owner {
            return LWP_CLOSED;
        }
        if mutex.recursion > 1 {
            mutex.recursion -= 1;
            return LWP_SUCCESSFUL;
        }
        if let Some(next_owner) = handoff_mutex(mutex, &mut *core::ptr::addr_of_mut!(THREADS)) {
            wake_index_locked(next_owner, WaitResult::Signaled, 0);
            recompute_priority_chain_locked(next_owner);
        }
        recompute_priority_chain_locked(owner);
        LWP_SUCCESSFUL
    }
}

unsafe extern "C" fn thread_bootstrap() -> ! {
    unsafe {
        arch::enable_interrupts();
        let index = CURRENT;
        let entry = THREADS[index].entry;
        let arg = THREADS[index].arg;
        let result = match entry {
            Some(entry) => entry(arg),
            None => ptr::null_mut(),
        };
        thread_exit(result)
    }
}

unsafe extern "C" fn idle_entry(_: *mut c_void) -> *mut c_void {
    unsafe {
        loop {
            let irq = arch::disable_interrupts();
            process_timeouts_locked(arch::read_timebase(), false);
            THREADS[CURRENT].state = ThreadState::Ready;
            switch_locked(CURRENT, irq);
            core::hint::spin_loop();
        }
    }
}

unsafe extern "C" fn deferred_entry(_: *mut c_void) -> *mut c_void {
    unsafe {
        loop {
            reap_detached_threads();
            let irq = arch::disable_interrupts();
            if interrupt_deferred_slots().len() == 0 && DEFERRED_LEN == 0 {
                let _ = block_current_locked(WaitKind::Deferred, 0, 0, NO_DEADLINE, irq);
                continue;
            }
            let item = if interrupt_deferred_slots().len() != 0 {
                let item = INTERRUPT_DEFERRED[INTERRUPT_DEFERRED_HEAD];
                INTERRUPT_DEFERRED[INTERRUPT_DEFERRED_HEAD] = DeferredItem::empty();
                INTERRUPT_DEFERRED_HEAD = (INTERRUPT_DEFERRED_HEAD + 1) % MAX_INTERRUPT_DEFERRED;
                if !interrupt_deferred_slots().dequeue() {
                    DEBUG_COUNTERS.dropped_deferred_callbacks =
                        DEBUG_COUNTERS.dropped_deferred_callbacks.saturating_add(1);
                }
                item
            } else {
                let item = DEFERRED[DEFERRED_HEAD];
                DEFERRED[DEFERRED_HEAD] = DeferredItem::empty();
                DEFERRED_HEAD = (DEFERRED_HEAD + 1) % MAX_DEFERRED;
                DEFERRED_LEN -= 1;
                item
            };
            arch::restore_interrupts(irq);
            if let Some(callback) = item.callback {
                callback(item.arg);
            }
            let irq = arch::disable_interrupts();
            DEBUG_COUNTERS.deferred_callbacks = DEBUG_COUNTERS.deferred_callbacks.saturating_add(1);
            debug_locked(
                EVENT_DEFERRED,
                DEFERRED_LEN as u32,
                interrupt_deferred_slots().len() as u32,
                0,
            );
            arch::restore_interrupts(irq);
            yield_thread();
        }
    }
}

pub fn now_ticks() -> u64 {
    arch::read_timebase()
}

/// Enters a scheduler-compatible interrupt critical section.
///
/// This is exposed for compatibility facades that must publish wait metadata
/// atomically with an LWP block operation. The returned state must be passed
/// exactly once to [`critical_exit`].
#[doc(hidden)]
pub fn critical_enter() -> u32 {
    arch::disable_interrupts()
}

/// Leaves a critical section created by [`critical_enter`].
#[doc(hidden)]
pub fn critical_exit(state: u32) {
    arch::restore_interrupts(state);
}

/// Creates a thread using caller-provided output and optional stack storage.
///
/// # Safety
/// `thread` must be writable. A non-null `stackbase` must identify an aligned,
/// uniquely owned region of at least `stack_size` bytes that outlives the thread.
/// `arg` and the entry function must obey their shared lifetime contract.
pub unsafe fn create_thread(
    thread: *mut LwpThread,
    entry: ThreadEntry,
    arg: *mut c_void,
    stackbase: *mut c_void,
    stack_size: u32,
    priority: u8,
) -> i32 {
    // SAFETY: The public caller supplies the stack and argument contract.
    unsafe { create_thread_inner(thread, entry, arg, stackbase, stack_size, priority, false) }
}

unsafe fn create_thread_inner(
    thread: *mut LwpThread,
    entry: ThreadEntry,
    arg: *mut c_void,
    stackbase: *mut c_void,
    stack_size: u32,
    priority: u8,
    std_owned: bool,
) -> i32 {
    if thread.is_null() || entry.is_none() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let size = if stack_size == 0 {
        8 * 1024
    } else {
        stack_size as usize
    };
    if size < 1024 || (!stackbase.is_null() && (stackbase as usize & 3) != 0) {
        unsafe { *thread = LWP_THREAD_NULL };
        return LWP_CLOSED;
    }

    let Some(index) = (unsafe { reserve_thread_slot() }) else {
        unsafe { *thread = LWP_THREAD_NULL };
        return LWP_CLOSED;
    };

    let mut owned_stack = ptr::null_mut();
    let mut owned_stack_bytes = 0usize;
    let base = if stackbase.is_null() {
        if index < THREAD_PAGE_SLOTS && size <= FALLBACK_STACK_SIZE {
            unsafe { STACKS[index].0.as_mut_ptr() }
        } else {
            let Some((allocation, allocation_size, aligned)) =
                (unsafe { allocate_owned_stack(size) })
            else {
                unsafe {
                    release_reserved_thread_slot(index);
                    *thread = LWP_THREAD_NULL;
                }
                return LWP_CLOSED;
            };
            owned_stack = allocation;
            owned_stack_bytes = allocation_size;
            aligned
        }
    } else {
        stackbase.cast::<u8>()
    };

    let irq = arch::disable_interrupts();
    unsafe {
        let top = base.add(size).cast();
        THREADS[index] = Thread::empty();
        THREADS[index].entry = entry;
        THREADS[index].arg = arg;
        THREADS[index].base_priority = decode_priority(priority as u32);
        THREADS[index].effective_priority = THREADS[index].base_priority;
        THREADS[index].state = ThreadState::Ready;
        THREADS[index].owned_stack = owned_stack;
        THREADS[index].owned_stack_bytes = owned_stack_bytes;
        THREADS[index].stack_bytes = size;
        THREADS[index].lifecycle = ThreadLifecycle::with_owner(std_owned);
        initialize_reent(&mut THREADS[index].reent);
        install_stack_guard(index, base);
        setup_context(index, top);
        *thread = thread_handle(index);
        DEBUG_COUNTERS.threads_created = DEBUG_COUNTERS.threads_created.saturating_add(1);
        debug_locked(
            EVENT_CREATE,
            thread_handle(index),
            priority as u32,
            size as u32,
        );
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

/// Creates a thread from a caller-provided stack-top pointer.
///
/// # Safety
/// `thread` must be writable. `stack_top` must be the aligned top of writable
/// stack storage that remains uniquely owned until the thread exits. `arg` and
/// the entry function must obey their shared lifetime contract.
pub unsafe fn create_thread_at_top(
    thread: *mut LwpThread,
    entry: ThreadEntry,
    arg: *mut c_void,
    stack_top: *mut c_void,
    priority: u8,
    start_suspended: bool,
) -> i32 {
    if thread.is_null() || entry.is_none() || stack_top.is_null() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let Some(index) = (unsafe { reserve_thread_slot() }) else {
        unsafe { *thread = LWP_THREAD_NULL };
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        THREADS[index] = Thread::empty();
        THREADS[index].entry = entry;
        THREADS[index].arg = arg;
        THREADS[index].base_priority = decode_priority(priority as u32);
        THREADS[index].effective_priority = THREADS[index].base_priority;
        THREADS[index].suspend_count = start_suspended as u16;
        THREADS[index].state = if start_suspended {
            ThreadState::Suspended
        } else {
            ThreadState::Ready
        };
        initialize_reent(&mut THREADS[index].reent);
        setup_context(index, stack_top);
        *thread = thread_handle(index);
        DEBUG_COUNTERS.threads_created = DEBUG_COUNTERS.threads_created.saturating_add(1);
        debug_locked(EVENT_CREATE, thread_handle(index), priority as u32, 0);
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn current_thread() -> LwpThread {
    ensure_initialized();
    unsafe { thread_handle(CURRENT) }
}

/// Validate a calling context before a std operation can block or use TLS.
pub fn current_thread_checked() -> Result<LwpThread, ThreadError> {
    if arch::in_exception() {
        return Err(ThreadError::ExceptionContext);
    }
    if arch::current_msr() & (1 << 15) == 0 {
        return Err(ThreadError::InterruptsDisabled);
    }
    Ok(current_thread())
}

/// Create a std worker with a checked stack size and the normal LWP priority.
/// Failure leaves ownership of the argument with the caller.
///
/// # Safety
/// entry must accept arg, and its code and referenced data must remain live
/// until the worker has finished. Success transfers the argument to the worker.
pub unsafe fn spawn_thread(
    entry: ThreadEntry,
    arg: *mut c_void,
    stack_size: usize,
) -> Result<LwpThread, ThreadError> {
    current_thread_checked()?;
    if entry.is_none() {
        return Err(ThreadError::MissingEntry);
    }
    let bytes = thread::stack_size(stack_size)?;
    let mut worker = LWP_THREAD_NULL;
    // SAFETY: The caller supplies the entry/argument lifetime. The existing
    // creation engine owns and reclaims the dynamic stack and reserved slot.
    let result =
        unsafe { create_thread_inner(&mut worker, entry, arg, ptr::null_mut(), bytes, 64, true) };
    if result != LWP_SUCCESSFUL {
        return Err(ThreadError::ResourceExhausted);
    }
    Ok(worker)
}

/// Register the runtime cleanup that must run after this thread's TLS values.
/// Repeated registration of the same function is permitted. No allocation occurs.
///
/// # Safety
/// cleanup must remain executable, return normally, and accept invocation after
/// all user TLS destructors. It must not unwind or terminate the thread.
pub unsafe fn register_thread_cleanup(cleanup: ThreadCleanup) -> Result<(), ThreadError> {
    current_thread_checked()?;
    let irq = arch::disable_interrupts();
    let result = unsafe {
        let thread = &mut THREADS[CURRENT];
        if thread.runtime_cleanup_ran {
            Err(ThreadError::CleanupComplete)
        } else if thread
            .runtime_cleanup
            .is_some_and(|registered| !core::ptr::fn_addr_eq(registered, cleanup))
        {
            Err(ThreadError::CleanupCallbackConflict)
        } else {
            thread.runtime_cleanup = Some(cleanup);
            Ok(())
        }
    };
    arch::restore_interrupts(irq);
    result
}

/// Store an observable scheduler name for the calling thread. Names are UTF-8
/// and are truncated at a character boundary to fit the 63-byte name payload.
pub fn set_current_thread_name(name: &str) -> Result<(), ThreadError> {
    current_thread_checked()?;
    let irq = arch::disable_interrupts();
    let result = unsafe { THREADS[CURRENT].name.set(name) };
    arch::restore_interrupts(irq);
    result
}

/// Copy a live LWP thread's actual name. Too-small outputs remain unchanged.
pub fn thread_name(thread: LwpThread, output: &mut [u8]) -> Result<usize, ThreadError> {
    let irq = arch::disable_interrupts();
    let result = match thread_index(thread) {
        Some(index) => unsafe { THREADS[index].name.copy_to(output) },
        None => Err(ThreadError::InvalidThread),
    };
    arch::restore_interrupts(irq);
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackBounds {
    pub low: usize,
    pub high: usize,
}

/// Return the stack extent above the guard and below the initial ABI frame.
/// A caller-supplied stack top without a known extent reports UnknownStack.
pub fn thread_stack_bounds(thread: LwpThread) -> Result<StackBounds, ThreadError> {
    let irq = arch::disable_interrupts();
    let result = match thread_index(thread) {
        Some(index) => unsafe {
            let thread = &THREADS[index];
            if thread.stack_guard.is_null() || thread.stack_bytes < 64 {
                Err(ThreadError::UnknownStack)
            } else {
                let base = thread.stack_guard as usize;
                Ok(StackBounds {
                    low: base + 32,
                    high: ((base + thread.stack_bytes) & !7) - 8,
                })
            }
        },
        None => Err(ThreadError::InvalidThread),
    };
    arch::restore_interrupts(irq);
    result
}

/// Returns the newlib reentrancy state for the running LWP thread.
///
/// Newlib calls this hook for ordinary libc entry points when dynamic
/// reentrancy is enabled. Exception handlers use the idle thread's state, as
/// the C Tuxedo implementation does.
/// Initializes caller-owned storage with the target newlib `_reent` layout.
///
/// # Safety
/// `storage` must point to at least 512 writable bytes aligned for `_reent`.
pub unsafe fn initialize_external_reent(storage: *mut c_void) -> bool {
    if storage.is_null() || (storage as usize & 3) != 0 {
        return false;
    }
    ensure_initialized();
    let mut initialized = ReentStorage::empty();
    unsafe { initialize_reent(&mut initialized) };
    unsafe { ptr::copy_nonoverlapping(initialized.0.as_ptr(), storage.cast::<u8>(), REENT_SIZE) };
    true
}

/// Makes caller-owned C-layout `PPCContext` storage authoritative for a thread.
///
/// Every switch-out and interrupt save is mirrored into this storage. Before
/// the thread is restored, its bytes are copied back into the driver's aligned
/// private context. This matches Tuxedo, where edits to a suspended thread's
/// public `KThread.ctx` affect the resumed register state. The byte copy avoids
/// imposing Rust's stronger `Context` alignment on caller-owned C storage.
///
/// # Safety
/// `storage` must point to at least 0x2c0 writable bytes with the C `PPCContext`
/// layout, be aligned to at least four bytes, and remain valid until the thread
/// is joined or the process terminates.
pub unsafe fn set_thread_context_mirror(thread: LwpThread, storage: *mut c_void) -> bool {
    if storage.is_null() || (storage as usize & 3) != 0 {
        return false;
    }
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return false;
    };
    let irq = arch::disable_interrupts();
    let valid = unsafe { THREADS[index].state != ThreadState::Unused };
    if valid {
        unsafe { THREADS[index].context_mirror = storage.cast::<u8>() };
    }
    arch::restore_interrupts(irq);
    valid
}

/// Makes caller-owned `_reent` storage authoritative for an LWP thread.
///
/// # Safety
/// `storage` must point to an initialized 512-byte `_reent` object that remains
/// valid until the thread is joined or the process terminates.
pub unsafe fn set_thread_reent_storage(thread: LwpThread, storage: *mut c_void) -> bool {
    if storage.is_null() || (storage as usize & 3) != 0 {
        return false;
    }
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return false;
    };
    let irq = arch::disable_interrupts();
    let valid = unsafe { THREADS[index].state != ThreadState::Unused };
    if valid {
        unsafe { THREADS[index].external_reent = storage.cast::<u8>() };
        if unsafe { index == CURRENT } {
            unsafe { _impure_ptr = storage };
        }
    }
    arch::restore_interrupts(irq);
    valid
}

pub fn current_reent() -> *mut c_void {
    unsafe {
        if !INITIALIZED {
            let irq = arch::disable_interrupts();
            if !FALLBACK_REENT_INITIALIZED {
                initialize_reent(&mut *ptr::addr_of_mut!(_impure_data));
                FALLBACK_REENT_INITIALIZED = true;
            }
            _impure_ptr = ptr::addr_of_mut!(_impure_data).cast();
            arch::restore_interrupts(irq);
            return ptr::addr_of_mut!(_impure_data).cast::<c_void>();
        }
        let index = if arch::in_exception() {
            IDLE_THREAD
        } else {
            CURRENT
        };
        thread_reent_ptr(index).cast::<c_void>()
    }
}

fn with_current_tls<T>(
    operation: impl FnOnce(&mut TlsRegistry, &mut TlsValues) -> Result<T, TlsError>,
) -> Result<T, TlsError> {
    if arch::in_exception() {
        return Err(TlsError::ExceptionContext);
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    // SAFETY: Interrupt masking serializes the registry and current thread.
    let result = unsafe {
        operation(
            &mut *ptr::addr_of_mut!(TLS_REGISTRY),
            &mut THREADS[CURRENT].tls,
        )
    };
    arch::restore_interrupts(irq);
    result
}

/// Create a TLS key. Access does not allocate or invoke a destructor.
///
/// # Safety
/// A destructor must accept every non-null value stored for this key. Its code
/// must remain valid until all threads that use the key have completed cleanup.
pub unsafe fn tls_create(destructor: Option<TlsDestructor>) -> Result<TlsKey, TlsError> {
    with_current_tls(|registry, _values| registry.create(destructor))
}

/// Delete a key without running destructors, as required by OS TLS key semantics.
pub fn tls_destroy(key: TlsKey) -> Result<(), TlsError> {
    with_current_tls(|registry, _values| registry.destroy(key))
}

/// Read the calling thread's value. A new key has a null value in each thread.
pub fn tls_get(key: TlsKey) -> Result<*mut c_void, TlsError> {
    with_current_tls(|registry, values| registry.get(values, key))
}

/// Store a value for the calling thread without allocating.
///
/// # Safety
/// A non-null value must satisfy the key destructor's contract until it is
/// replaced, the key is deleted, or the calling thread completes cleanup.
pub unsafe fn tls_set(key: TlsKey, value: *mut c_void) -> Result<(), TlsError> {
    with_current_tls(|registry, values| registry.set(values, key, value))
}

fn run_current_tls_destructors() -> Result<(), TlsError> {
    let irq = arch::disable_interrupts();
    // SAFETY: This function is called by the current thread before it exits.
    unsafe {
        if THREADS[CURRENT].tls_destructing {
            arch::restore_interrupts(irq);
            return Err(TlsError::ReentrantCleanup);
        }
        THREADS[CURRENT].tls_destructing = true;
    }
    arch::restore_interrupts(irq);
    for _ in 0..TLS_DESTRUCTOR_ITERATIONS {
        for index in 0..TLS_KEY_CAPACITY {
            let irq = arch::disable_interrupts();
            // SAFETY: The value is removed while serialized. No registry or
            // thread reference remains live when the destructor is invoked.
            let pending = unsafe {
                (&*ptr::addr_of!(TLS_REGISTRY)).take_destructor(&mut THREADS[CURRENT].tls, index)
            };
            arch::restore_interrupts(irq);
            if let Some((destructor, value)) = pending {
                // SAFETY: tls_create and tls_set require the destructor/value
                // lifetime contract. Callbacks run in the exiting thread.
                unsafe { destructor(value) };
            }
        }
        let irq = arch::disable_interrupts();
        let pending =
            unsafe { (&*ptr::addr_of!(TLS_REGISTRY)).has_destructors(&THREADS[CURRENT].tls) };
        arch::restore_interrupts(irq);
        if !pending {
            return Ok(());
        }
    }
    Err(TlsError::DestructorLimit)
}

fn run_current_thread_cleanup() -> Result<(), TlsError> {
    let irq = arch::disable_interrupts();
    unsafe {
        let thread = &mut THREADS[CURRENT];
        if let Some(result) = thread.tls_cleanup_result {
            arch::restore_interrupts(irq);
            return result;
        }
        if thread.cleanup_running {
            arch::restore_interrupts(irq);
            return Err(TlsError::ReentrantCleanup);
        }
        thread.cleanup_running = true;
    }
    arch::restore_interrupts(irq);
    let mut result = run_current_tls_destructors();
    let irq = arch::disable_interrupts();
    let cleanup = unsafe {
        let thread = &mut THREADS[CURRENT];
        thread.runtime_cleanup_ran = true;
        thread.runtime_cleanup.take()
    };
    arch::restore_interrupts(irq);
    if let Some(cleanup) = cleanup {
        // SAFETY: Registration requires a live, non-unwinding callback. User
        // TLS destruction has finished, and no scheduler reference is held.
        unsafe { cleanup() };
    }
    let irq = arch::disable_interrupts();
    unsafe {
        let pending = (&*ptr::addr_of!(TLS_REGISTRY)).has_destructors(&THREADS[CURRENT].tls);
        let thread = &mut THREADS[CURRENT];
        if result.is_ok() && pending {
            result = Err(TlsError::DestructorLimit);
        }
        if result.is_ok() && thread.cleanup_failed {
            result = Err(TlsError::ReentrantCleanup);
        }
        thread.cleanup_running = false;
        thread.tls_cleanup_result = Some(result);
    }
    arch::restore_interrupts(irq);
    result
}

unsafe fn record_cleanup_failure(error: TlsError) {
    unsafe {
        if THREADS[CURRENT].cleanup_failed {
            return;
        }
        THREADS[CURRENT].cleanup_failed = true;
        THREAD_CLEANUP_COUNTERS.tls_failures =
            THREAD_CLEANUP_COUNTERS.tls_failures.saturating_add(1);
        debug_locked(EVENT_TLS_FAILURE, thread_handle(CURRENT), error as u32, 0);
    }
}

/// Run TLS destructors and registered runtime cleanup on the calling stack.
/// The initial thread uses this without terminating or freeing its own stack.
///
/// # Safety
/// No TLS value may be borrowed, and no user code that can access this thread's
/// TLS may run after cleanup. Call once at the runtime's final boundary.
pub unsafe fn cleanup_current_thread() -> Result<(), ThreadError> {
    current_thread_checked()?;
    let result = run_current_thread_cleanup();
    if let Err(error) = result {
        let irq = arch::disable_interrupts();
        unsafe { record_cleanup_failure(error) };
        arch::restore_interrupts(irq);
        return Err(ThreadError::TlsCleanup);
    }
    Ok(())
}

/// Read cleanup counters without losing failures from detached threads.
pub fn thread_cleanup_counters() -> ThreadCleanupCounters {
    let irq = arch::disable_interrupts();
    let counters = unsafe { THREAD_CLEANUP_COUNTERS };
    arch::restore_interrupts(irq);
    counters
}

struct FinishedThread {
    stack: *mut u8,
    stack_bytes: usize,
    result: *mut c_void,
    cleanup_failed: bool,
}

// Caller must hold the scheduler lock and ensure this finished thread is not
// CURRENT and is exclusively owned by its joiner or detached reaper.
unsafe fn take_finished_thread_locked(index: usize) -> FinishedThread {
    unsafe {
        let thread = &mut THREADS[index];
        let finished = FinishedThread {
            stack: thread.owned_stack,
            stack_bytes: thread.owned_stack_bytes,
            result: thread.result,
            cleanup_failed: thread.cleanup_failed,
        };
        *thread = Thread::empty();
        finished
    }
}

fn release_finished_stack(thread: &FinishedThread) {
    if !thread.stack.is_null() {
        // SAFETY: take_finished_thread_locked transferred this finished
        // thread's unique allocation. This caller runs on another stack.
        unsafe { workspace_free(thread.stack, thread.stack_bytes) };
    }
}

fn reap_detached_threads() {
    let mut index = 3;
    loop {
        let irq = arch::disable_interrupts();
        let finished = unsafe {
            if index >= thread_capacity() {
                arch::restore_interrupts(irq);
                break;
            }
            if index != CURRENT
                && THREADS[index].state == ThreadState::Finished
                && THREADS[index].lifecycle.is_detached()
                && THREADS[index].completion.is_complete()
            {
                Some(take_finished_thread_locked(index))
            } else {
                None
            }
        };
        arch::restore_interrupts(irq);
        if let Some(thread) = finished {
            release_finished_stack(&thread);
            let irq = arch::disable_interrupts();
            // Report completion only after the owned stack has been released.
            unsafe {
                THREAD_CLEANUP_COUNTERS.detached_reaped =
                    THREAD_CLEANUP_COUNTERS.detached_reaped.saturating_add(1);
            }
            arch::restore_interrupts(irq);
        }
        index += 1;
    }
}

/// Detach a thread. A finished detached thread is reclaimed on another stack.
/// Reject internal threads, repeated detach, and a thread claimed by a joiner.
pub fn detach_thread(thread: LwpThread) -> i32 {
    match detach_thread_inner(thread, false) {
        Ok(()) => LWP_SUCCESSFUL,
        Err(_) => LWP_CLOSED,
    }
}

/// Detach an exclusively owned std native handle.
///
/// # Safety
/// thread must be owned from spawn_thread, with no concurrent join/detach. This
/// consumes that ownership. Invalid sentinels return an error.
pub unsafe fn detach_thread_checked(thread: LwpThread) -> Result<(), ThreadError> {
    current_thread_checked()?;
    detach_thread_inner(thread, true)
}

fn detach_thread_inner(thread: LwpThread, std_owned: bool) -> Result<(), ThreadError> {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    let result = unsafe {
        match thread_index(thread) {
            Some(index) if index >= 3 && THREADS[index].state != ThreadState::Reserved => THREADS
                [index]
                .lifecycle
                .detach(std_owned)
                .map_err(|_error| ThreadError::InvalidThread),
            _ => Err(ThreadError::InvalidThread),
        }
    };
    arch::restore_interrupts(irq);
    if result.is_ok() {
        reap_detached_threads();
    }
    result
}

/// newlib dynamic reentrancy hook. The returned storage follows DevkitPPC's
/// `_reent` layout and is initialized when each LWP thread is created.
#[unsafe(no_mangle)]
pub extern "C" fn __getreent() -> *mut c_void {
    current_reent()
}

/// Copy the last saved PowerPC context for a thread.
///
/// A running thread's live register state is not stored in the scheduler until
/// it switches out. Exception handlers must use their exception frame for that
/// thread and use this snapshot only for other threads.
pub fn thread_context_snapshot(thread: LwpThread) -> Option<Context> {
    ensure_initialized();
    let index = thread_index(thread)?;
    let irq = arch::disable_interrupts();
    let context = unsafe {
        if THREADS[index].state == ThreadState::Unused {
            None
        } else if !THREADS[index].context_mirror.is_null() {
            Some(ptr::read_unaligned(
                THREADS[index].context_mirror.cast::<Context>(),
            ))
        } else {
            Some(THREADS[index].context)
        }
    };
    arch::restore_interrupts(irq);
    context
}

pub fn thread_argument(thread: LwpThread) -> Option<*mut c_void> {
    ensure_initialized();
    let index = thread_index(thread)?;
    let irq = arch::disable_interrupts();
    let arg = unsafe { THREADS[index].arg };
    arch::restore_interrupts(irq);
    Some(arg)
}

pub fn thread_handle_by_argument(arg: *mut c_void) -> Option<LwpThread> {
    if arg.is_null() {
        return None;
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    let capacity = thread_capacity();
    let mut result = None;
    for index in 3..capacity {
        let thread = unsafe { &THREADS[index] };
        if thread.state != ThreadState::Unused
            && thread.state != ThreadState::Reserved
            && thread.arg == arg
        {
            result = Some(thread_handle(index));
            break;
        }
    }
    arch::restore_interrupts(irq);
    result
}

pub fn suspend_thread(thread: LwpThread) -> i32 {
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if THREADS[index].state == ThreadState::Unused {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        THREADS[index].suspend_count = THREADS[index].suspend_count.saturating_add(1);
        if THREADS[index].suspend_count > 1 {
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        if index == CURRENT {
            THREADS[index].state = ThreadState::Suspended;
            switch_locked(index, irq);
        } else {
            if THREADS[index].state == ThreadState::Ready {
                THREADS[index].state = ThreadState::Suspended;
            }
            arch::restore_interrupts(irq);
        }
    }
    LWP_SUCCESSFUL
}

pub fn resume_thread(thread: LwpThread) -> i32 {
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if THREADS[index].suspend_count == 0 {
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        THREADS[index].suspend_count -= 1;
        if THREADS[index].suspend_count == 0 && THREADS[index].state == ThreadState::Suspended {
            THREADS[index].state = ThreadState::Ready;
        }
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn thread_is_suspended(thread: LwpThread) -> bool {
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return false;
    };
    let irq = arch::disable_interrupts();
    let value = unsafe { THREADS[index].suspend_count != 0 };
    arch::restore_interrupts(irq);
    value
}

pub fn set_thread_priority(thread: LwpThread, priority: u32) {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    let index = if thread == LWP_THREAD_NULL {
        unsafe {
            (CURRENT < thread_capacity() && THREADS[CURRENT].state != ThreadState::Unused)
                .then_some(CURRENT)
        }
    } else {
        thread_index(thread)
    };
    let Some(index) = index else {
        arch::restore_interrupts(irq);
        return;
    };
    unsafe {
        THREADS[index].base_priority = decode_priority(priority);
        recompute_priority_chain_locked(index);
    }
    arch::restore_interrupts(irq);
}

/// Joins a thread and optionally writes its result pointer.
///
/// # Safety
/// A non-null `value` must be writable for one pointer for the duration of the call.
pub unsafe fn join_thread(thread: LwpThread, value: *mut *mut c_void) -> i32 {
    match join_thread_inner(thread, false) {
        Ok(result) => {
            if !value.is_null() {
                // SAFETY: The caller supplies a writable result pointer.
                unsafe { value.write(result) };
            }
            LWP_SUCCESSFUL
        }
        Err(_) => LWP_CLOSED,
    }
}

/// Join after validating the calling context. Completion includes TLS and the
/// registered runtime cleanup. Errors do not masquerade as a Rust panic result.
///
/// # Safety
/// thread must be exclusively owned from spawn_thread, with no concurrent
/// join/detach. This consumes ownership. Invalid sentinels return an error.
pub unsafe fn join_thread_checked(thread: LwpThread) -> Result<*mut c_void, ThreadError> {
    current_thread_checked()?;
    join_thread_inner(thread, true)
}

fn join_thread_inner(thread: LwpThread, std_owned: bool) -> Result<*mut c_void, ThreadError> {
    ensure_initialized();
    let Some(index) = thread_index(thread) else {
        return Err(ThreadError::InvalidThread);
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if index == CURRENT
            || index < 3
            || THREADS[index].state == ThreadState::Unused
            || THREADS[index].lifecycle.begin_join(std_owned).is_err()
        {
            arch::restore_interrupts(irq);
            return Err(ThreadError::InvalidThread);
        }
        if THREADS[index].state != ThreadState::Finished {
            if !THREADS[index].join_waiters.is_empty() {
                THREADS[index].lifecycle.cancel_join();
                arch::restore_interrupts(irq);
                return Err(ThreadError::InvalidThread);
            }
            wait_queue_insert_locked(
                core::ptr::addr_of_mut!(THREADS[index].join_waiters),
                CURRENT,
            );
            let wait_result = block_current_locked(WaitKind::Join, index, 0, NO_DEADLINE, irq);
            if wait_result == WaitResult::Cancelled {
                let irq = arch::disable_interrupts();
                THREADS[index].lifecycle.cancel_join();
                arch::restore_interrupts(irq);
                return Err(ThreadError::Cancelled);
            }
        } else {
            arch::restore_interrupts(irq);
        }
        let irq2 = arch::disable_interrupts();
        if THREADS[index].state != ThreadState::Finished || !THREADS[index].completion.is_complete()
        {
            THREADS[index].lifecycle.cancel_join();
            arch::restore_interrupts(irq2);
            return Err(ThreadError::InvalidThread);
        }
        let finished = take_finished_thread_locked(index);
        arch::restore_interrupts(irq2);
        release_finished_stack(&finished);
        if finished.cleanup_failed {
            return Err(ThreadError::TlsCleanup);
        }
        Ok(finished.result)
    }
}

pub fn thread_exit(value: *mut c_void) -> ! {
    ensure_initialized();
    let cleanup = run_current_thread_cleanup();
    let irq = arch::disable_interrupts();
    unsafe {
        let current = CURRENT;
        if let Err(error) = cleanup {
            record_cleanup_failure(error);
        }
        THREADS[current].result = value;
        THREADS[current].state = ThreadState::Finished;
        THREADS[current].wait_kind = WaitKind::None;
        THREADS[current].completion.publish();
        let mut waiters = THREADS[current].join_waiters;
        THREADS[current].join_waiters = WaitQueue::empty();
        wake_all_locked(&mut waiters, WaitResult::Signaled, 0);
        DEBUG_COUNTERS.threads_exited = DEBUG_COUNTERS.threads_exited.saturating_add(1);
        debug_locked(EVENT_EXIT, thread_handle(current), value as usize as u32, 0);
        if THREADS[current].lifecycle.is_detached() {
            wake_index_locked(DEFERRED_THREAD, WaitResult::Signaled, 0);
        }
        switch_locked(current, irq);
    }
    loop {
        core::hint::spin_loop();
    }
}

pub fn yield_thread() {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        process_timeouts_locked(arch::read_timebase(), false);
        let current = CURRENT;
        if THREADS[current].state == ThreadState::Running {
            THREADS[current].state = ThreadState::Ready;
        }
        switch_locked(current, irq);
    }
}

pub fn reschedule(priority: u32) {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        process_timeouts_locked(arch::read_timebase(), false);
        rotate_priority_locked(priority.min(LWP_PRIO_HIGHEST) as u8);
        let current = CURRENT;
        if THREADS[current].state == ThreadState::Running {
            THREADS[current].state = ThreadState::Ready;
        }
        switch_locked(current, irq);
    }
}

pub fn sleep_ticks(ticks: u64) {
    ensure_initialized();
    if ticks == 0 {
        yield_thread();
        return;
    }
    let irq = arch::disable_interrupts();
    unsafe {
        let wait_result = block_current_locked(
            WaitKind::Sleep,
            0,
            0,
            arch::read_timebase().wrapping_add(ticks),
            irq,
        );
        debug_assert_eq!(wait_result, WaitResult::TimedOut);
    }
}

/// Sleep for at least the requested duration using bounded LWP deadlines.
/// Recheck real elapsed ticks after an early wake. A zero duration needs no wait.
pub fn sleep_for(duration: core::time::Duration) -> Result<(), ThreadError> {
    current_thread_checked()?;
    if duration.is_zero() {
        return Ok(());
    }
    let mut budget = futex::TickBudget::new(duration, TICKS_PER_SECOND);
    loop {
        let started = arch::read_timebase();
        let irq = arch::disable_interrupts();
        let deadline = match futex::deadline(arch::read_timebase(), budget.chunk()) {
            Ok(deadline) => deadline,
            Err(_) => {
                arch::restore_interrupts(irq);
                return Err(ThreadError::InvalidTimeout);
            }
        };
        // SAFETY: The checked thread context can block. The scheduler restores
        // irq after it publishes the sleep and switches to another stack.
        let result = unsafe { block_current_locked(WaitKind::Sleep, 0, 0, deadline, irq) };
        if result == WaitResult::Cancelled {
            return Err(ThreadError::Cancelled);
        }
        if budget.elapse(arch::read_timebase().wrapping_sub(started)) {
            return Ok(());
        }
    }
}

/// Initializes a thread queue handle.
///
/// # Safety
/// `output` must be writable for one [`QueueHandle`].
pub unsafe fn init_queue(output: *mut QueueHandle) -> i32 {
    if output.is_null() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        if let Some(index) = (0..MAX_QUEUES).find(|&i| !QUEUES[i].used) {
            QUEUES[index] = Queue {
                used: true,
                waiters: WaitQueue::empty(),
            };
            *output = synchronization_handle(index);
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        *output = LWP_TQUEUE_NULL;
    }
    arch::restore_interrupts(irq);
    LWP_CLOSED
}

pub fn close_queue(queue: QueueHandle) {
    let Some(index) = decode(queue, MAX_QUEUES, LWP_TQUEUE_NULL) else {
        return;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if QUEUES[index].used {
            wake_all_locked(&mut QUEUES[index].waiters, WaitResult::Cancelled, 0);
            QUEUES[index] = Queue::empty();
        }
    }
    arch::restore_interrupts(irq);
}

pub fn queue_sleep(queue: QueueHandle) -> i32 {
    ensure_initialized();
    let Some(index) = decode(queue, MAX_QUEUES, LWP_TQUEUE_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !QUEUES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        wait_queue_insert_locked(core::ptr::addr_of_mut!(QUEUES[index].waiters), CURRENT);
        block_current_locked(WaitKind::Queue, index, 0, NO_DEADLINE, irq) as i32
    }
}

pub fn queue_signal(queue: QueueHandle) {
    let Some(index) = decode(queue, MAX_QUEUES, LWP_TQUEUE_NULL) else {
        return;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if QUEUES[index].used {
            let _ = try_wake_one_locked(&mut QUEUES[index].waiters, WaitResult::Signaled, 0);
        }
    }
    arch::restore_interrupts(irq);
}

pub fn queue_broadcast(queue: QueueHandle) {
    let Some(index) = decode(queue, MAX_QUEUES, LWP_TQUEUE_NULL) else {
        return;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if QUEUES[index].used {
            wake_all_locked(&mut QUEUES[index].waiters, WaitResult::Signaled, 0);
        }
    }
    arch::restore_interrupts(irq);
}

/// Initializes a mutex handle.
///
/// # Safety
/// `output` must be writable for one [`MutexHandle`].
pub unsafe fn init_mutex(output: *mut MutexHandle, recursive: bool) -> i32 {
    if output.is_null() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        if let Some(index) = (0..MAX_MUTEXES).find(|&i| !MUTEXES[i].used) {
            // The C compatibility layer only allocates the handle slot here;
            // it does not reinitialize the underlying static KRMutex object.
            MUTEXES[index].used = true;
            MUTEXES[index].recursive = recursive;
            *output = synchronization_handle(index);
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        *output = LWP_MUTEX_NULL;
    }
    arch::restore_interrupts(irq);
    LWP_CLOSED
}

pub fn close_mutex(mutex: MutexHandle) -> i32 {
    let Some(index) = decode(mutex, MAX_MUTEXES, LWP_MUTEX_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !MUTEXES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        // C frees only the compatibility handle slot, even when the mutex is
        // owned or has waiters. Preserve the underlying state for slot reuse.
        MUTEXES[index].used = false;
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn mutex_try_lock(mutex: MutexHandle) -> i32 {
    ensure_initialized();
    let Some(index) = decode(mutex, MAX_MUTEXES, LWP_MUTEX_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    let rc = unsafe {
        let current = CURRENT;
        let state = &mut MUTEXES[index];
        if !state.used {
            LWP_CLOSED
        } else if state.owner == NO_THREAD {
            state.owner = current;
            state.recursion = 1;
            LWP_SUCCESSFUL
        } else if state.owner == current && state.recursive {
            state.recursion = state.recursion.saturating_add(1);
            LWP_SUCCESSFUL
        } else {
            1
        }
    };
    arch::restore_interrupts(irq);
    rc
}

pub fn mutex_lock(mutex: MutexHandle) -> i32 {
    ensure_initialized();
    let Some(index) = decode(mutex, MAX_MUTEXES, LWP_MUTEX_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        let current = CURRENT;
        let state = &mut MUTEXES[index];
        if !state.used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        if state.owner == NO_THREAD {
            state.owner = current;
            state.recursion = 1;
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        if state.owner == current && state.recursive {
            state.recursion = state.recursion.saturating_add(1);
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        wait_queue_insert_locked(core::ptr::addr_of_mut!(state.waiters), current);
        let owner = state.owner;
        if owner < thread_capacity() {
            recompute_priority_chain_locked(owner);
        }
        match block_current_locked(WaitKind::Mutex, index, 0, NO_DEADLINE, irq) {
            WaitResult::Signaled => LWP_SUCCESSFUL,
            WaitResult::TimedOut | WaitResult::Cancelled => LWP_CLOSED,
        }
    }
}

pub fn mutex_unlock(mutex: MutexHandle) -> i32 {
    ensure_initialized();
    let Some(index) = decode(mutex, MAX_MUTEXES, LWP_MUTEX_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    let rc = unsafe { release_mutex_locked(index, CURRENT) };
    arch::restore_interrupts(irq);
    rc
}

/// Initializes a condition-variable handle.
///
/// # Safety
/// `output` must be writable for one [`CondHandle`].
pub unsafe fn init_cond(output: *mut CondHandle) -> i32 {
    if output.is_null() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        if let Some(index) = (0..MAX_CONDS).find(|&i| !CONDS[i].used) {
            // C allocates a handle for the static KCondVar object without
            // clearing waiters that may still be queued on a destroyed slot.
            CONDS[index].used = true;
            *output = synchronization_handle(index);
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        *output = LWP_COND_NULL;
    }
    arch::restore_interrupts(irq);
    LWP_CLOSED
}

pub fn close_cond(cond: CondHandle) -> i32 {
    let Some(index) = decode(cond, MAX_CONDS, LWP_COND_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !CONDS[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        // C only releases the compatibility slot. It does not cancel or wake
        // waiters queued on the underlying static condition variable.
        CONDS[index].used = false;
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn cond_wait(cond: CondHandle, mutex: MutexHandle, timeout_ticks: Option<u64>) -> i32 {
    ensure_initialized();
    let Some(cond_index) = decode(cond, MAX_CONDS, LWP_COND_NULL) else {
        return LWP_CLOSED;
    };
    let Some(mutex_index) = decode(mutex, MAX_MUTEXES, LWP_MUTEX_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !CONDS[cond_index].used
            || !MUTEXES[mutex_index].used
            || MUTEXES[mutex_index].owner != CURRENT
        {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        let recursion = MUTEXES[mutex_index].recursion;
        if matches!(timeout_ticks, Some(0)) {
            arch::restore_interrupts(irq);
            return WaitResult::TimedOut as i32;
        }
        MUTEXES[mutex_index].recursion = 1;
        if release_mutex_locked(mutex_index, CURRENT) != LWP_SUCCESSFUL {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        wait_queue_insert_locked(core::ptr::addr_of_mut!(CONDS[cond_index].waiters), CURRENT);
        let deadline = timeout_ticks
            .map(|ticks| arch::read_timebase().wrapping_add(ticks))
            .unwrap_or(NO_DEADLINE);
        let result = block_current_locked(WaitKind::Cond, cond_index, 0, deadline, irq);
        let lock_rc = mutex_lock(mutex);
        if lock_rc != LWP_SUCCESSFUL {
            return lock_rc;
        }
        if recursion > 1 {
            let irq2 = arch::disable_interrupts();
            MUTEXES[mutex_index].recursion = recursion;
            arch::restore_interrupts(irq2);
        }
        result as i32
    }
}

pub fn cond_signal(cond: CondHandle) -> i32 {
    let Some(index) = decode(cond, MAX_CONDS, LWP_COND_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !CONDS[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        wake_one_locked(&mut CONDS[index].waiters, WaitResult::Signaled, 0);
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn cond_broadcast(cond: CondHandle) -> i32 {
    let Some(index) = decode(cond, MAX_CONDS, LWP_COND_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !CONDS[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        wake_all_locked(&mut CONDS[index].waiters, WaitResult::Signaled, 0);
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

/// Initializes a semaphore handle.
///
/// # Safety
/// `output` must be writable for one [`SemaphoreHandle`].
pub unsafe fn init_semaphore(output: *mut SemaphoreHandle, start: u32, _max: u32) -> i32 {
    if output.is_null() {
        return LWP_CLOSED;
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        if let Some(index) = (0..MAX_SEMAPHORES).find(|&i| !SEMAPHORES[i].used) {
            reinitialize_semaphore_state(&mut SEMAPHORES[index], start);
            *output = synchronization_handle(index);
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        *output = LWP_SEM_NULL;
    }
    arch::restore_interrupts(irq);
    LWP_CLOSED
}

fn reinitialize_semaphore_state(state: &mut SemaphoreState, start: u32) {
    // C stores waiters in a global queue that is independent of the slot table.
    // Reusing a freed slot resets only the count; old waiters remain queued by
    // the same numeric semaphore handle.
    let waiters = state.waiters;
    *state = SemaphoreState {
        used: true,
        value: start as i32,
        waiters,
    };
}

pub fn close_semaphore(sem: SemaphoreHandle) -> i32 {
    let Some(index) = decode(sem, MAX_SEMAPHORES, LWP_SEM_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !SEMAPHORES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        // C only frees the compatibility slot. Blocked waiters remain on the
        // global semaphore queue and still carry this numeric handle.
        SEMAPHORES[index].used = false;
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

pub fn semaphore_wait(sem: SemaphoreHandle) -> i32 {
    ensure_initialized();
    let Some(index) = decode(sem, MAX_SEMAPHORES, LWP_SEM_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !SEMAPHORES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        SEMAPHORES[index].value = SEMAPHORES[index].value.wrapping_sub(1);
        if SEMAPHORES[index].value >= 0 {
            arch::restore_interrupts(irq);
            return LWP_SUCCESSFUL;
        }
        wait_queue_insert_locked(core::ptr::addr_of_mut!(SEMAPHORES[index].waiters), CURRENT);
        match block_current_locked(WaitKind::Semaphore, index, 0, NO_DEADLINE, irq) {
            WaitResult::Signaled => LWP_SUCCESSFUL,
            WaitResult::TimedOut | WaitResult::Cancelled => LWP_CLOSED,
        }
    }
}

pub fn semaphore_post(sem: SemaphoreHandle) -> i32 {
    let Some(index) = decode(sem, MAX_SEMAPHORES, LWP_SEM_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !SEMAPHORES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        let previous = SEMAPHORES[index].value;
        SEMAPHORES[index].value = previous.wrapping_add(1);
        if previous < 0 {
            let _ = try_wake_one_locked(&mut SEMAPHORES[index].waiters, WaitResult::Signaled, 0);
        }
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

/// Writes a semaphore's current value.
///
/// # Safety
/// `output` must be writable for one `u32`.
pub unsafe fn semaphore_get_value(sem: SemaphoreHandle, output: *mut u32) -> i32 {
    if output.is_null() {
        return LWP_CLOSED;
    }
    let Some(index) = decode(sem, MAX_SEMAPHORES, LWP_SEM_NULL) else {
        return LWP_CLOSED;
    };
    let irq = arch::disable_interrupts();
    unsafe {
        if !SEMAPHORES[index].used {
            arch::restore_interrupts(irq);
            return LWP_CLOSED;
        }
        *output = if SEMAPHORES[index].value >= 0 {
            SEMAPHORES[index].value as u32
        } else {
            0
        };
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

/// Initializes a message-queue handle.
///
/// # Safety
/// `output` must be writable for one [`MessageQueueHandle`].
pub unsafe fn init_message_queue(output: *mut MessageQueueHandle, count: u32) -> i32 {
    if output.is_null() || count == 0 {
        return MQ_ERROR_TOOMANY;
    }
    let capacity = count as usize;
    let Some(message_bytes) = capacity.checked_mul(core::mem::size_of::<Message>()) else {
        unsafe { *output = MQ_BOX_NULL };
        return MQ_ERROR_TOOMANY;
    };
    let state_ptr = unsafe { workspace_alloc(core::mem::size_of::<MessageQueueState>()) }
        .cast::<MessageQueueState>();
    if state_ptr.is_null() {
        unsafe { *output = MQ_BOX_NULL };
        return MQ_ERROR_TOOMANY;
    }
    let messages = unsafe { workspace_alloc(message_bytes) }.cast::<Message>();
    if messages.is_null() {
        unsafe { workspace_free(state_ptr.cast(), core::mem::size_of::<MessageQueueState>()) };
        unsafe { *output = MQ_BOX_NULL };
        return MQ_ERROR_TOOMANY;
    }
    unsafe { ptr::write_bytes(messages, 0, capacity) };

    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        state_ptr.write(MessageQueueState {
            used: true,
            capacity,
            head: 0,
            len: 0,
            messages,
            send_waiters: WaitQueue::empty(),
            receive_waiters: WaitQueue::empty(),
            next: MESSAGE_QUEUE_HEAD,
        });
        MESSAGE_QUEUE_HEAD = state_ptr;
        *output = encode_message_queue_handle(state_ptr);
    }
    arch::restore_interrupts(irq);
    MQ_ERROR_SUCCESSFUL
}

pub fn close_message_queue(mq: MessageQueueHandle) {
    let irq = arch::disable_interrupts();
    let state_ptr = unsafe {
        let Some(state_ptr) = message_queue_state(mq) else {
            arch::restore_interrupts(irq);
            return;
        };
        let mut link = core::ptr::addr_of_mut!(MESSAGE_QUEUE_HEAD);
        while !(*link).is_null() {
            if *link == state_ptr {
                *link = (*state_ptr).next;
                break;
            }
            link = core::ptr::addr_of_mut!((**link).next);
        }
        (*state_ptr).used = false;
        state_ptr
    };
    arch::restore_interrupts(irq);

    unsafe {
        let state = &*state_ptr;
        let bytes = state
            .capacity
            .saturating_mul(core::mem::size_of::<Message>());
        workspace_free(state.messages.cast(), bytes);
        workspace_free(state_ptr.cast(), core::mem::size_of::<MessageQueueState>());
    }
}

fn message_insert(mq: MessageQueueHandle, message: Message, flags: u32, jam: bool) -> bool {
    ensure_initialized();
    loop {
        let irq = arch::disable_interrupts();
        unsafe {
            let Some(state_ptr) = message_queue_state(mq) else {
                arch::restore_interrupts(irq);
                return false;
            };
            let state = &mut *state_ptr;
            if state.len < state.capacity {
                if jam {
                    state.head = (state.head + state.capacity - 1) % state.capacity;
                    state.messages.add(state.head).write(message);
                } else {
                    let tail = (state.head + state.len) % state.capacity;
                    state.messages.add(tail).write(message);
                }
                state.len += 1;
                wake_one_locked(&mut state.receive_waiters, WaitResult::Signaled, 0);
                arch::restore_interrupts(irq);
                return true;
            }
            if message_nonblocking(flags) {
                arch::restore_interrupts(irq);
                return false;
            }
            wait_queue_insert_locked(core::ptr::addr_of_mut!(state.send_waiters), CURRENT);
            if block_current_locked(WaitKind::MessageSend, mq as usize, 0, NO_DEADLINE, irq)
                == WaitResult::Cancelled
            {
                return false;
            }
        }
    }
}

pub fn message_send(mq: MessageQueueHandle, message: Message, flags: u32) -> bool {
    // C's MQ_Send currently forces MQ_MSG_NOBLOCK through its unconditional
    // PPCIsInExcpt guard. Keep the same observable behavior at the Rust API.
    message_insert(mq, message, flags | MQ_MSG_NOBLOCK, false)
}
pub fn message_jam(mq: MessageQueueHandle, message: Message, flags: u32) -> bool {
    message_insert(mq, message, flags, true)
}

#[inline]
fn message_nonblocking(flags: u32) -> bool {
    flags & MQ_MSG_NOBLOCK != 0
}

unsafe fn message_pop(state: &mut MessageQueueState, output: *mut Message) -> bool {
    if state.len == 0 {
        return false;
    }
    let message = unsafe { state.messages.add(state.head).read() };
    if !output.is_null() {
        unsafe { *output = message };
    }
    unsafe { state.messages.add(state.head).write(ptr::null_mut()) };
    state.head = (state.head + 1) % state.capacity;
    state.len -= 1;
    true
}

/// Receives a message and optionally writes it to `output`.
///
/// # Safety
/// A non-null `output` must be writable for one [`Message`].
pub unsafe fn message_receive(mq: MessageQueueHandle, output: *mut Message, flags: u32) -> bool {
    ensure_initialized();
    let flags = flags
        | if arch::in_exception() {
            MQ_MSG_NOBLOCK
        } else {
            0
        };
    loop {
        let irq = arch::disable_interrupts();
        unsafe {
            let Some(state_ptr) = message_queue_state(mq) else {
                arch::restore_interrupts(irq);
                return false;
            };
            let state = &mut *state_ptr;
            if message_pop(state, output) {
                wake_one_locked(&mut state.send_waiters, WaitResult::Signaled, 0);
                arch::restore_interrupts(irq);
                return true;
            }
            if message_nonblocking(flags) {
                arch::restore_interrupts(irq);
                return false;
            }
            wait_queue_insert_locked(core::ptr::addr_of_mut!(state.receive_waiters), CURRENT);
            if block_current_locked(WaitKind::MessageReceive, mq as usize, 0, NO_DEADLINE, irq)
                == WaitResult::Cancelled
            {
                return false;
            }
        }
    }
}

/// Wait while a live atomic word still has the expected value.
///
/// The comparison and scheduler registration occur with interrupts masked, so
/// a store followed by a wake cannot be lost between those operations on Wii's
/// single CPU. The word must remain at this address through the wait. No lock
/// or condition pool slot is allocated. Cancellation is an error, not a wake.
pub fn futex_wait(
    word: &AtomicU32,
    expected: u32,
    timeout_ticks: Option<u64>,
) -> Result<FutexWait, FutexError> {
    if arch::in_exception() {
        return Err(FutexError::ExceptionContext);
    }
    if arch::current_msr() & (1 << 15) == 0 {
        return Err(FutexError::InterruptsDisabled);
    }
    if timeout_ticks.is_some_and(|ticks| ticks > futex::FUTEX_MAX_TIMEOUT_TICKS) {
        return Err(FutexError::InvalidTimeout);
    }
    ensure_initialized();
    let irq = arch::disable_interrupts();
    if word.load(Ordering::Relaxed) != expected {
        arch::restore_interrupts(irq);
        return Ok(FutexWait::ValueChanged);
    }
    if timeout_ticks == Some(0) {
        arch::restore_interrupts(irq);
        return Ok(FutexWait::TimedOut);
    }
    let deadline = match timeout_ticks {
        Some(ticks) => match futex::deadline(arch::read_timebase(), ticks) {
            Ok(deadline) => deadline,
            Err(error) => {
                arch::restore_interrupts(irq);
                return Err(error);
            }
        },
        None => NO_DEADLINE,
    };
    // SAFETY: Interrupts remain masked from the comparison through publication
    // of the blocked thread. The scheduler restores irq when it switches out.
    match unsafe {
        block_current_locked(
            WaitKind::Futex,
            word as *const AtomicU32 as usize,
            0,
            deadline,
            irq,
        )
    } {
        WaitResult::Signaled => Ok(FutexWait::Woken),
        WaitResult::TimedOut => Ok(FutexWait::TimedOut),
        WaitResult::Cancelled => Err(FutexError::Cancelled),
    }
}

/// Wake one waiter or all waiters at this word's address. Return the real count.
/// Wake-one selects the highest effective priority, then the oldest waiter.
/// A wake is not retained when there is no waiter. Atomic values and ordering
/// belong to the caller's synchronization protocol. This may run in an IRQ.
pub fn futex_wake(word: &AtomicU32, all: bool) -> usize {
    let irq = arch::disable_interrupts();
    let key = word as *const AtomicU32 as usize;
    let mut count = 0;
    // SAFETY: Interrupt masking serializes all scheduler state. Wake does not
    // switch threads or access the contents of the caller's atomic word.
    unsafe {
        if INITIALIZED {
            let mut best = None;
            for index in 0..thread_capacity() {
                let thread = &THREADS[index];
                if thread.state == ThreadState::Blocked
                    && thread.wait_kind == WaitKind::Futex
                    && thread.wait_object == key
                {
                    if all {
                        wake_index_locked(index, WaitResult::Signaled, 0);
                        count += 1;
                    } else {
                        best = futex::select(
                            best,
                            futex::Waiter {
                                index,
                                priority: thread.effective_priority,
                                order: thread.wait_order,
                            },
                        );
                    }
                }
            }
            if let Some(waiter) = best {
                wake_index_locked(waiter.index, WaitResult::Signaled, 0);
                count = 1;
            }
        }
    }
    arch::restore_interrupts(irq);
    count
}

/// Duration form of futex_wait. Long waits use bounded scheduler deadlines;
/// each timeout deducts the real elapsed ticks before another wait is issued.
pub fn futex_wait_for(
    word: &AtomicU32,
    expected: u32,
    timeout: Option<core::time::Duration>,
) -> Result<FutexWait, FutexError> {
    let Some(timeout) = timeout else {
        return futex_wait(word, expected, None);
    };
    let mut budget = futex::TickBudget::new(timeout, TICKS_PER_SECOND);
    loop {
        let ticks = budget.chunk();
        let started = now_ticks();
        let result = futex_wait(word, expected, Some(ticks))?;
        if result != FutexWait::TimedOut {
            return Ok(result);
        }
        if budget.elapse(now_ticks().wrapping_sub(started)) {
            return Ok(FutexWait::TimedOut);
        }
    }
}

pub fn block_current_external(key: usize, token: u32) -> u32 {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    unsafe {
        let current = CURRENT;
        if THREADS[current].pending_wake {
            THREADS[current].pending_wake = false;
            let pending = THREADS[current].pending_token;
            THREADS[current].pending_token = 0;
            arch::restore_interrupts(irq);
            return pending;
        }
        let wait_result = block_current_locked(WaitKind::External, key, token, NO_DEADLINE, irq);
        if wait_result == WaitResult::Cancelled {
            0
        } else {
            THREADS[current].wait_token
        }
    }
}

/// Wakes a Tuxedo-compatible external wait. If the target thread is between
/// publishing its wait intent and entering the scheduler, the token is latched
/// and consumed by `block_current_external` instead of being lost.
pub fn wake_thread_with_token(thread: LwpThread, token: u32) -> bool {
    let Some(index) = thread_index(thread) else {
        return false;
    };
    let irq = arch::disable_interrupts();
    let ok = unsafe {
        if THREADS[index].state == ThreadState::Blocked
            && THREADS[index].wait_kind == WaitKind::External
        {
            wake_index_locked(index, WaitResult::Signaled, token);
            true
        } else if !matches!(
            THREADS[index].state,
            ThreadState::Unused | ThreadState::Finished
        ) {
            THREADS[index].pending_wake = true;
            THREADS[index].pending_token = token;
            true
        } else {
            false
        }
    };
    arch::restore_interrupts(irq);
    ok
}

fn wake_external(key: usize, reference: u32, mask: bool, all: bool) {
    let irq = arch::disable_interrupts();
    unsafe {
        let mut index = 0;
        let capacity = thread_capacity();
        while index < capacity {
            if THREADS[index].state == ThreadState::Blocked
                && THREADS[index].wait_kind == WaitKind::External
                && THREADS[index].wait_object == key
            {
                let matched = if mask {
                    (THREADS[index].wait_token & reference) != 0
                } else {
                    THREADS[index].wait_token == reference
                };
                if matched {
                    let token = if mask {
                        THREADS[index].wait_token & reference
                    } else {
                        1
                    };
                    wake_index_locked(index, WaitResult::Signaled, token);
                    if !all {
                        break;
                    }
                }
            }
            index += 1;
        }
    }
    arch::restore_interrupts(irq);
}
pub fn wake_external_by_value(key: usize, reference: u32, all: bool) {
    wake_external(key, reference, false, all);
}
pub fn wake_external_by_mask(key: usize, reference: u32, all: bool) {
    wake_external(key, reference, true, all);
}
pub fn broadcast_external(key: usize) {
    let irq = arch::disable_interrupts();
    unsafe {
        let mut index = 0;
        let capacity = thread_capacity();
        while index < capacity {
            if THREADS[index].state == ThreadState::Blocked
                && THREADS[index].wait_kind == WaitKind::External
                && THREADS[index].wait_object == key
            {
                wake_index_locked(index, WaitResult::Signaled, 1);
            }
            index += 1;
        }
    }
    arch::restore_interrupts(irq);
}
pub fn cancel_external_wait(key: usize, thread: LwpThread) {
    if let Some(index) = thread_index(thread) {
        let irq = arch::disable_interrupts();
        unsafe {
            if THREADS[index].wait_kind == WaitKind::External && THREADS[index].wait_object == key {
                wake_index_locked(index, WaitResult::Cancelled, 0);
            } else {
                THREADS[index].pending_wake = false;
                THREADS[index].pending_token = 0;
            }
        }
        arch::restore_interrupts(irq);
    }
}

/// Queues a callback for execution on the deferred worker.
///
/// # Safety
/// `arg` must satisfy the callback's contract and remain valid until the callback runs.
pub unsafe fn deferred_enqueue(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    let rc = unsafe { deferred_enqueue_locked(callback, arg) };
    arch::restore_interrupts(irq);
    rc
}

/// Reserves one interrupt-deferred queue entry for a completion that will be
/// delivered later from interrupt context.
///
/// The caller must either cancel the reservation or enqueue one callback with
/// [`deferred_enqueue_reserved_from_interrupt`]. Ordinary interrupt callbacks
/// cannot consume reserved capacity.
pub fn deferred_reserve_interrupt_slot() -> Option<u32> {
    ensure_initialized();
    let irq = arch::disable_interrupts();
    let reservation = unsafe { interrupt_deferred_slots().reserve() };
    arch::restore_interrupts(irq);
    reservation
}

/// Releases one interrupt-deferred queue reservation that was not used.
pub fn deferred_cancel_interrupt_slot(reservation: u32) -> i32 {
    let irq = arch::disable_interrupts();
    let cancelled = unsafe { interrupt_deferred_slots().cancel_reservation(reservation) };
    arch::restore_interrupts(irq);
    if cancelled {
        LWP_SUCCESSFUL
    } else {
        LWP_CLOSED
    }
}

/// Queues a callback from hardware/IOS interrupt context without running user
/// code in the handler. The caller must have initialized LWP in thread context.
/// # Safety
/// `arg` must satisfy the callback's contract and remain valid until the callback runs.
pub unsafe fn deferred_enqueue_from_interrupt(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    let irq = arch::disable_interrupts();
    let rc = unsafe {
        if !INITIALIZED {
            LWP_CLOSED
        } else {
            interrupt_deferred_enqueue_locked(callback, arg)
        }
    };
    arch::restore_interrupts(irq);
    rc
}

/// Queues a callback by consuming a slot reserved before interrupt context.
///
/// # Safety
/// The caller must own one reservation. `arg` must satisfy the callback's
/// contract and remain valid until the callback runs.
pub unsafe fn deferred_enqueue_reserved_from_interrupt(
    reservation: u32,
    callback: DeferredCallback,
    arg: *mut c_void,
) -> i32 {
    let irq = arch::disable_interrupts();
    let rc = unsafe {
        if !INITIALIZED {
            LWP_CLOSED
        } else {
            interrupt_deferred_enqueue_reserved_locked(reservation, callback, arg)
        }
    };
    arch::restore_interrupts(irq);
    rc
}

/// Registers a timer callback for the PowerPC decrementer handler.
///
/// # Safety
/// The callback runs with interrupts disabled in decrementer exception context.
/// It must not block or depend on thread-only state. `arg` must satisfy the
/// callback's contract and remain valid until cancellation or its final call.
pub unsafe fn timer_start(
    key: usize,
    callback: TimerCallback,
    arg: *mut c_void,
    delay: u64,
    period: u64,
) -> i32 {
    unsafe { timer_start_inner(key, callback, None, arg, delay, period) }
}

/// Register a timer callback that receives its stable key.
///
/// The callback runs with interrupts disabled in decrementer exception context.
/// It must not block or depend on thread-only state.
///
/// # Safety
/// `arg` must remain valid until the timer is cancelled or its final call completes.
pub unsafe fn timer_start_with_key(
    key: usize,
    callback: KeyedTimerCallback,
    arg: *mut c_void,
    delay: u64,
    period: u64,
) -> i32 {
    unsafe { timer_start_inner(key, None, callback, arg, delay, period) }
}

unsafe fn timer_start_inner(
    key: usize,
    callback: TimerCallback,
    keyed_callback: KeyedTimerCallback,
    arg: *mut c_void,
    delay: u64,
    period: u64,
) -> i32 {
    if callback.is_none() && keyed_callback.is_none() {
        return LWP_CLOSED;
    }
    ensure_initialized();

    let irq = arch::disable_interrupts();
    unsafe {
        let mut current = TIMER_HEAD;
        while !current.is_null() {
            if (*current).timer.key == key {
                let order = next_timer_order_locked();
                (*current).timer = TimerState {
                    key,
                    order,
                    callback,
                    keyed_callback,
                    arg,
                    deadline: arch::read_timebase().wrapping_add(delay),
                    period,
                };
                reschedule_decrementer_locked(arch::read_timebase());
                arch::restore_interrupts(irq);
                return LWP_SUCCESSFUL;
            }
            current = (*current).next;
        }
    }
    arch::restore_interrupts(irq);

    let irq = arch::disable_interrupts();
    let recycled = unsafe {
        let node = TIMER_FREE_HEAD;
        if !node.is_null() {
            TIMER_FREE_HEAD = (*node).next;
        }
        node
    };
    arch::restore_interrupts(irq);
    let node = if recycled.is_null() {
        unsafe { workspace_alloc(core::mem::size_of::<TimerNode>()) }.cast::<TimerNode>()
    } else {
        recycled
    };
    if node.is_null() {
        return LWP_CLOSED;
    }

    let irq = arch::disable_interrupts();
    unsafe {
        let mut current = TIMER_HEAD;
        while !current.is_null() {
            if (*current).timer.key == key {
                let order = next_timer_order_locked();
                (*current).timer = TimerState {
                    key,
                    order,
                    callback,
                    keyed_callback,
                    arg,
                    deadline: arch::read_timebase().wrapping_add(delay),
                    period,
                };
                recycle_timer_node(node);
                arch::restore_interrupts(irq);
                return LWP_SUCCESSFUL;
            }
            current = (*current).next;
        }
        let order = next_timer_order_locked();
        node.write(TimerNode {
            timer: TimerState {
                key,
                order,
                callback,
                keyed_callback,
                arg,
                deadline: arch::read_timebase().wrapping_add(delay),
                period,
            },
            next: TIMER_HEAD,
        });
        TIMER_HEAD = node;
        reschedule_decrementer_locked(arch::read_timebase());
    }
    arch::restore_interrupts(irq);
    LWP_SUCCESSFUL
}

fn timer_due(now: u64, deadline: u64) -> bool {
    // C uses `(s64)(target - now) <= 0`. Casting the wrapping difference
    // preserves the exact half-range boundary behavior as well as ordinary
    // wraparound semantics.
    deadline.wrapping_sub(now) as i64 <= 0
}

fn timer_precedes(deadline: u64, order: u64, other_deadline: u64, other_order: u64) -> bool {
    let distance = deadline.wrapping_sub(other_deadline) as i64;
    distance < 0 || (distance == 0 && order > other_order)
}

unsafe fn next_timer_order_locked() -> u64 {
    unsafe {
        let order = *core::ptr::addr_of!(NEXT_TIMER_ORDER);
        let next = order.wrapping_add(1);
        *core::ptr::addr_of_mut!(NEXT_TIMER_ORDER) = if next == 0 { 1 } else { next };
        order
    }
}
pub fn timer_cancel(key: usize) {
    let irq = arch::disable_interrupts();
    let removed = unsafe {
        let mut link = core::ptr::addr_of_mut!(TIMER_HEAD);
        let mut removed = ptr::null_mut();
        while !(*link).is_null() {
            if (**link).timer.key == key {
                removed = *link;
                *link = (*removed).next;
                break;
            }
            link = core::ptr::addr_of_mut!((**link).next);
        }
        reschedule_decrementer_locked(arch::read_timebase());
        removed
    };
    if !removed.is_null() {
        unsafe { recycle_timer_node(removed) };
    }
    arch::restore_interrupts(irq);
}

pub fn set_debug_hook(hook: DebugHook) {
    let irq = arch::disable_interrupts();
    unsafe {
        DEBUG_HOOK = hook;
    }
    arch::restore_interrupts(irq);
}
pub fn debug_counters() -> DebugCounters {
    let irq = arch::disable_interrupts();
    let value = unsafe { DEBUG_COUNTERS };
    arch::restore_interrupts(irq);
    value
}
pub fn debug_drain(output: &mut [DebugEvent]) -> usize {
    let irq = arch::disable_interrupts();
    let count = unsafe { output.len().min(DEBUG_LEN) };
    unsafe {
        for item in output.iter_mut().take(count) {
            *item = DEBUG_EVENTS[DEBUG_HEAD];
            DEBUG_HEAD = (DEBUG_HEAD + 1) % MAX_DEBUG_EVENTS;
            DEBUG_LEN -= 1;
        }
    }
    arch::restore_interrupts(irq);
    count
}
pub fn note_ipc_wake(queue: QueueHandle) {
    let irq = arch::disable_interrupts();
    unsafe {
        DEBUG_COUNTERS.ipc_wakeups = DEBUG_COUNTERS.ipc_wakeups.saturating_add(1);
        debug_locked(EVENT_IPC_WAKE, queue, 0, 0);
    }
    arch::restore_interrupts(irq);
}

#[cfg(test)]
mod waiter_priority_tests {
    use super::*;

    fn current_base_priority() -> u8 {
        let irq = arch::disable_interrupts();
        let priority = unsafe { THREADS[CURRENT].base_priority };
        arch::restore_interrupts(irq);
        priority
    }

    #[test]
    fn null_priority_handle_selects_current_thread_and_invalid_handle_is_ignored() {
        ensure_initialized();
        let current = current_thread();
        let original_priority = current_base_priority();

        set_thread_priority(LWP_THREAD_NULL, 37);
        assert_eq!(current_base_priority(), 37);

        set_thread_priority(current, 52);
        assert_eq!(current_base_priority(), 52);

        set_thread_priority(0, 9);
        assert_eq!(current_base_priority(), 52);

        set_thread_priority(current, original_priority as u32);
    }

    #[test]
    fn synchronization_handles_round_trip_for_every_object_pool() {
        for (capacity, null) in [
            (MAX_QUEUES, LWP_TQUEUE_NULL),
            (MAX_MUTEXES, LWP_MUTEX_NULL),
            (MAX_CONDS, LWP_COND_NULL),
            (MAX_SEMAPHORES, LWP_SEM_NULL),
        ] {
            assert_eq!(synchronization_handle(0), 1);
            for index in 0..capacity {
                assert_eq!(
                    decode(synchronization_handle(index), capacity, null),
                    Some(index)
                );
            }
            assert_eq!(decode(0, capacity, null), None);
            assert_eq!(decode(null, capacity, null), None);
            assert_eq!(
                decode(synchronization_handle(capacity), capacity, null),
                None
            );
        }
    }

    #[test]
    fn changed_blocked_priority_requeues_after_existing_equal_priority_waiters() {
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        for (index, priority, order) in [(0, 8, 10), (1, 7, 11), (2, 7, 12)] {
            threads[index].state = ThreadState::Blocked;
            threads[index].wait_kind = WaitKind::Queue;
            threads[index].effective_priority = priority;
            threads[index].wait_order = order;
        }
        let mut next_order = 13;

        assert!(update_effective_priority(
            &mut threads[0],
            7,
            &mut next_order
        ));
        assert_eq!(threads[0].wait_order, 13);

        let mut waiters = WaitQueue::empty();
        for index in [0, 1, 2] {
            wait_queue_insert(&mut waiters, &mut threads, index);
        }
        assert_eq!(take_best_waiter_by(&mut waiters, &mut threads), Some(1));
        assert_eq!(take_best_waiter_by(&mut waiters, &mut threads), Some(2));
        assert_eq!(take_best_waiter_by(&mut waiters, &mut threads), Some(0));
    }

    #[test]
    fn unchanged_blocked_priority_preserves_fifo_order() {
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        for (index, order) in [(0, 20), (1, 21)] {
            threads[index].state = ThreadState::Blocked;
            threads[index].wait_kind = WaitKind::Mutex;
            threads[index].effective_priority = 7;
            threads[index].wait_order = order;
        }
        let mut next_order = 22;

        assert!(!update_effective_priority(
            &mut threads[0],
            7,
            &mut next_order
        ));
        assert_eq!(threads[0].wait_order, 20);
        let mut waiters = WaitQueue::empty();
        wait_queue_insert(&mut waiters, &mut threads, 0);
        wait_queue_insert(&mut waiters, &mut threads, 1);
        assert_eq!(take_best_waiter_by(&mut waiters, &mut threads), Some(0));
        assert_eq!(take_best_waiter_by(&mut waiters, &mut threads), Some(1));
    }

    #[test]
    fn inherited_priority_propagates_through_a_mutex_owner_chain() {
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        threads[0].state = ThreadState::Blocked;
        threads[0].wait_kind = WaitKind::Mutex;
        threads[0].wait_object = 1;
        threads[0].base_priority = 10;
        threads[0].effective_priority = 10;
        threads[1].state = ThreadState::Running;
        threads[1].base_priority = 20;
        threads[1].effective_priority = 20;
        threads[2].state = ThreadState::Blocked;
        threads[2].wait_kind = WaitKind::Mutex;
        threads[2].base_priority = 100;
        threads[2].effective_priority = 100;

        let mut mutexes = [MutexState::empty(); MAX_MUTEXES];
        mutexes[0].used = true;
        mutexes[0].owner = 0;
        wait_queue_insert(&mut mutexes[0].waiters, &mut threads, 2);
        mutexes[1].used = true;
        mutexes[1].owner = 1;
        wait_queue_insert(&mut mutexes[1].waiters, &mut threads, 0);
        let mut next_order = 1;

        recompute_priority_chain(&mut threads, &mutexes, 0, &mut next_order);

        assert_eq!(threads[0].effective_priority, 100);
        assert_eq!(threads[1].effective_priority, 100);
    }

    #[test]
    fn priority_values_use_the_c_mask() {
        assert_eq!(decode_priority(127), 127);
        assert_eq!(decode_priority(128), 0);
        assert_eq!(decode_priority(255), 127);
    }

    #[test]
    fn message_no_block_flag_uses_bit_masking() {
        assert!(!message_nonblocking(MQ_MSG_BLOCK));
        assert!(message_nonblocking(MQ_MSG_NOBLOCK));
        assert!(message_nonblocking(MQ_MSG_NOBLOCK | 2));
    }

    #[test]
    fn receiving_without_output_still_consumes_the_message() {
        let mut state = MessageQueueState::empty();
        state.used = true;
        state.capacity = 2;
        state.len = 1;
        let mut messages = [1usize as Message, ptr::null_mut()];
        state.messages = messages.as_mut_ptr();

        assert!(unsafe { message_pop(&mut state, ptr::null_mut()) });
        assert_eq!(state.len, 0);
        assert!(messages[0].is_null());
        assert!(!unsafe { message_pop(&mut state, ptr::null_mut()) });
    }

    #[test]
    fn message_queue_close_does_not_wake_blocked_waiters() {
        let mut state = MessageQueueState::empty();
        state.used = true;
        state.capacity = 4;
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        wait_queue_insert(&mut state.send_waiters, &mut threads, 2);
        wait_queue_insert(&mut state.receive_waiters, &mut threads, 5);

        state.used = false;

        assert!(!state.used);
        assert_eq!(state.send_waiters.head, 2);
        assert_eq!(state.receive_waiters.head, 5);
    }

    #[test]
    fn semaphore_destroy_and_reuse_preserve_blocked_waiters() {
        let mut state = SemaphoreState {
            used: true,
            value: -1,
            waiters: WaitQueue { head: 3 },
        };

        state.used = false;
        assert_eq!(state.waiters.head, 3);

        reinitialize_semaphore_state(&mut state, 0);
        assert!(state.used);
        assert_eq!(state.value, 0);
        assert_eq!(state.waiters.head, 3);
    }

    #[test]
    fn timer_due_matches_c_at_half_range_boundary() {
        assert!(timer_due(0, 0));
        assert!(!timer_due(0, 1));
        assert!(timer_due(1, 0));
        assert!(timer_due(0, 1u64 << 63));
    }

    #[test]
    fn timer_order_matches_c_for_equal_deadlines() {
        assert!(timer_precedes(100, 9, 100, 8));
        assert!(!timer_precedes(100, 8, 100, 9));
        assert!(timer_precedes(99, 1, 100, 9));
        assert!(!timer_precedes(101, 10, 100, 9));
    }

    #[test]
    fn mutex_handoff_assigns_the_highest_priority_waiter() {
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        threads[3].effective_priority = 30;
        threads[3].wait_order = 1;
        threads[4].effective_priority = 80;
        threads[4].wait_order = 2;
        let mut mutex = MutexState {
            used: true,
            recursive: false,
            owner: 1,
            recursion: 1,
            waiters: WaitQueue::empty(),
        };
        wait_queue_insert(&mut mutex.waiters, &mut threads, 3);
        wait_queue_insert(&mut mutex.waiters, &mut threads, 4);

        assert_eq!(handoff_mutex(&mut mutex, &mut threads), Some(4));
        assert_eq!(mutex.owner, 4);
        assert_eq!(mutex.recursion, 1);
        assert_eq!(mutex.waiters.head, 3);
    }
}

#[cfg(test)]
mod context_mirror_tests {
    use super::*;

    #[test]
    fn restore_consumes_caller_owned_context_without_context_alignment() {
        let mut expected = Context::zeroed();
        expected.pc = 0x1234_5678;
        expected.lr = 0x8765_4321;
        expected.gpr[3] = 0xfeed_beef;
        expected.gqr[7] = 0x1357_2468;

        #[repr(align(32))]
        struct AlignedWords([u32; 177]);

        let mut storage = AlignedWords([0; 177]);
        let mirror = unsafe { storage.0.as_mut_ptr().add(1) }.cast::<u8>();
        assert_eq!(mirror as usize % core::mem::align_of::<u32>(), 0);
        assert_ne!(mirror as usize % core::mem::align_of::<Context>(), 0);
        unsafe {
            ptr::copy_nonoverlapping(
                ptr::addr_of!(expected).cast::<u8>(),
                mirror,
                core::mem::size_of::<Context>(),
            )
        };

        let mut restored = Context::zeroed();
        unsafe { copy_context_from_mirror(ptr::addr_of_mut!(restored), mirror) };

        assert_eq!(restored.pc, expected.pc);
        assert_eq!(restored.lr, expected.lr);
        assert_eq!(restored.gpr[3], expected.gpr[3]);
        assert_eq!(restored.gqr[7], expected.gqr[7]);
    }
}

#[cfg(test)]
mod interrupt_preemption_tests {
    use super::*;

    #[test]
    fn interrupt_return_preempts_only_for_a_strictly_higher_ready_thread() {
        let mut threads = [const { Thread::empty() }; THREAD_PAGE_SLOTS];
        threads[0].state = ThreadState::Ready;
        threads[0].effective_priority = 40;
        threads[1].state = ThreadState::Ready;
        threads[1].effective_priority = 39;
        assert!(!unsafe { has_higher_priority_ready(threads.as_ptr(), 40) });

        threads[2].state = ThreadState::Ready;
        threads[2].effective_priority = 41;
        assert!(unsafe { has_higher_priority_ready(threads.as_ptr(), 40) });

        threads[2].state = ThreadState::Blocked;
        assert!(!unsafe { has_higher_priority_ready(threads.as_ptr(), 40) });
    }
}

#[cfg(test)]
mod reent_tests {
    use super::*;

    #[test]
    fn newlib_reent_init_matches_devkitppc_zeroed_initialization() {
        let mut storage = ReentStorage::empty();
        let mut files = [0u8; REENT_FILE_SIZE * 3];
        unsafe { initialize_reent_with_streams(&mut storage, files.as_mut_ptr()) };

        assert_eq!(
            unsafe { ptr::read_unaligned(storage.0.as_ptr().cast::<i32>()) },
            0
        );
        for stream in 0..3 {
            let expected = unsafe { files.as_mut_ptr().add(stream * REENT_FILE_SIZE) } as usize;
            let actual = unsafe {
                ptr::read_unaligned(
                    storage
                        .0
                        .as_ptr()
                        .add(REENT_STREAMS_OFFSET + stream * core::mem::size_of::<usize>())
                        .cast::<usize>(),
                )
            };
            assert_eq!(actual, expected);
        }
        assert_eq!(
            unsafe {
                ptr::read_unaligned(storage.0.as_ptr().add(REENT_RAND_NEXT_OFFSET).cast::<u64>())
            },
            1
        );
        let rand48 = unsafe {
            core::slice::from_raw_parts(
                storage
                    .0
                    .as_ptr()
                    .add(REENT_RAND48_SEED_OFFSET)
                    .cast::<u16>(),
                7,
            )
        };
        assert_eq!(
            rand48,
            &[0x330e, 0xabcd, 0x1234, 0xe66d, 0xdeec, 0x0005, 0x000b]
        );
    }
}
