use crate::common::{bytes_to_u32, float_bytes_to_fixed};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedMarketUpdate<'a> {
    pub symbol: &'a [u8],
    pub price: i64,      // Scaled integer
    pub quantity: u32,
    pub side: Side,
}

pub struct InPlaceFixParser;

impl InPlaceFixParser {
    /// Parses a FIX protocol byte slice in-place.
    /// Standard FIX tags used:
    /// - 55: Symbol (e.g., AAPL)
    /// - 44: Price (e.g., 150.25)
    /// - 38: Quantity (e.g., 100)
    /// - 54: Side (1 = Buy, 2 = Sell)
    #[inline(always)]
    pub fn parse_md_update(raw_msg: &[u8]) -> Option<ParsedMarketUpdate<'_>> {
        let mut symbol: &[u8] = &[];
        let mut price: i64 = 0;
        let mut quantity: u32 = 0;
        let mut side: Side = Side::Buy;

        let mut pos = 0;
        let len = raw_msg.len();
        
        while pos < len {
            // Find the '=' starting from pos
            let mut eq_idx = pos;
            while eq_idx < len && raw_msg[eq_idx] != b'=' {
                eq_idx += 1;
            }
            if eq_idx >= len {
                break;
            }
            
            let tag = &raw_msg[pos..eq_idx];
            
            let val_start = eq_idx + 1;
            let mut soh_idx = val_start;
            while soh_idx < len && raw_msg[soh_idx] != b'\x01' {
                soh_idx += 1;
            }
            
            let val = &raw_msg[val_start..soh_idx];
            
            // Fast tag parsing: convert to integer
            let mut tag_num = 0u32;
            for &b in tag {
                if b >= b'0' && b <= b'9' {
                    tag_num = tag_num * 10 + (b - b'0') as u32;
                } else {
                    tag_num = 0;
                    break;
                }
            }
            
            match tag_num {
                55 => symbol = val,
                44 => price = float_bytes_to_fixed(val),
                38 => quantity = bytes_to_u32(val),
                54 => {
                    if !val.is_empty() {
                        side = if val[0] == b'1' { Side::Buy } else { Side::Sell };
                    }
                }
                _ => {}
            }
            
            pos = soh_idx + 1;
        }

        if symbol.is_empty() || price == 0 || quantity == 0 {
            None
        } else {
            Some(ParsedMarketUpdate {
                symbol,
                price,
                quantity,
                side,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_msg() {
        let msg = b"8=FIX.4.2\x0135=X\x0155=AAPL\x0144=150.2500\x0138=500\x0154=1\x01";
        let parsed = InPlaceFixParser::parse_md_update(msg).unwrap();
        assert_eq!(parsed.symbol, b"AAPL");
        assert_eq!(parsed.price, 1502500); // scaled by 10_000
        assert_eq!(parsed.quantity, 500);
        assert_eq!(parsed.side, Side::Buy);
    }

    #[test]
    fn test_parse_invalid_msg() {
        let msg = b"8=FIX.4.2\x0135=X\x0155=\x0144=150.25\x01";
        assert!(InPlaceFixParser::parse_md_update(msg).is_none());
    }
}
