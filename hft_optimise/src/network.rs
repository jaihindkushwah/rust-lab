use std::net::UdpSocket;

/// Pre-allocated pool of FIX messages to avoid any allocation inside the benchmark loop.
pub struct MessagePool {
    messages: Vec<Vec<u8>>,
}

impl MessagePool {
    pub fn new(size: usize) -> Self {
        let mut messages = Vec::with_capacity(size);
        for seq in 0..size {
            let symbol = match seq % 4 {
                0 => "AAPL",
                1 => "MSFT",
                2 => "GOOG",
                _ => "TSLA",
            };
            let side = if seq % 2 == 0 { "1" } else { "2" };
            // Price alternates between 100.00 and 109.90
            let price = 100.0 + (seq % 100) as f64 * 0.10;
            let qty = 100 + (seq % 10) * 100;
            let msg = format!(
                "8=FIX.4.2\x0135=X\x0155={}\x0144={:.2}\x0138={}\x0154={}\x01",
                symbol, price, qty, side
            );
            messages.push(msg.into_bytes());
        }
        Self { messages }
    }

    /// Retrieve a pre-allocated message byte slice by sequence.
    #[inline(always)]
    pub fn get(&self, seq: usize) -> &[u8] {
        &self.messages[seq % self.messages.len()]
    }
}

pub struct UdpReceiver {
    socket: Option<UdpSocket>,
}

impl UdpReceiver {
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Some(socket),
        })
    }

    pub fn new_mock() -> Self {
        Self { socket: None }
    }

    /// Attempts to read a packet from the network socket into the provided buffer.
    /// Returns the length of the read packet, or 0 if no packet is available.
    #[inline(always)]
    pub fn recv(&self, buf: &mut [u8]) -> usize {
        if let Some(ref sock) = self.socket {
            match sock.recv_from(buf) {
                Ok((amt, _src)) => amt,
                Err(_) => 0, // No data (non-blocking)
            }
        } else {
            0
        }
    }
}
