use crate::parser::Side;
use crate::risk::PreTradeRiskEngine;

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderInstruction {
    pub price: i64,
    pub quantity: u32,
    pub side: Side,
    pub tsc_timestamp: u64, // Used to measure end-to-end latency from ingress
}

pub struct ExecutionStrategy {
    current_position: i64,
    spread: i64,        // Fixed-point half-spread skew (e.g. 5 ticks = 500)
    order_qty: u32,     // Standard order execution quantity
    risk_engine: PreTradeRiskEngine,
}

impl ExecutionStrategy {
    pub fn new(spread: i64, order_qty: u32, risk_engine: PreTradeRiskEngine) -> Self {
        Self {
            current_position: 0,
            spread,
            order_qty,
            risk_engine,
        }
    }

    /// Evaluates the current market state.
    /// Runs pre-trade risk checks in-line and outputs an instruction if valid.
    #[inline(always)]
    pub fn evaluate_market(
        &mut self,
        best_bid: i64,
        best_ask: i64,
        arrival_tsc: u64,
    ) -> Option<OrderInstruction> {
        if best_bid == 0 || best_ask == 0 {
            return None;
        }

        // Calculate mid-price using fast bit-shifting (division by 2)
        let mid_price = (best_bid + best_ask) >> 1;

        // Inventory-based skew. If we are long (positive position), we skew prices
        // down to encourage selling and discourage buying. If short (negative position),
        // we skew prices up. Skew is scaled to price units (e.g., skew offset = position / 10).
        let skew_offset = self.current_position >> 3; // Shift right by 3 is fast division by 8

        // Place a Buy order slightly below mid skew or Sell order slightly above mid skew
        // depending on current inventory skew rules.
        let (side, target_price) = if self.current_position > 1000 {
            // Too long: skew towards selling
            (Side::Sell, mid_price + self.spread - skew_offset)
        } else if self.current_position < -1000 {
            // Too short: skew towards buying
            (Side::Buy, mid_price - self.spread - skew_offset)
        } else {
            // Neutral: alternate sides based on least significant bit of timestamp to prevent bias
            let choice_bit = arrival_tsc & 1;
            if choice_bit == 0 {
                (Side::Buy, mid_price - self.spread - skew_offset)
            } else {
                (Side::Sell, mid_price + self.spread - skew_offset)
            }
        };

        let is_buy = match side {
            Side::Buy => true,
            Side::Sell => false,
        };

        // 3. Pre-Trade Risk Engine Check (must be <100ns)
        match self.risk_engine.check_order(
            self.order_qty,
            target_price,
            self.current_position,
            mid_price,
            is_buy,
            arrival_tsc,
        ) {
            Ok(()) => {
                // Order approved. Update position.
                if is_buy {
                    self.current_position += self.order_qty as i64;
                } else {
                    self.current_position -= self.order_qty as i64;
                }

                Some(OrderInstruction {
                    price: target_price,
                    quantity: self.order_qty,
                    side,
                    tsc_timestamp: arrival_tsc,
                })
            }
            Err(_reason) => {
                // Reject order (e.g., position breach, kill switch active, etc.)
                None
            }
        }
    }

    #[inline(always)]
    pub fn get_position(&self) -> i64 {
        self.current_position
    }

    #[inline(always)]
    pub fn reset_position(&mut self) {
        self.current_position = 0;
    }
}
