use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// 1. PERFORMANCE UTILITIES & CONSTANTS
// ============================================================================
const MAX_LEVELS: usize = 10;
const RING_BUFFER_SIZE: usize = 1024; // Must be a power of 2

// Fixed-point calculation setting: 4 decimal places (e.g., 150.25 -> 1502500)
const PRICE_MULTIPLIER: i64 = 10000;

#[inline(always)]
fn float_bytes_to_fixed(bytes: &[u8]) -> i64 {
    // Fast float parsing directly from bytes to eliminate heap string conversion
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Ok(f) = s.parse::<f64>() {
            return (f * PRICE_MULTIPLIER as f64) as i64;
        }
    }
    0
}

#[inline(always)]
fn bytes_to_uint(bytes: &[u8]) -> u32 {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Ok(u) = s.parse::<u32>() {
            return u;
        }
    }
    0
}

// ============================================================================
// 2. ZERO-ALLOCATION IN-PLACE FIX PARSER
// ============================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy)]
pub struct ParsedMarketUpdate<'a> {
    pub symbol: &'a [u8],
    pub price: i64,
    pub quantity: u32,
    pub side: Side,
}

pub struct InPlaceFixParser;

impl InPlaceFixParser {
    // Parses raw bytes in-place using continuous slices (Zero Allocations)
    #[inline(always)]
    pub fn parse_md_update(raw_msg: &[u8]) -> Option<ParsedMarketUpdate> {
        let mut symbol: &[u8] = &[];
        let mut price: i64 = 0;
        let mut quantity: u32 = 0;
        let mut side: Side = Side::Buy;

        let mut pos = 0;
        while pos < raw_msg.len() {
            // Find tag delimiter '='
            let eq_pos = raw_msg[pos..].iter().position(|&b| b == b'=')?;
            let tag = &raw_msg[pos..pos + eq_pos];
            let val_start = pos + eq_pos + 1;

            // Find SOH delimiter '\x01'
            let soh_pos = raw_msg[val_start..].iter().position(|&b| b == b'\x01')
                .unwrap_or(raw_msg.len() - val_start);
            let val = &raw_msg[val_start..val_start + soh_pos];

            // Fast matching using byte slices
            match tag {
                b"55" => symbol = val,
                b"44" => price = float_bytes_to_fixed(val),
                b"38" => quantity = bytes_to_uint(val),
                b"54" => side = if val[0] == b'1' { Side::Buy } else { Side::Sell },
                _ => {}
            }

            pos = val_start + soh_pos + 1;
        }

        Some(ParsedMarketUpdate { symbol, price, quantity, side })
    }
}

// ============================================================================
// 3. CACHE-LINE ALIGNED, FIXED-SIZE ORDER BOOK
// ============================================================================
#[derive(Copy, Clone, Debug, Default)]
pub struct PriceLevel {
    pub price: i64,
    pub quantity: u32,
}

// Cache line alignment (64 bytes) prevents core-to-core cache thrashing
#[repr(align(64))]
pub struct OrderBook {
    pub bids: [PriceLevel; MAX_LEVELS], // Pre-allocated flat stack arrays
    pub asks: [PriceLevel; MAX_LEVELS],
    pub num_bids: usize,
    pub num_asks: usize,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: [PriceLevel::default(); MAX_LEVELS],
            asks: [PriceLevel::default(); MAX_LEVELS],
            num_bids: 0,
            num_asks: 0,
        }
    }

    #[inline(always)]
    pub fn update_level(&mut self, side: Side, price: i64, qty: u32) {
        let (levels, count) = match side {
            Side::Buy => (&mut self.bids, &mut self.num_bids),
            Side::Sell => (&mut self.asks, &mut self.num_asks),
        };

        // Linear array scan beats complex tree allocations in small order depths
        for i in 0..*count {
            if levels[i].price == price {
                if qty == 0 {
                    // Shift values flatly to delete the price point
                    for j in i..(*count - 1) {
                        levels[j] = levels[j + 1];
                    }
                    *count -= 1;
                } else {
                    levels[i].quantity = qty;
                }
                return;
            }
        }

        if qty > 0 && *count < MAX_LEVELS {
            levels[*count] = PriceLevel { price, quantity: qty };
            *count += 1;
        }
    }
}

// ============================================================================
// 4. LOCK-FREE SINGLE-PRODUCER SINGLE-CONSUMER RING BUFFER
// ============================================================================
// UnsafeCell wrapper used natively in low-level Rust systems to bypass heap-safety bounds checks safely
struct Slot<T> {
    value: UnsafeCell<T>,
}

impl<T: Default> Slot<T> {
    fn new() -> Self {
        Self { value: UnsafeCell::new(T::default()) }
    }
}

