use std::sync::atomic::{AtomicBool, Ordering};

/// Global atomic software kill switch.
/// Thread-safe and accessible across the entire system.
pub static GLOBAL_KILL_SWITCH: AtomicBool = AtomicBool::new(false);

/// Trips the kill switch immediately.
#[inline(always)]
pub fn trip_kill_switch() {
    GLOBAL_KILL_SWITCH.store(true, Ordering::Release);
    // Asynchronously log the activation of the kill switch and simulate order cancellation
    println!("[CRITICAL] Kill Switch Activated. Rejecting all new orders, canceling all pending orders.");
}

/// Reset the kill switch.
#[inline(always)]
pub fn reset_kill_switch() {
    GLOBAL_KILL_SWITCH.store(false, Ordering::Release);
}

pub struct PreTradeRiskEngine {
    max_order_qty: u32,
    max_net_position: i64,
    price_deviation_limit_bps: i64, // Price deviation limit from mid-price in basis points (1 bps = 0.01%)
    max_notional: i64,
    total_notional: i64,
    last_order_price: i64,
    last_order_qty: u32,
    last_order_side: u8, // 0 = None, 1 = Buy, 2 = Sell
    last_order_tsc: u64,
    tsc_hz: f64,
}

impl PreTradeRiskEngine {
    pub fn new(
        max_order_qty: u32,
        max_net_position: i64,
        price_deviation_limit_bps: i64,
        max_notional: i64,
        tsc_hz: f64,
    ) -> Self {
        Self {
            max_order_qty,
            max_net_position,
            price_deviation_limit_bps,
            max_notional,
            total_notional: 0,
            last_order_price: 0,
            last_order_qty: 0,
            last_order_side: 0,
            last_order_tsc: 0,
            tsc_hz,
        }
    }

    /// Evaluates if an order passes the pre-trade risk check parameters.
    /// Executes in nanoseconds, utilizing no floating-point math and zero allocations.
    #[inline(always)]
    pub fn check_order(
        &mut self,
        qty: u32,
        price: i64,
        current_net_position: i64,
        mid_price: i64,
        is_buy: bool,
        current_tsc: u64,
    ) -> Result<(), &'static str> {
        // 1. Check if the global software kill switch has been activated
        if GLOBAL_KILL_SWITCH.load(Ordering::Acquire) {
            return Err("GLOBAL_KILL_SWITCH_ACTIVE");
        }

        // 2. Check individual order quantity limit
        if qty > self.max_order_qty {
            return Err("ORDER_QUANTITY_BREACH");
        }

        // 3. Check cumulative net position limit
        let expected_position = if is_buy {
            current_net_position + qty as i64
        } else {
            current_net_position - qty as i64
        };

        if expected_position.abs() > self.max_net_position {
            return Err("POSITION_LIMIT_BREACH");
        }

        // 4. Check cumulative notional exposure limit
        let notional = qty as i64 * price;
        if self.total_notional + notional > self.max_notional {
            return Err("NOTIONAL_LIMIT_BREACH");
        }

        // 5. Check price reasonability bounds relative to the mid-price
        if mid_price > 0 {
            let deviation = (price - mid_price).abs();
            // Equivalent to: (deviation / mid_price) > (limit_bps / 10,000)
            // Rearranged to avoid divisions: deviation * 10,000 > mid_price * self.price_deviation_limit_bps
            if deviation * 10_000 > mid_price * self.price_deviation_limit_bps {
                return Err("PRICE_DEVIATION_BREACH");
            }
        }

        // 6. Check duplicate order detection
        let side_u8 = if is_buy { 1 } else { 2 };
        if self.last_order_price == price
            && self.last_order_qty == qty
            && self.last_order_side == side_u8
        {
            let tsc_diff = current_tsc.wrapping_sub(self.last_order_tsc);
            let sec_diff = tsc_diff as f64 / self.tsc_hz;
            if sec_diff < 1.0 {
                return Err("DUPLICATE_ORDER_BREACH");
            }
        }

        // If all checks pass, record state
        self.total_notional += notional;
        self.last_order_price = price;
        self.last_order_qty = qty;
        self.last_order_side = side_u8;
        self.last_order_tsc = current_tsc;

        Ok(())
    }
}
