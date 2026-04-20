#[derive(Debug, Clone, Copy)]
pub(in crate::cartridge::mapper::bandai_fcg) enum BandaiEepromNext {
    ReceiveAddress,
    ReceiveData,
    SendData,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::cartridge::mapper::bandai_fcg) enum BandaiEepromPhase {
    Idle,
    ReceivingControl,
    ReceivingAddress,
    ReceivingData,
    AckPending(BandaiEepromNext),
    AckLow(BandaiEepromNext),
    Sending { byte: u8, bit_index: u8 },
    WaitAckPending,
    WaitAck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::cartridge::mapper::bandai_fcg) enum BandaiEepromKind {
    None,
    C24C02,
    X24C01,
}
