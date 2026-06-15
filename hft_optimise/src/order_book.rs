use crate::parser::Side;

pub const MAX_LEVELS: usize = 10;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PriceLevel {
    pub price: i64,
    pub quantity: u32,
}

/// A cache-aligned Order Book.
/// Forced 64-byte alignment fits CPU cache lines and prevents false sharing.
#[repr(align(64))]
#[derive(Clone, Debug)]
pub struct OrderBook {
    pub bids: [PriceLevel; MAX_LEVELS],
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

    /// Updates or inserts a price level in the order book.
    /// Bids are sorted in descending order (highest price first).
    /// Asks are sorted in ascending order (lowest price first).
    /// Quantity of 0 deletes the level.
    #[inline(always)]
    pub fn update_level(&mut self, side: Side, price: i64, qty: u32) {
        match side {
            Side::Buy => self.update_buy(price, qty),
            Side::Sell => self.update_sell(price, qty),
        }
    }

    #[inline(always)]
    fn update_buy(&mut self, price: i64, qty: u32) {
        // 1. Search if the price already exists
        for i in 0..self.num_bids {
            if self.bids[i].price == price {
                if qty == 0 {
                    // Delete the price level
                    for j in i..(self.num_bids - 1) {
                        self.bids[j] = self.bids[j + 1];
                    }
                    self.bids[self.num_bids - 1] = PriceLevel::default();
                    self.num_bids -= 1;
                } else {
                    // Update quantity
                    self.bids[i].quantity = qty;
                }
                return;
            }
        }

        // 2. If it's a new level and qty > 0, find insertion point
        if qty > 0 {
            let mut insert_idx = self.num_bids;
            for i in 0..self.num_bids {
                if price > self.bids[i].price {
                    insert_idx = i;
                    break;
                }
            }

            if insert_idx < MAX_LEVELS {
                // Shift elements to the right
                let end = if self.num_bids < MAX_LEVELS {
                    self.num_bids
                } else {
                    MAX_LEVELS - 1
                };
                
                for j in (insert_idx..end).rev() {
                    self.bids[j + 1] = self.bids[j];
                }
                self.bids[insert_idx] = PriceLevel { price, quantity: qty };
                if self.num_bids < MAX_LEVELS {
                    self.num_bids += 1;
                }
            }
        }
    }

    #[inline(always)]
    fn update_sell(&mut self, price: i64, qty: u32) {
        // 1. Search if the price already exists
        for i in 0..self.num_asks {
            if self.asks[i].price == price {
                if qty == 0 {
                    // Delete the price level
                    for j in i..(self.num_asks - 1) {
                        self.asks[j] = self.asks[j + 1];
                    }
                    self.asks[self.num_asks - 1] = PriceLevel::default();
                    self.num_asks -= 1;
                } else {
                    // Update quantity
                    self.asks[i].quantity = qty;
                }
                return;
            }
        }

        // 2. If it's a new level and qty > 0, find insertion point
        if qty > 0 {
            let mut insert_idx = self.num_asks;
            for i in 0..self.num_asks {
                if price < self.asks[i].price {
                    insert_idx = i;
                    break;
                }
            }

            if insert_idx < MAX_LEVELS {
                // Shift elements to the right
                let end = if self.num_asks < MAX_LEVELS {
                    self.num_asks
                } else {
                    MAX_LEVELS - 1
                };

                for j in (insert_idx..end).rev() {
                    self.asks[j + 1] = self.asks[j];
                }
                self.asks[insert_idx] = PriceLevel { price, quantity: qty };
                if self.num_asks < MAX_LEVELS {
                    self.num_asks += 1;
                }
            }
        }
    }

    /// Clears the entire book state.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.bids = [PriceLevel::default(); MAX_LEVELS];
        self.asks = [PriceLevel::default(); MAX_LEVELS];
        self.num_bids = 0;
        self.num_asks = 0;
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_sorting_and_updates() {
        let mut book = OrderBook::new();

        // Add bids in random order
        book.update_level(Side::Buy, 1000, 10);
        book.update_level(Side::Buy, 1020, 20);
        book.update_level(Side::Buy, 1010, 30);

        assert_eq!(book.num_bids, 3);
        // Bids must be descending
        assert_eq!(book.bids[0].price, 1020);
        assert_eq!(book.bids[0].quantity, 20);
        assert_eq!(book.bids[1].price, 1010);
        assert_eq!(book.bids[1].quantity, 30);
        assert_eq!(book.bids[2].price, 1000);
        assert_eq!(book.bids[2].quantity, 10);

        // Update a level
        book.update_level(Side::Buy, 1010, 50);
        assert_eq!(book.bids[1].quantity, 50);

        // Delete a level
        book.update_level(Side::Buy, 1010, 0);
        assert_eq!(book.num_bids, 2);
        assert_eq!(book.bids[0].price, 1020);
        assert_eq!(book.bids[1].price, 1000);

        // Add asks in random order
        book.update_level(Side::Sell, 2020, 10);
        book.update_level(Side::Sell, 2000, 20);
        book.update_level(Side::Sell, 2010, 30);

        assert_eq!(book.num_asks, 3);
        // Asks must be ascending
        assert_eq!(book.asks[0].price, 2000);
        assert_eq!(book.asks[0].quantity, 20);
        assert_eq!(book.asks[1].price, 2010);
        assert_eq!(book.asks[1].quantity, 30);
        assert_eq!(book.asks[2].price, 2020);
        assert_eq!(book.asks[2].quantity, 10);
    }
}
