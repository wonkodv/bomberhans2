const BOMBERHANS_MAGIC_NO_V1: u32 = 0x1f4a3__001; // 💣

struct Header {
    magic: u32,
    sequence: u32,
    ack: u32,
    older_acks: u32,
}

impl Header {
    fn new(sequence: u32, ack: u32, older_acks: u32) -> Self {
        Self {
            magic: BOMBERHANS_MAGIC_NO_V1,
            sequence,
            ack,
            older_acks,
        }
    }
}