unsafe impl<T> Sync for Slot<T> {}

#[repr(align(64))]
pub struct SpscRingBuffer<T, const N: usize> {
    buffer: Vec<Slot<T>>,
    #[業界(align(64))]
    head: AtomicUsize,
    #[業界(align(64))]
    tail: AtomicUsize,
}

impl<T: Default + Copy, const N: usize> SpscRingBuffer<T, N> {
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Buffer size must be a power of 2");
        let mut buffer = Vec::with_capacity(N);
        for _ in 0..N {
            buffer.push(Slot::new());
        }
        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn enqueue(&self, item: T) -> bool {
        let current_tail = self.tail.load(Ordering::Relaxed);
        let current_head = self.head.load(Ordering::Acquire);

        if (current_tail - current_head) >= N {
            return false; // Buffer Full
        }

        unsafe {
            let ptr = self.buffer[current_tail & (N - 1)].value.get();
            *ptr = item;
        }
        self.tail.store(current_tail + 1, Ordering::Release);
        true
    }

    #[inline(always)]
    pub fn dequeue(&self, item: &mut T) -> bool {
        let current_head = self.head.load(Ordering::Relaxed);
        let current_tail = self.tail.load(Ordering::Acquire);

        if current_head == current_tail {
            return false; // Buffer Empty
        }

        unsafe {
            let ptr = self.buffer[current_head & (N - 1)].value.get();
            *item = *ptr;
        }
        self.head.store(current_head + 1, Ordering::Release);
        true
    }
}

// ============================================================================
// 5. STRATEGY ENGINE & PRE-TRADE RISK CHECKS
// ============================================================================
pub struct ExecutionStrategy {
    max_position: i64,
    current_position: i64,
}

impl ExecutionStrategy {
    pub fn new() -> Self {
        Self {
            max_position: 10000, // Pre-trade risk rule boundary limit
            current_position: 0,
        }
    }

    #[inline(always)]
    pub fn evaluate_market(&mut self, book: &OrderBook) {
        if book.num_bids == 0 || book.num_asks == 0 {
            return;
        }

        let best_bid = book.bids[0].price;
        let best_ask = book.asks[0].price;
        
        // Branchless bit-shift computation instead of division
        let mid_price = (best_bid + best_ask) >> 1;

        if self.current_position < self.max_position {
            self.generate_order(mid_price - 100, 100, Side::Buy);
        }
    }

    #[inline(always)]
    fn generate_order(&mut self, price: i64, qty: u32, side: Side) {
        // Pre-Trade Risk Verification Validation Layer (<1 microsecond)
        if side == Side::Buy && (self.current_position + qty as i64) > self.max_position {
            println!("[RISK REJECT] Order exceeds maximum position limits.");
            return;
        }

        // Inline zero-copy generation mimicking hardware tracking parameters
        println!(
            "[ORDER SENT] Side: {:?} | Price: {:.4} | Qty: {}",
            side,
            price as f64 / PRICE_MULTIPLIER as f64,
            qty
        );

        if side == Side::Buy {
            self.current_position += qty as i64;
        }
    }
}

// ============================================================================
// MAIN APPLICATION TESTING LOOP
// ============================================================================
fn main() {
    // Initialize lock-free memory paths
    let pipeline_buffer = Arc::new(SpscRingBuffer::<ParsedMarketUpdate, RING_BUFFER_SIZE>::new());
    let mut global_order_book = OrderBook::new();
    let mut strategy = ExecutionStrategy::new();

    // Mock incoming network network data packet stream
    let inbound_packet = b"8=FIX.4.2\x01\x33\x35=X\x01\x35\x35=AAPL\x01\x34\x34=150.25\x01\x33\x38=500\x01\x35\x34=1\x01";

    println!("--- Initializing Rust HFT Engine Pipeline Core ---");

    let start_time = Instant::now();

    // 1. In-place stream parser converts raw data directly
    if let Some(update) = InPlaceFixParser::parse_md_update(inbound_packet) {
        // 2. Queue into thread pipeline without any lock contention
        pipeline_buffer.enqueue(update);
    }

    let mut processed_update = ParsedMarketUpdate {
        symbol: &[],
        price: 0,
        quantity: 0,
        side: Side::Buy,
    };

    // 3. Dequeue processed updates securely
    if pipeline_buffer.dequeue(&mut processed_update) {
        // 4. Update core layout stack allocations
        global_order_book.update_level(
            processed_update.side,
            processed_update.price,
            processed_update.quantity,
        );

        // 5. Strategy engine performs evaluation checks and issues order signals
        strategy.evaluate_market(&global_order_book);
    }

    let duration = start_time.elapsed().as_micros();
    println!("End-to-End Rust Processing Time: {} microseconds", duration);
}